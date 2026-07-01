//! `new` — scaffold a post file from a title.

use ghost_cms_shared::text::slugify;
use miette::IntoDiagnostic;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::editor::open_in_editor;

/// Scaffold a new post file under `<blog_dir>/posts/`.
#[derive(Debug, clap::Args)]
pub(crate) struct NewArgs {
    /// Post title.
    title: String,
    /// Slug override (default: derived from the title).
    #[arg(long)]
    slug: Option<String>,
    /// Tag (repeatable).
    #[arg(long = "tag")]
    tags: Vec<String>,
    /// Do not open the new file in `$EDITOR`.
    #[arg(long)]
    no_edit: bool,
}

/// Build the front-matter + body scaffold for a new post.
fn scaffold(title: &str, slug: &str, tags: &[String]) -> String {
    let tags_line = if tags.is_empty() {
        "[]".to_owned()
    } else {
        format!("[{}]", tags.join(", "))
    };
    format!(
        "---\ntitle: {title:?}\nslug: {slug}\nstatus: draft\ntags: {tags_line}\nexcerpt: \"\"\n---\n\n# {title}\n\nWrite your post here.\n"
    )
}

impl Command for NewArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let slug = self.slug.clone().unwrap_or_else(|| slugify(&self.title));
        let dir = ctx.settings.blog_dir.join("posts");
        std::fs::create_dir_all(&dir).into_diagnostic()?;
        let path = dir.join(format!("{slug}.md"));
        if path.exists() {
            return Err(miette::miette!(
                help = "Pick another slug with --slug, or edit the existing file.",
                "{} already exists",
                path.display()
            ));
        }

        std::fs::write(&path, scaffold(&self.title, &slug, &self.tags)).into_diagnostic()?;
        ctx.success(&format!("created {}", path.display()));

        if !self.no_edit {
            open_in_editor(&path)?;
        }
        Ok(())
    }
}
