//! Idempotent create-or-update orchestration for a single post file.

mod execute;
mod plan;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::client::Ghost;
use crate::domain::{
    AuthorInput, CodeInjection, OpenGraph, PostInput, PostStatus, SeoMeta, TagInput, TwitterCard,
};
use crate::error::Result;
use crate::filesystem::Files;
use crate::frontmatter::FrontMatter;
use crate::images::ResolvedContent;
use crate::preparation::{PrepareOptions, PreparedCollection, PreparedPost};
use crate::transport::HttpTransport;

use self::plan::decide;

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
    /// Public Ghost route, defaulting to `/{slug}/`.
    pub route_template: Option<String>,
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
pub(crate) fn build_input(
    front: &FrontMatter,
    resolved: ResolvedContent,
    status: PostStatus,
) -> PostInput {
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
        feature_image_alt: front.feature_image_alt.clone(),
        feature_image_caption: front.feature_image_caption.clone(),
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
    files: Files,
) -> Result<PublishOutcome> {
    let collection = PreparedCollection::load(&[path.to_owned()], prepare_options(opts), files)?;
    let post = collection
        .posts()
        .first()
        .ok_or_else(|| crate::CoreError::FrontMatter("empty publication collection".to_owned()))?;
    publish_prepared(client, post, opts).await
}

/// Idempotently create-or-update a post from parsed front matter and a Markdown body.
///
/// Validates, renders GFM, looks up the slug, and decides a [`PlannedAction`].
/// A dry run reports the plan without writing; a real run uploads images and
/// executes with `updated_at` conflict detection.
///
/// # Errors
/// Propagates render, upload and API errors.
pub async fn publish_post<T: HttpTransport>(
    client: &Ghost<T>,
    front: FrontMatter,
    body_md: &str,
    base_dir: &Path,
    opts: &PublishOptions,
    files: Files,
) -> Result<PublishOutcome> {
    let options = prepare_options(opts);
    let mut post = PreparedPost::from_parts(
        base_dir.join("inline.md"),
        front,
        body_md.to_owned(),
        &options,
        files,
    )?;
    post.resolve(&BTreeMap::new(), &options)?;
    publish_prepared(client, &post, opts).await
}

/// Create the same local validation options for every publishing adapter.
#[must_use]
pub fn prepare_options(opts: &PublishOptions) -> PrepareOptions {
    PrepareOptions {
        allow_raw_html: opts.allow_raw_html,
        status_override: opts.status_override,
        route_template: opts
            .route_template
            .clone()
            .unwrap_or_else(|| "/{slug}/".to_owned()),
    }
}

/// Publish a locally validated post without reparsing its source file.
///
/// # Errors
/// Refuses remote edits, invalid receipts, changed media, and failed writes.
pub async fn publish_prepared<T: HttpTransport>(
    client: &Ghost<T>,
    post: &PreparedPost,
    opts: &PublishOptions,
) -> Result<PublishOutcome> {
    let _lease = if opts.dry_run {
        None
    } else {
        opts.state_path
            .as_deref()
            .map(|path| crate::hash::lock(path, post.files.as_ref()))
            .transpose()?
    };
    let mut cache = opts
        .state_path
        .as_deref()
        .map(|path| crate::hash::StateCache::load(path, post.files.as_ref()))
        .transpose()?
        .unwrap_or_default();
    let existing = client.posts().find_by_slug(&post.front.slug).await?;
    let receipt = cache
        .sites
        .get(client.cache_namespace())
        .and_then(|s| s.posts.get(&post.front.slug));
    let plan = decide(existing, &post.source_hash, receipt, opts.force)?;
    if opts.dry_run {
        return Ok(PublishOutcome::DryRun {
            action: plan.action(),
            slug: post.front.slug.clone(),
            html_bytes: post.html.len(),
        });
    }
    tracing::info!(slug = %post.front.slug, action = ?plan.action(), "publication plan ready");
    execute::execute(client, plan, post, &mut cache, opts).await
}
