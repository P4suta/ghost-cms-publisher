//! Idempotent create-or-update orchestration for a single post file.

mod execute;
mod plan;

use std::path::{Path, PathBuf};

use crate::client::Ghost;
use crate::domain::{
    AuthorInput, CodeInjection, OpenGraph, PostInput, PostStatus, SeoMeta, TagInput, TwitterCard,
};
use crate::error::Result;
use crate::frontmatter::FrontMatter;
use crate::images::{ImageResolver, ResolvedContent};
use crate::markdown;
use crate::transport::HttpTransport;

use self::plan::{decide, load_cached_hash};

/// Options controlling a publish run.
#[derive(Debug, Clone, Default)]
pub struct PublishOptions {
    /// Plan only; perform no writes (uploads and create/update are skipped).
    pub dry_run: bool,
    /// Publish even when the content hash is unchanged.
    pub force: bool,
    /// Override the status declared in front matter.
    pub status_override: Option<PostStatus>,
    /// Pass raw HTML in the Markdown through unescaped.
    pub allow_raw_html: bool,
    /// Path to the JSON state cache (enables unchanged-skip).
    pub state_path: Option<PathBuf>,
}

/// What a dry run would do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedAction {
    /// No existing post with this slug — would create.
    Create,
    /// Existing post would be updated.
    Update,
    /// Content unchanged — would skip.
    Skip,
}

/// Outcome of [`publish_file`].
#[derive(Debug, Clone)]
pub enum PublishOutcome {
    /// A new post was created.
    Created {
        /// New post id.
        id: String,
        /// Public/preview URL.
        url: Option<String>,
    },
    /// An existing post was updated.
    Updated {
        /// Post id.
        id: String,
        /// Public/preview URL.
        url: Option<String>,
    },
    /// Content was unchanged; nothing was sent.
    SkippedUnchanged {
        /// Post id.
        id: String,
    },
    /// Dry-run plan.
    DryRun {
        /// What would happen.
        action: PlannedAction,
        /// Target slug.
        slug: String,
        /// Size of the rendered HTML body in bytes.
        html_bytes: usize,
    },
}

/// Assemble the API payload from front matter and resolved content.
fn build_input(front: &FrontMatter, resolved: ResolvedContent, status: PostStatus) -> PostInput {
    PostInput {
        title: front.title.clone(),
        html: Some(resolved.html),
        status,
        tags: front
            .tags
            .iter()
            .map(|t| TagInput { name: t.clone() })
            .collect(),
        feature_image: resolved.feature_image,
        custom_excerpt: front.excerpt.clone(),
        featured: front.featured,
        visibility: front.visibility,
        email_only: front.email_only,
        authors: front
            .authors
            .iter()
            .map(|email| AuthorInput {
                email: email.clone(),
            })
            .collect(),
        slug: Some(front.slug.clone()),
        published_at: front.published_at.clone(),
        updated_at: None,
        seo: SeoMeta {
            meta_title: front.meta_title.clone(),
            meta_description: front.meta_description.clone(),
            canonical_url: front.canonical_url.clone(),
        },
        open_graph: OpenGraph {
            og_image: resolved.og_image,
            og_title: front.og_title.clone(),
            og_description: front.og_description.clone(),
        },
        twitter: TwitterCard {
            twitter_image: resolved.twitter_image,
            twitter_title: front.twitter_title.clone(),
            twitter_description: front.twitter_description.clone(),
        },
        code_injection: CodeInjection {
            codeinjection_head: front.codeinjection_head.clone(),
            codeinjection_foot: front.codeinjection_foot.clone(),
        },
    }
}

/// Read, render, and idempotently create-or-update the post at `path`.
///
/// Convenience wrapper over [`publish_post`]: reads the file, parses the front
/// matter and body, and resolves images relative to the file's directory.
///
/// # Errors
/// Propagates parse, render, upload and API errors.
pub async fn publish_file<T: HttpTransport>(
    client: &Ghost<T>,
    path: &Path,
    opts: &PublishOptions,
) -> Result<PublishOutcome> {
    let raw = std::fs::read_to_string(path)?;
    let parsed = markdown::parse(&raw)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    publish_post(client, parsed.front, &parsed.body_md, base_dir, opts).await
}

/// Idempotently create-or-update a post from already-parsed front matter and a
/// Markdown body.
///
/// The flow is: validate, render GFM to HTML, look the slug up on Ghost, then
/// decide a [`Plan`] (create / update / skip). A dry run reports the plan
/// without uploading images or writing; a real run uploads local images, hashes
/// the payload, and executes the plan with `updated_at` conflict detection.
///
/// # Errors
/// Propagates render, upload and API errors.
pub async fn publish_post<T: HttpTransport>(
    client: &Ghost<T>,
    front: FrontMatter,
    body_md: &str,
    base_dir: &Path,
    opts: &PublishOptions,
) -> Result<PublishOutcome> {
    front.validate()?;
    let html_raw = markdown::render_html(body_md, opts.allow_raw_html);
    let html_bytes = html_raw.len();

    // In a dry run we keep local paths (no uploads); otherwise resolve images.
    let resolved = if opts.dry_run {
        ResolvedContent {
            html: html_raw,
            feature_image: front.feature_image.clone(),
            og_image: front.og_image.clone(),
            twitter_image: front.twitter_image.clone(),
        }
    } else {
        let mut resolver = ImageResolver::new(client, base_dir);
        ResolvedContent {
            html: resolver.html(&html_raw).await?,
            feature_image: resolver.field(front.feature_image.as_deref()).await?,
            og_image: resolver.field(front.og_image.as_deref()).await?,
            twitter_image: resolver.field(front.twitter_image.as_deref()).await?,
        }
    };

    let status = opts.status_override.unwrap_or(front.status);
    let input = build_input(&front, resolved, status);
    let content_hash = hash_input(&input)?;

    let existing = client.posts().find_by_slug(&front.slug).await?;
    let cached_hash = load_cached_hash(opts, &front.slug);
    let plan = decide(existing, &content_hash, cached_hash.as_deref(), opts.force)?;

    if opts.dry_run {
        return Ok(PublishOutcome::DryRun {
            action: plan.action(),
            slug: front.slug,
            html_bytes,
        });
    }

    execute::execute(client, plan, &input, &front.slug, content_hash, opts).await
}

/// Hash the serialized payload, used as the idempotency key.
fn hash_input(input: &PostInput) -> Result<String> {
    Ok(crate::hash::content_hash(&[&serde_json::to_string(input)?]))
}
