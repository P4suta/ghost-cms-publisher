//! `get` — fetch a single post by slug.

use ghost_cms_shared::error::require_post_by_slug;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;
use crate::output::PostView;

/// Fetch one post by slug and print it.
#[derive(Debug, clap::Args)]
pub(crate) struct GetArgs {
    /// Post slug (omit to pick one interactively).
    slug: Option<String>,
    /// Open the post in the browser.
    #[arg(long)]
    open: bool,
}

impl Command for GetArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let slug = crate::pick::post_slug(ctx, &client, self.slug).await?;
        let post = require_post_by_slug(&client, &slug).await.friendly()?;
        if self.open
            && let Some(url) = &post.url
        {
            let _ = open::that(url);
        }
        ctx.emit(&PostView(&post));
        Ok(())
    }
}
