//! `edit` — open the local file for a slug in `$EDITOR`.

use ghost_cms_shared::paths::find_post_file;
use owo_colors::Stream;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::editor::open_in_editor;
use crate::error::Friendly;

/// Open the local post file matching `slug` in the user's editor.
#[derive(Debug, clap::Args)]
pub(crate) struct EditArgs {
    /// Post slug (omit to pick one interactively).
    slug: Option<String>,
}

impl Command for EditArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let slug = crate::pick::local_post_slug(ctx, self.slug)?;
        let path = find_post_file(&ctx.settings.blog_dir, &slug).friendly()?;
        ctx.note(&format!(
            "opening {}",
            crate::ui::path(Stream::Stderr, &path.display().to_string())
        ));
        open_in_editor(&path)
    }
}
