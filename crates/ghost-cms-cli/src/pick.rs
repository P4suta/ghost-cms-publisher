//! Resolve an optional slug argument, falling back to an interactive fuzzy picker.
//!
//! When the slug is omitted and the session is interactive, fetch candidates and
//! fuzzy-pick; otherwise return a "slug required" error so scripts fail predictably.

use std::io::IsTerminal;

use ghost_cms_core::Ghost;
use ghost_cms_core::transport::ReqwestTransport;
use miette::IntoDiagnostic;

use crate::ctx::Ctx;
use crate::error::Friendly;

/// Upper bound on how many candidates the picker fetches.
const CANDIDATE_LIMIT: u32 = 100;

/// Whether we may launch an interactive prompt: a real terminal on both stdin and
/// stderr (where inquire draws), and not in `--json` mode.
fn is_interactive(ctx: &Ctx) -> bool {
    !ctx.json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Require an interactive session, or explain how to supply the slug directly.
fn require_interactive(ctx: &Ctx) -> miette::Result<()> {
    if is_interactive(ctx) {
        Ok(())
    } else {
        Err(miette::miette!(
            help = "Pass a slug (e.g. `ghost-cms get my-post`), or run in an interactive terminal.",
            "a slug is required"
        ))
    }
}

/// Present a fuzzy picker over `candidates`, returning the chosen slug.
fn choose(prompt: &str, mut candidates: Vec<String>) -> miette::Result<String> {
    if candidates.is_empty() {
        return Err(miette::miette!("nothing to choose from"));
    }
    candidates.sort();
    inquire::Select::new(prompt, candidates)
        .prompt()
        .into_diagnostic()
}

/// Resolve a post slug: the provided value, else an interactive pick over recent posts.
pub(crate) async fn post_slug(
    ctx: &Ctx,
    client: &Ghost<ReqwestTransport>,
    provided: Option<String>,
) -> miette::Result<String> {
    if let Some(slug) = provided {
        return Ok(slug);
    }
    require_interactive(ctx)?;
    let posts = client.posts().list(CANDIDATE_LIMIT, 1).await.friendly()?;
    choose("Select a post", posts.into_iter().map(|p| p.slug).collect())
}

/// Resolve a tag slug: the provided value, else an interactive pick over tags.
pub(crate) async fn tag_slug(
    ctx: &Ctx,
    client: &Ghost<ReqwestTransport>,
    provided: Option<String>,
) -> miette::Result<String> {
    if let Some(slug) = provided {
        return Ok(slug);
    }
    require_interactive(ctx)?;
    let tags = client.tags().list(CANDIDATE_LIMIT, 1).await.friendly()?;
    choose("Select a tag", tags.into_iter().map(|t| t.slug).collect())
}

/// Resolve a local post slug (for `edit`): the provided value, else a pick over
/// the slugs of on-disk post files.
pub(crate) fn local_post_slug(ctx: &Ctx, provided: Option<String>) -> miette::Result<String> {
    if let Some(slug) = provided {
        return Ok(slug);
    }
    require_interactive(ctx)?;
    let slugs = ghost_cms_shared::paths::local_post_slugs(&ctx.settings.blog_dir).friendly()?;
    choose("Select a post", slugs)
}
