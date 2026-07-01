//! `open` — open a post's URL in the browser.

use ghost_cms_shared::error::{Error, require_post_by_slug};
use miette::IntoDiagnostic;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

/// Open the post's public URL in the default browser.
#[derive(Debug, clap::Args)]
pub(crate) struct OpenArgs {
    /// Post slug (omit to pick one interactively).
    slug: Option<String>,
}

impl Command for OpenArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let slug = crate::pick::post_slug(ctx, &client, self.slug).await?;
        let post = require_post_by_slug(&client, &slug).await.friendly()?;
        let url = post.url.ok_or(Error::NoUrl { slug }).friendly()?;
        open::that(&url).into_diagnostic()?;
        ctx.success(&format!("opened {url}"));
        Ok(())
    }
}
