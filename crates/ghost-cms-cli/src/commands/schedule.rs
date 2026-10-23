//! Reviewable collection schedules with durable recovery state.

use std::path::PathBuf;

use ghost_cms_core::domain::PostStatus;
use ghost_cms_core::preparation::PrepareOptions;
use ghost_cms_core::schedule::{
    ReleaseReport, apply_release_plan, create_release_plan, stage_drafts, verify_release_plan,
};
use ghost_cms_shared::filesystem::{local_files, prepare_collection};
use ghost_cms_shared::paths::expand_inputs;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

#[derive(Debug, clap::Args)]
pub(crate) struct ScheduleArgs {
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, clap::Subcommand)]
enum Action {
    /// Create or resume a collection of drafts after checking every slug.
    Drafts {
        /// Markdown files or directories containing the complete collection.
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
        /// Durable draft receipt and publication cache location.
        #[arg(long)]
        receipt: PathBuf,
        /// Exact public site URL expected from the authenticated Admin API.
        #[arg(long)]
        site_url: String,
        /// Require a cover, alternative text, and caption on every post.
        #[arg(long)]
        require_feature_images: bool,
    },
    /// Freeze verified drafts into ten-minute slots starting at a future whole hour.
    Plan {
        /// Completed receipt from schedule drafts.
        #[arg(long)]
        drafts: PathBuf,
        /// JSON array of slug groups in publication order, with simultaneous posts in one group.
        #[arg(long)]
        order: PathBuf,
        /// New JSON plan file, also used to record progress during apply.
        #[arg(long)]
        output: PathBuf,
    },
    /// Register the schedule or resume its unfinished entries.
    Apply {
        /// Frozen plan file from schedule plan.
        #[arg(long)]
        plan: PathBuf,
    },
    /// Read back all scheduled or published posts without changing Ghost.
    Verify {
        /// Frozen plan file from schedule plan.
        #[arg(long)]
        plan: PathBuf,
    },
}

fn now() -> miette::Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| miette::miette!("invalid system clock: {e}"))
}

fn emit(ctx: &Ctx, report: &ReleaseReport) -> miette::Result<()> {
    if ctx.json {
        crate::output::print_json(
            &serde_json::to_value(report)
                .map_err(|e| miette::miette!("cannot serialize release report: {e}"))?,
        );
    } else {
        let scheduled = report
            .posts
            .iter()
            .filter(|p| p.status == PostStatus::Scheduled)
            .count();
        let published = report
            .posts
            .iter()
            .filter(|p| p.status == PostStatus::Published)
            .count();
        ctx.success(&format!(
            "Read back {} posts: {scheduled} scheduled, {published} published.",
            report.posts.len()
        ));
        if let (Some(first), Some(last)) = (report.posts.first(), report.posts.last()) {
            ctx.note(&format!(
                "Release window: {} to {} (UTC).",
                first.published_at, last.published_at
            ));
        }
    }
    Ok(())
}

impl Command for ScheduleArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        match self.action {
            Action::Drafts {
                files,
                receipt,
                site_url,
                require_feature_images,
            } => {
                let paths = expand_inputs(&files).friendly()?;
                let collection =
                    prepare_collection(&paths, PrepareOptions::default()).friendly()?;
                let count = stage_drafts(
                    &ctx.client()?,
                    &collection,
                    &receipt,
                    &site_url,
                    require_feature_images,
                )
                .await
                .friendly()?;
                if ctx.json {
                    crate::output::print_json(
                        &serde_json::json!({"drafts":count, "receipt":receipt}),
                    );
                } else {
                    ctx.success(&format!("Prepared and read back {count} drafts."));
                }
                Ok(())
            },
            Action::Plan {
                drafts,
                order,
                output,
            } => {
                let bytes = std::fs::read(&order)
                    .map_err(|e| miette::miette!("cannot read release order: {e}"))?;
                let groups: Vec<Vec<String>> = serde_json::from_slice(&bytes)
                    .map_err(|e| miette::miette!("invalid release order: {e}"))?;
                let report = create_release_plan(
                    &ctx.client()?,
                    &drafts,
                    &groups,
                    &output,
                    now()?,
                    local_files(),
                )
                .await
                .friendly()?;
                emit(ctx, &report)
            },
            Action::Apply { plan } => {
                let report = apply_release_plan(&ctx.client()?, &plan, now()?, local_files())
                    .await
                    .friendly()?;
                emit(ctx, &report)
            },
            Action::Verify { plan } => {
                let report = verify_release_plan(&ctx.client()?, &plan, local_files())
                    .await
                    .friendly()?;
                emit(ctx, &report)
            },
        }
    }
}
