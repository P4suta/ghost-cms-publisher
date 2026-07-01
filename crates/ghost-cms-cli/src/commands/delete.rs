//! `delete` — remove a post by slug (with confirmation).

use ghost_cms_shared::error::require_post_by_slug;
use miette::IntoDiagnostic;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

/// Delete a post by slug, confirming first unless `--yes`.
#[derive(Debug, clap::Args)]
pub(crate) struct DeleteArgs {
    /// Post slug (omit to pick one interactively).
    slug: Option<String>,
    /// Skip the confirmation prompt.
    #[arg(long)]
    yes: bool,
}

impl Command for DeleteArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let slug = crate::pick::post_slug(ctx, &client, self.slug).await?;
        let post = require_post_by_slug(&client, &slug).await.friendly()?;

        if !self.yes {
            let confirmed = inquire::Confirm::new(&format!("Delete '{slug}' ({})?", post.id))
                .with_default(false)
                .prompt()
                .into_diagnostic()?;
            if !confirmed {
                ctx.note("aborted");
                return Ok(());
            }
        }

        let sp = ctx.spinner("deleting…");
        let res = client.posts().delete(&post.id).await.friendly();
        sp.finish_and_clear();
        res?;
        ctx.success(&format!("deleted {slug}"));
        Ok(())
    }
}
