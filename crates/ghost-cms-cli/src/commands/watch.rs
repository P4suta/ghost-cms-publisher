//! `watch` — auto-publish saved Markdown files.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use ghost_cms_core::Ghost;
use ghost_cms_core::publish::{PublishOptions, publish_file};
use ghost_cms_core::transport::ReqwestTransport;
use ghost_cms_shared::error::Error;
use ghost_cms_shared::paths::state_path;
use ghost_cms_shared::render::outcome_line;
use miette::IntoDiagnostic;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer};

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::to_report;

/// Watch the blog directory and auto-publish saved Markdown files.
#[derive(Debug, clap::Args)]
pub(crate) struct WatchArgs {
    /// Directory to watch (default `<blog_dir>/posts`).
    #[arg(value_hint = clap::ValueHint::DirPath)]
    dir: Option<PathBuf>,
}

impl Command for WatchArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let watch_dir = self
            .dir
            .unwrap_or_else(|| ctx.settings.blog_dir.join("posts"));
        if !watch_dir.is_dir() {
            return Err(miette::miette!(
                "watch directory {} does not exist",
                watch_dir.display()
            ));
        }

        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(
            Duration::from_millis(400),
            move |res: DebounceEventResult| {
                let _ = tx.send(res);
            },
        )
        .into_diagnostic()?;
        debouncer
            .watcher()
            .watch(&watch_dir, RecursiveMode::NonRecursive)
            .into_diagnostic()?;

        ctx.info(&format!(
            "watching {} — Ctrl-C to stop",
            watch_dir.display()
        ));

        let mut published = 0usize;
        while let Ok(event) = rx.recv() {
            match event {
                Ok(events) => {
                    for ev in events {
                        if is_markdown(&ev.path)
                            && ev.path.is_file()
                            && publish_changed(ctx, &client, &ev.path).await
                        {
                            published += 1;
                            ctx.note(&format!("{published} published this session"));
                        }
                    }
                },
                Err(e) => ctx.warn(&format!("watch error: {e:?}")),
            }
        }
        Ok(())
    }
}

/// Whether `path` looks like a Markdown file.
fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Publish one changed file, reporting success or a friendly error. Returns
/// `true` when the file was published (so the caller can tally the session).
async fn publish_changed(ctx: &Ctx, client: &Ghost<ReqwestTransport>, path: &Path) -> bool {
    let opts = PublishOptions {
        state_path: Some(state_path(&ctx.settings.blog_dir)),
        ..PublishOptions::default()
    };
    match publish_file(client, path, &opts).await {
        Ok(outcome) => {
            ctx.success(&format!("{}: {}", path.display(), outcome_line(&outcome)));
            true
        },
        Err(e) => {
            crate::ctx::fail(&format!(
                "{}: {}",
                path.display(),
                to_report(Error::Core(e))
            ));
            false
        },
    }
}
