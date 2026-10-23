//! Offline collection checks and browser previews.

use std::path::PathBuf;

use ghost_cms_core::CoreError;
use ghost_cms_core::preparation::{CheckReport, PrepareOptions, PreparedCollection};
use ghost_cms_shared::filesystem::prepare_collection;
use ghost_cms_shared::paths::expand_inputs;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::{Friendly, to_report};

#[derive(Debug, clap::Args)]
pub(crate) struct LocalArgs {
    /// Markdown files or directories containing posts.
    #[arg(required = true, num_args = 1.., value_hint = clap::ValueHint::FilePath)]
    files: Vec<PathBuf>,
    /// Permit explicit HTML in the source.
    #[arg(long)]
    allow_raw_html: bool,
    /// Public Ghost route, containing one {slug} placeholder.
    #[arg(long, default_value = "/{slug}/")]
    route_template: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CheckArgs {
    #[command(flatten)]
    input: LocalArgs,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PreviewArgs {
    #[command(flatten)]
    input: LocalArgs,
    /// New output directory for HTML and copied media.
    #[arg(long, value_hint = clap::ValueHint::DirPath)]
    output: PathBuf,
}

impl LocalArgs {
    fn prepare(self, ctx: &Ctx) -> miette::Result<PreparedCollection> {
        let paths = expand_inputs(&self.files).friendly()?;
        match prepare_collection(
            &paths,
            PrepareOptions {
                allow_raw_html: self.allow_raw_html,
                route_template: self.route_template,
                ..PrepareOptions::default()
            },
        ) {
            Ok(collection) => Ok(collection),
            Err(CoreError::Preparation(error)) => {
                if ctx.json {
                    emit_report(ctx, &error.report)?;
                }
                Err(to_report(CoreError::Preparation(error).into()))
            },
            Err(error) => Err(to_report(error.into())),
        }
    }
}

fn emit_report(ctx: &Ctx, report: &CheckReport) -> miette::Result<()> {
    if ctx.json {
        crate::output::print_json(
            &serde_json::to_value(report)
                .map_err(|e| miette::miette!("cannot serialize check report: {e}"))?,
        );
    } else {
        let media = report.posts.iter().map(|p| p.local_assets).sum::<usize>();
        ctx.success(&format!(
            "Validated {} posts and {media} local media references without network access.",
            report.posts.len()
        ));
        let captions = report
            .posts
            .iter()
            .map(|p| p.rendering.media_captions)
            .sum::<usize>();
        let missing = report
            .posts
            .iter()
            .map(|p| p.rendering.uncaptioned_media)
            .sum::<usize>();
        let cards = report
            .posts
            .iter()
            .map(|p| p.rendering.link_cards)
            .sum::<usize>();
        ctx.note(&format!("Rendered {captions} media captions and {cards} link cards; {missing} media figures have no caption."));
        let external = report
            .posts
            .iter()
            .flat_map(|p| p.external_references.iter())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if external > 0 {
            ctx.note(&format!(
                "{external} external URLs depend on their original hosts."
            ));
        }
    }
    Ok(())
}

impl Command for CheckArgs {
    fn run(self, ctx: &Ctx) -> impl std::future::Future<Output = miette::Result<()>> {
        std::future::ready(
            self.input
                .prepare(ctx)
                .and_then(|collection| emit_report(ctx, &collection.report())),
        )
    }
}

impl Command for PreviewArgs {
    fn run(self, ctx: &Ctx) -> impl std::future::Future<Output = miette::Result<()>> {
        std::future::ready((|| {
            let collection = self.input.prepare(ctx)?;
            let index = collection
                .write_preview(&self.output)
                .map_err(|e| to_report(e.into()))?;
            if ctx.json {
                crate::output::print_json(
                    &serde_json::json!({"index": index, "report": collection.report()}),
                );
            } else {
                emit_report(ctx, &collection.report())?;
                ctx.success(&format!("Preview: {}", index.display()));
            }
            Ok(())
        })())
    }
}
