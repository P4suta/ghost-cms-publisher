//! `tags` — list, inspect, upsert, and delete tags.

use std::path::Path;

use ghost_cms_core::Ghost;
use ghost_cms_core::transport::ReqwestTransport;
use ghost_cms_shared::error::require_tag_by_slug;
use ghost_cms_shared::tag::{TagMeta, build_upsert};
use ghost_cms_shared::upload::upload_if_local;
use miette::IntoDiagnostic;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;
use crate::output::{TagList, TagView};

/// Manage tags (list, get, set metadata, delete).
#[derive(Debug, clap::Args)]
pub(crate) struct TagsArgs {
    #[command(subcommand)]
    cmd: TagCmd,
}

/// `tags` subcommands.
#[derive(Debug, clap::Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "clap arg structs are sized inline; #[command(flatten)] forbids boxing the field"
)]
enum TagCmd {
    /// List tags with post counts.
    List {
        /// Maximum tags to list.
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Page number (1-based).
        #[arg(long, default_value_t = 1)]
        page: u32,
    },
    /// Show one tag by slug.
    Get {
        /// Tag slug (omit to pick one interactively).
        slug: Option<String>,
    },
    /// Create or update a tag's metadata (upsert by slug).
    Set {
        /// Tag slug (idempotency key; omit to pick one interactively).
        slug: Option<String>,
        #[command(flatten)]
        meta: TagSetArgs,
    },
    /// Delete a tag by slug.
    Delete {
        /// Tag slug (omit to pick one interactively).
        slug: Option<String>,
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
}

/// Optional tag metadata flags (image fields accept local paths or URLs).
#[derive(Debug, clap::Args)]
struct TagSetArgs {
    /// Display name.
    #[arg(long)]
    name: Option<String>,
    /// Description.
    #[arg(long)]
    description: Option<String>,
    /// Feature image (local path is uploaded).
    #[arg(long)]
    feature_image: Option<String>,
    /// Accent color (hex, e.g. `#7C3AED`).
    #[arg(long)]
    accent_color: Option<String>,
    /// Visibility (`public`/`internal`).
    #[arg(long)]
    visibility: Option<String>,
    /// Canonical URL.
    #[arg(long)]
    canonical_url: Option<String>,
    /// SEO meta title.
    #[arg(long)]
    meta_title: Option<String>,
    /// SEO meta description.
    #[arg(long)]
    meta_description: Option<String>,
    /// Open Graph image (local path is uploaded).
    #[arg(long)]
    og_image: Option<String>,
    /// Open Graph title.
    #[arg(long)]
    og_title: Option<String>,
    /// Open Graph description.
    #[arg(long)]
    og_description: Option<String>,
    /// Twitter image (local path is uploaded).
    #[arg(long)]
    twitter_image: Option<String>,
    /// Twitter title.
    #[arg(long)]
    twitter_title: Option<String>,
    /// Twitter description.
    #[arg(long)]
    twitter_description: Option<String>,
    /// HTML injected into the tag archive `<head>`.
    #[arg(long)]
    codeinjection_head: Option<String>,
    /// HTML injected before the tag archive `</body>`.
    #[arg(long)]
    codeinjection_foot: Option<String>,
}

impl From<TagSetArgs> for TagMeta {
    fn from(a: TagSetArgs) -> Self {
        Self {
            name: a.name,
            description: a.description,
            feature_image: a.feature_image,
            accent_color: a.accent_color,
            visibility: a.visibility,
            canonical_url: a.canonical_url,
            meta_title: a.meta_title,
            meta_description: a.meta_description,
            og_image: a.og_image,
            og_title: a.og_title,
            og_description: a.og_description,
            twitter_image: a.twitter_image,
            twitter_title: a.twitter_title,
            twitter_description: a.twitter_description,
            codeinjection_head: a.codeinjection_head,
            codeinjection_foot: a.codeinjection_foot,
        }
    }
}

impl Command for TagsArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        match self.cmd {
            TagCmd::List { limit, page } => list(ctx, limit, page).await,
            TagCmd::Get { slug } => get(ctx, slug).await,
            TagCmd::Set { slug, meta } => set(ctx, slug, meta).await,
            TagCmd::Delete { slug, yes } => delete(ctx, slug, yes).await,
        }
    }
}

async fn list(ctx: &Ctx, limit: u32, page: u32) -> miette::Result<()> {
    let client = ctx.client()?;
    let sp = ctx.spinner("loading tags…");
    let tags = client.tags().list(limit, page).await.friendly();
    sp.finish_and_clear();
    ctx.emit(&TagList(&tags?));
    Ok(())
}

async fn get(ctx: &Ctx, slug: Option<String>) -> miette::Result<()> {
    let client = ctx.client()?;
    let slug = crate::pick::tag_slug(ctx, &client, slug).await?;
    let tag = require_tag_by_slug(&client, &slug).await.friendly()?;
    ctx.emit(&TagView(&tag));
    Ok(())
}

async fn set(ctx: &Ctx, slug: Option<String>, meta: TagSetArgs) -> miette::Result<()> {
    let client = ctx.client()?;
    let slug = crate::pick::tag_slug(ctx, &client, slug).await?;
    let existing = client.tags().find_by_slug(&slug).await.friendly()?;

    // Local images upload against the current directory.
    let base = Path::new(".");
    let mut meta: TagMeta = meta.into();
    meta.feature_image = upload_if_local(&client, base, meta.feature_image)
        .await
        .friendly()?;
    meta.og_image = upload_if_local(&client, base, meta.og_image)
        .await
        .friendly()?;
    meta.twitter_image = upload_if_local(&client, base, meta.twitter_image)
        .await
        .friendly()?;

    let input = build_upsert(&slug, meta, existing.as_ref().map(|t| t.name.as_str())).friendly()?;

    let sp = ctx.spinner("saving tag…");
    let result = upsert(&client, existing.as_ref(), &input).await;
    sp.finish_and_clear();
    let tag = result?;

    let verb = if existing.is_some() {
        "updated"
    } else {
        "created"
    };
    ctx.success(&format!("{verb} tag '{}'", tag.slug));
    Ok(())
}

/// Create or update a tag, depending on whether it already exists.
async fn upsert(
    client: &Ghost<ReqwestTransport>,
    existing: Option<&ghost_cms_core::domain::Tag>,
    input: &ghost_cms_core::domain::TagUpsertInput,
) -> miette::Result<ghost_cms_core::domain::Tag> {
    match existing {
        Some(tag) => {
            let updated_at = tag
                .updated_at
                .clone()
                .ok_or_else(|| miette::miette!("existing tag is missing updated_at"))?;
            client.tags().update(&tag.id, input, &updated_at).await
        },
        None => client.tags().create(input).await,
    }
    .friendly()
}

async fn delete(ctx: &Ctx, slug: Option<String>, yes: bool) -> miette::Result<()> {
    let client = ctx.client()?;
    let slug = crate::pick::tag_slug(ctx, &client, slug).await?;
    let tag = require_tag_by_slug(&client, &slug).await.friendly()?;
    if !yes {
        let confirmed = inquire::Confirm::new(&format!("Delete tag '{slug}' ({})?", tag.id))
            .with_default(false)
            .prompt()
            .into_diagnostic()?;
        if !confirmed {
            ctx.note("aborted");
            return Ok(());
        }
    }
    client.tags().delete(&tag.id).await.friendly()?;
    ctx.success(&format!("deleted tag '{slug}'"));
    Ok(())
}
