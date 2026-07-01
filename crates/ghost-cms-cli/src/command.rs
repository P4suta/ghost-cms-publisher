//! The uniform command interface every subcommand implements.

use crate::ctx::Ctx;

/// A runnable subcommand.
///
/// Each subcommand is a `clap::Args` struct that owns its parsed fields and
/// renders its own result, so dispatch in `main` is one uniform match.
pub(crate) trait Command {
    /// Run the command against the shared context.
    async fn run(self, ctx: &Ctx) -> miette::Result<()>;
}
