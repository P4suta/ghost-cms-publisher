//! `publish` — create or update posts from Markdown files (idempotent by slug).

use std::path::{Path, PathBuf};

use ghost_cms_core::CoreError;
use ghost_cms_core::Ghost;
use ghost_cms_core::domain::PostStatus;
use ghost_cms_core::publish::{PublishOptions, PublishOutcome, publish_file};
use ghost_cms_core::transport::ReqwestTransport;
use ghost_cms_shared::error::Error;
use ghost_cms_shared::paths::{expand_inputs, state_path};
use ghost_cms_shared::render::summarize;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::{Friendly, frontmatter_report, to_report};
use crate::output::{OutcomeView, print_publish_summary};

/// Create or update one or more posts from Markdown files.
#[allow(
    clippy::struct_excessive_bools,
    reason = "a CLI flag bundle mirrors the clap arguments verbatim"
)]
#[derive(Debug, clap::Args)]
pub(crate) struct PublishArgs {
    /// Post Markdown files (one or more).
    #[arg(required = true, num_args = 1.., value_hint = clap::ValueHint::FilePath)]
    files: Vec<PathBuf>,
    /// Plan only; perform no writes.
    #[arg(long)]
    dry_run: bool,
    /// Publish even if the content is unchanged.
    #[arg(long)]
    force: bool,
    /// Force status to published.
    #[arg(long, conflicts_with = "draft")]
    publish: bool,
    /// Force status to draft.
    #[arg(long)]
    draft: bool,
    /// Pass raw HTML in the Markdown through unescaped.
    #[arg(long)]
    allow_raw_html: bool,
    /// Disable the local publish-state cache.
    #[arg(long)]
    no_cache: bool,
    /// Open each published post in the browser afterwards.
    #[arg(long)]
    open: bool,
}

impl PublishArgs {
    /// The status override implied by `--publish`/`--draft`.
    const fn status_override(&self) -> Option<PostStatus> {
        if self.publish {
            Some(PostStatus::Published)
        } else if self.draft {
            Some(PostStatus::Draft)
        } else {
            None
        }
    }
}

impl Command for PublishArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let status_override = self.status_override();
        let expanded = expand_inputs(&self.files).friendly()?;
        if expanded.is_empty() {
            return Err(miette::miette!("no Markdown files to publish"));
        }
        let total = expanded.len();
        let mut outcomes = Vec::with_capacity(total);
        for (idx, file) in expanded.iter().enumerate() {
            let outcome = self
                .publish_one(ctx, &client, file, status_override, idx + 1, total)
                .await?;
            outcomes.push(outcome);
        }
        // A batch deserves a closing tally; a single file already says it all.
        if total > 1 && !ctx.json {
            print_publish_summary(ctx, summarize(&outcomes));
        }
        Ok(())
    }
}

impl PublishArgs {
    async fn publish_one(
        &self,
        ctx: &Ctx,
        client: &Ghost<ReqwestTransport>,
        file: &Path,
        status_override: Option<PostStatus>,
        index: usize,
        total: usize,
    ) -> miette::Result<PublishOutcome> {
        let opts = PublishOptions {
            dry_run: self.dry_run,
            force: self.force,
            status_override,
            allow_raw_html: self.allow_raw_html,
            state_path: (!self.no_cache).then(|| state_path(&ctx.settings.blog_dir)),
        };

        let label = if total > 1 {
            format!("[{index}/{total}] publishing {}…", file.display())
        } else {
            format!("publishing {}…", file.display())
        };
        let sp = ctx.spinner(&label);
        let result = publish_file(client, file, &opts).await;
        sp.finish_and_clear();

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(CoreError::FrontMatterSyntax {
                offset, message, ..
            }) => return Err(frontmatter_report(file, &message, offset)),
            Err(e) => return Err(to_report(Error::Core(e))),
        };

        ctx.emit(&OutcomeView {
            file: Some(file),
            outcome: &outcome,
        });
        if self.open {
            open_outcome_url(&outcome);
        }
        Ok(outcome)
    }
}

/// Open the published post's URL, if it has one.
fn open_outcome_url(outcome: &PublishOutcome) {
    let url = match outcome {
        PublishOutcome::Created { url, .. } | PublishOutcome::Updated { url, .. } => url.as_deref(),
        PublishOutcome::SkippedUnchanged { .. } | PublishOutcome::DryRun { .. } => None,
    };
    if let Some(u) = url {
        let _ = open::that(u);
    }
}
