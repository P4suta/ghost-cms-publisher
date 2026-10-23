//! `new` — scaffold a post file from a title.

use std::io::Write as _;

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
fn scaffold(title: &str, slug: &str, tags: &[String]) -> serde_json::Result<String> {
    let title_yaml = serde_json::to_string(title)?;
    let tags_yaml = serde_json::to_string(tags)?;
    Ok(format!(
        "---\ntitle: {title_yaml}\nslug: {slug}\nstatus: draft\ntags: {tags_yaml}\nexcerpt: \"\"\n---\n\nWrite your post here.\n"
    ))
}

impl Command for NewArgs {
    fn run(self, ctx: &Ctx) -> impl std::future::Future<Output = miette::Result<()>> {
        std::future::ready(self.create(ctx))
    }
}

impl NewArgs {
    fn create(self, ctx: &Ctx) -> miette::Result<()> {
        let slug = self.slug.clone().unwrap_or_else(|| slugify(&self.title));
        ghost_cms_core::frontmatter::FrontMatter::new(self.title.clone(), slug.clone())
            .validate()
            .into_diagnostic()?;
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

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .into_diagnostic()?;
        file.write_all(
            scaffold(&self.title, &slug, &self.tags)
                .into_diagnostic()?
                .as_bytes(),
        )
        .into_diagnostic()?;
        ctx.success(&format!("created {}", path.display()));

        if !self.no_edit {
            open_in_editor(&path)?;
        }
        Ok(())
    }
}
