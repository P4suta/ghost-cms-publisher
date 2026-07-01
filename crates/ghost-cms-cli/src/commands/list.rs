//! `list` — show recent posts as a table (or JSON).

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;
use crate::output::PostList;

/// List recent posts.
#[derive(Debug, clap::Args)]
pub(crate) struct ListArgs {
    /// Maximum posts to list.
    #[arg(long, default_value_t = 20)]
    limit: u32,
    /// Page number (1-based).
    #[arg(long, default_value_t = 1)]
    page: u32,
}

impl Command for ListArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let sp = ctx.spinner("loading posts…");
        let posts = client.posts().list(self.limit, self.page).await.friendly();
        sp.finish_and_clear();
        ctx.emit(&PostList(&posts?));
        Ok(())
    }
}
