//! Post domain types and the post request/response DTOs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::meta::{CodeInjection, OpenGraph, SeoMeta, TwitterCard, Visibility};

/// Publication state of a post.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostStatus {
    /// Not visible; an unpublished draft.
    #[default]
    Draft,
    /// Publicly visible.
    Published,
    /// Set to publish at `published_at`.
    Scheduled,
    /// A status Ghost reported that this tool does not model.
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for PostStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Scheduled => "scheduled",
            Self::Unknown => "unknown",
        })
    }
}

impl std::str::FromStr for PostStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            "scheduled" => Ok(Self::Scheduled),
            "unknown" => Ok(Self::Unknown),
            _ => Err(()),
        }
    }
}

/// A tag reference sent when creating/updating a post (Ghost resolves or
/// creates the tag by name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInput {
    /// Tag display name.
    pub name: String,
}

/// An author reference sent on a post (Ghost resolves the user by email).
#[derive(Debug, Clone, Serialize)]
pub struct AuthorInput {
    /// Author's email — must match an existing Ghost staff user.
    pub email: String,
}

/// The post fields ghost-cms-publisher sends to Ghost.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PostInput {
    /// Post title (the only field Ghost strictly requires).
    pub title: String,
    /// Rendered HTML body (sent with `?source=html`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// Desired publication state.
    pub status: PostStatus,
    /// Tags by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagInput>,
    /// Feature image URL (already uploaded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_image: Option<String>,
    /// Accessible feature image description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_image_alt: Option<String>,
    /// Feature image caption and attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_image_caption: Option<String>,
    /// Custom excerpt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_excerpt: Option<String>,
    /// Whether the post is featured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub featured: Option<bool>,
    /// Visibility tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    /// Whether the post is sent only as email (not published on the site).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_only: Option<bool>,
    /// Authors (resolved by email).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<AuthorInput>,
    /// Desired slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Scheduled publication time (ISO 8601), for `scheduled` posts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Last-known `updated_at`, required on update for conflict detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// SEO metadata (flattened on the wire).
    #[serde(flatten)]
    pub seo: SeoMeta,
    /// Open Graph metadata (flattened on the wire).
    #[serde(flatten)]
    pub open_graph: OpenGraph,
    /// Twitter card metadata (flattened on the wire).
    #[serde(flatten)]
    pub twitter: TwitterCard,
    /// Code injection (flattened on the wire).
    #[serde(flatten)]
    pub code_injection: CodeInjection,
}

/// A post as returned by Ghost.
#[derive(Debug, Clone, Deserialize)]
pub struct Post {
    /// Ghost object id.
    pub id: String,
    /// Title, if present.
    #[serde(default)]
    pub title: Option<String>,
    /// URL slug.
    pub slug: String,
    /// Publication status.
    #[serde(default)]
    pub status: PostStatus,
    /// Public or preview URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Server-side last-modified timestamp (needed to update).
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Rendered HTML, present when `formats=html` was requested.
    #[serde(default)]
    pub html: Option<String>,
    /// Serialized editor document returned by Ghost.
    #[serde(default)]
    pub lexical: Option<String>,
    /// Assigned tags, projected to their display names.
    #[serde(default)]
    pub tags: Vec<TagInput>,
    /// Feature image URL returned by Ghost.
    #[serde(default)]
    pub feature_image: Option<String>,
    /// Accessible feature image description.
    #[serde(default)]
    pub feature_image_alt: Option<String>,
    /// Feature image caption and attribution.
    #[serde(default)]
    pub feature_image_caption: Option<String>,
    /// Actual or scheduled publication time.
    #[serde(default)]
    pub published_at: Option<String>,
}

#[derive(Deserialize)]
struct EditorDocument {
    root: EditorRoot,
}

#[derive(Deserialize)]
struct EditorRoot {
    #[serde(rename = "type")]
    kind: String,
    children: Vec<EditorBlock>,
}

#[derive(Deserialize)]
struct EditorBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    children: Vec<Self>,
}

impl EditorBlock {
    fn native_content(&self) -> bool {
        match self.kind.as_str() {
            "html" | "linebreak" => false,
            "text" | "extended-text" => !self.text.trim().is_empty(),
            "paragraph" | "heading" | "quote" | "list" | "listitem" | "link" | "autolink" => {
                !self.text.trim().is_empty() || self.children.iter().any(Self::native_content)
            },
            _ => !self.kind.is_empty(),
        }
    }
}

impl Post {
    /// Count HTML cards and nonempty native blocks in the saved Ghost editor document.
    /// # Errors
    /// Refuses missing, malformed, or empty editor documents.
    pub fn editor_blocks(&self) -> crate::Result<BTreeMap<String, usize>> {
        let invalid = || {
            crate::CoreError::Inconsistent(format!(
                "post {} has no valid editor document",
                self.slug
            ))
        };
        let document: EditorDocument =
            serde_json::from_str(self.lexical.as_deref().ok_or_else(invalid)?)?;
        crate::video::editor_videos(self.lexical.as_deref().ok_or_else(invalid)?)?;
        if document.root.kind != "root" || document.root.children.is_empty() {
            return Err(invalid());
        }
        let mut counts = BTreeMap::new();
        for block in document.root.children {
            if block.kind.is_empty() {
                return Err(invalid());
            }
            if block.kind != "html" && !block.native_content() {
                continue;
            }
            *counts.entry(block.kind).or_insert(0) += 1;
        }
        if counts.is_empty() {
            return Err(invalid());
        }
        Ok(counts)
    }
}

/// Request envelope for the `posts` resource.
#[derive(Debug, Serialize)]
pub struct PostsRequest {
    /// One-element vector — Ghost takes a batch but the tool sends one.
    pub posts: Vec<PostInput>,
}

/// Response envelope for the `posts` resource.
#[derive(Debug, Deserialize)]
pub struct PostsResponse {
    /// Returned posts.
    pub posts: Vec<Post>,
}
