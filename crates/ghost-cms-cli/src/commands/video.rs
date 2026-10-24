//! Native video metadata audits and repairs.

use std::path::PathBuf;

use ghost_cms_core::video::{repair_owned_videos, verify_repair};
use ghost_cms_shared::filesystem::local_files;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

#[derive(Debug, clap::Args)]
pub(crate) struct VideoArgs {
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, clap::Subcommand)]
enum Action {
    /// Plan metadata changes without remote writes, or apply them explicitly.
    Repair {
        /// Existing ownership receipt from schedule drafts.
        #[arg(long)]
        receipt: PathBuf,
        /// New private report with backups and per-post progress.
        #[arg(long)]
        report: PathBuf,
        /// Upload frame thumbnails and update only native video metadata.
        #[arg(long)]
        apply: bool,
    },
    /// Read back a completed repair without changing Ghost.
    Verify {
        /// Completed applied repair report.
        #[arg(long)]
        report: PathBuf,
    },
}

impl Command for VideoArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let report = match self.action {
            Action::Repair {
                receipt,
                report,
                apply,
            } => repair_owned_videos(&client, &receipt, &report, apply, local_files())
                .await
                .friendly()?,
            Action::Verify { report } => verify_repair(&client, &report, local_files())
                .await
                .friendly()?,
        };
        if ctx.json {
            crate::output::print_json(&serde_json::to_value(&report).map_err(|error| {
                miette::miette!("cannot serialize video repair report: {error}")
            })?);
        } else {
            let outcome = if report.applied() {
                "posts changed and verified"
            } else {
                "posts need metadata changes"
            };
            ctx.success(&format!(
                "Inspected {} posts and {} video cards; {} {outcome}.",
                report.posts(),
                report.videos(),
                report.changed()
            ));
        }
        Ok(())
    }
}
