//! Presentation-neutral rendering: JSON shapes, detail blocks, table rows, and
//! one-line summaries, with no color or table dependency.

use std::path::Path;

use ghost_cms_core::domain::{Post, PostStatus, SiteInfo, Tag};
use ghost_cms_core::{PlannedAction, PublishOutcome};
use serde_json::{Value, json};

/// `"  <url>"` or empty.
fn url_suffix(url: Option<&str>) -> String {
    url.map(|u| format!("  {u}")).unwrap_or_default()
}

/// The verb for a planned dry-run action.
const fn plan_verb(action: PlannedAction) -> &'static str {
    match action {
        PlannedAction::Create => "create",
        PlannedAction::Update => "update",
        PlannedAction::Skip => "skip",
    }
}

/// The capitalized plan name used in JSON output.
const fn plan_name(action: PlannedAction) -> &'static str {
    match action {
        PlannedAction::Create => "Create",
        PlannedAction::Update => "Update",
        PlannedAction::Skip => "Skip",
    }
}

/// A one-line, plain-text summary of a publish outcome.
#[must_use]
pub fn outcome_line(outcome: &PublishOutcome) -> String {
    match outcome {
        PublishOutcome::Created { id, url } => {
            format!("created {id}{}", url_suffix(url.as_deref()))
        },
        PublishOutcome::Updated { id, url } => {
            format!("updated {id}{}", url_suffix(url.as_deref()))
        },
        PublishOutcome::SkippedUnchanged { id } => format!("unchanged {id} (skipped)"),
        PublishOutcome::DryRun {
            action,
            slug,
            html_bytes,
        } => format!(
            "[dry-run] would {} '{slug}' ({html_bytes} bytes of HTML)",
            plan_verb(*action)
        ),
    }
}

/// A JSON projection of a publish outcome, optionally tagged with the file.
#[must_use]
pub fn outcome_value(file: Option<&Path>, outcome: &PublishOutcome) -> Value {
    let file = file.map(|f| f.display().to_string());
    match outcome {
        PublishOutcome::Created { id, url } => {
            json!({ "file": file, "action": "created", "id": id, "url": url })
        },
        PublishOutcome::Updated { id, url } => {
            json!({ "file": file, "action": "updated", "id": id, "url": url })
        },
        PublishOutcome::SkippedUnchanged { id } => {
            json!({ "file": file, "action": "skipped", "id": id })
        },
        PublishOutcome::DryRun {
            action,
            slug,
            html_bytes,
        } => json!({
            "file": file,
            "action": "dry-run",
            "plan": plan_name(*action),
            "slug": slug,
            "html_bytes": html_bytes,
        }),
    }
}

/// Tallied counts across a batch of publish outcomes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PublishSummary {
    /// Posts created.
    pub created: usize,
    /// Posts updated.
    pub updated: usize,
    /// Posts skipped because their content was unchanged.
    pub skipped: usize,
    /// Posts that were only planned (`--dry-run`).
    pub dry_run: usize,
}

impl PublishSummary {
    /// Total outcomes tallied.
    #[must_use]
    pub const fn total(self) -> usize {
        self.created + self.updated + self.skipped + self.dry_run
    }
}

/// Tally a batch of publish outcomes into a [`PublishSummary`].
#[must_use]
pub fn summarize<'a, I>(outcomes: I) -> PublishSummary
where
    I: IntoIterator<Item = &'a PublishOutcome>,
{
    let mut summary = PublishSummary::default();
    for outcome in outcomes {
        match outcome {
            PublishOutcome::Created { .. } => summary.created += 1,
            PublishOutcome::Updated { .. } => summary.updated += 1,
            PublishOutcome::SkippedUnchanged { .. } => summary.skipped += 1,
            PublishOutcome::DryRun { .. } => summary.dry_run += 1,
        }
    }
    summary
}

/// A JSON projection of a post.
#[must_use]
pub fn post_value(post: &Post) -> Value {
    json!({
        "id": post.id,
        "slug": post.slug,
        "status": post.status,
        "title": post.title,
        "url": post.url,
        "updated_at": post.updated_at,
        "html": post.html,
    })
}

/// A JSON projection of a tag.
#[must_use]
pub fn tag_value(tag: &Tag) -> Value {
    json!({
        "id": tag.id,
        "name": tag.name,
        "slug": tag.slug,
        "description": tag.description,
        "visibility": tag.visibility,
        "accent_color": tag.accent_color,
        "feature_image": tag.feature_image,
        "url": tag.url,
        "posts": tag.count.as_ref().and_then(|c| c.posts),
    })
}

/// A JSON projection of site metadata.
#[must_use]
pub fn site_value(site: &SiteInfo) -> Value {
    json!({ "title": site.title, "url": site.url, "version": site.version })
}

/// Optional string or a `-` placeholder.
fn or_dash(value: Option<&str>) -> String {
    value.unwrap_or("-").to_owned()
}

/// Key/value detail rows for a single post.
#[must_use]
pub fn post_detail_fields(post: &Post) -> Vec<(&'static str, String)> {
    vec![
        ("id", post.id.clone()),
        ("slug", post.slug.clone()),
        ("status", post.status.to_string()),
        ("title", or_dash(post.title.as_deref())),
        ("url", or_dash(post.url.as_deref())),
        ("updated_at", or_dash(post.updated_at.as_deref())),
    ]
}

/// Key/value detail rows for a single tag.
#[must_use]
pub fn tag_detail_fields(tag: &Tag) -> Vec<(&'static str, String)> {
    vec![
        ("slug", tag.slug.clone()),
        ("name", tag.name.clone()),
        ("visibility", or_dash(tag.visibility.as_deref())),
        ("accent", or_dash(tag.accent_color.as_deref())),
        ("description", or_dash(tag.description.as_deref())),
        ("url", or_dash(tag.url.as_deref())),
    ]
}

/// A neutral table row for a post listing.
pub struct PostRow<'a> {
    /// Publication status (frontends color this).
    pub status: PostStatus,
    /// URL slug.
    pub slug: &'a str,
    /// Last-updated timestamp, or `-`.
    pub updated: &'a str,
    /// Title, or `(untitled)`.
    pub title: &'a str,
}

/// A neutral table row for a tag listing.
pub struct TagRow<'a> {
    /// URL slug.
    pub slug: &'a str,
    /// Display name.
    pub name: &'a str,
    /// Post count, or `-`.
    pub posts: String,
    /// Visibility, or `-`.
    pub visibility: &'a str,
}

/// Build neutral rows for a post listing.
#[must_use]
pub fn post_rows(posts: &[Post]) -> Vec<PostRow<'_>> {
    posts
        .iter()
        .map(|p| PostRow {
            status: p.status,
            slug: &p.slug,
            updated: p.updated_at.as_deref().unwrap_or("-"),
            title: p.title.as_deref().unwrap_or("(untitled)"),
        })
        .collect()
}

/// Build neutral rows for a tag listing.
#[must_use]
pub fn tag_rows(tags: &[Tag]) -> Vec<TagRow<'_>> {
    tags.iter()
        .map(|t| TagRow {
            slug: &t.slug,
            name: &t.name,
            posts: t
                .count
                .as_ref()
                .and_then(|c| c.posts)
                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
            visibility: t.visibility.as_deref().unwrap_or("-"),
        })
        .collect()
}
