//! Tag domain types and the tag request/response DTOs.

use serde::{Deserialize, Serialize};

use super::meta::{CodeInjection, OpenGraph, SeoMeta, TagVisibility, TwitterCard};

/// Post-count sub-object returned for a tag when `include=count.posts`.
#[derive(Debug, Clone, Deserialize)]
pub struct TagCount {
    /// Number of posts using the tag.
    #[serde(default)]
    pub posts: Option<u64>,
}

/// A tag as returned by Ghost.
#[derive(Debug, Clone, Deserialize)]
pub struct Tag {
    /// Ghost object id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// URL slug.
    pub slug: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Feature image URL.
    #[serde(default)]
    pub feature_image: Option<String>,
    /// Visibility (`public` or `internal`).
    #[serde(default)]
    pub visibility: Option<String>,
    /// Accent color (hex).
    #[serde(default)]
    pub accent_color: Option<String>,
    /// Public URL of the tag archive.
    #[serde(default)]
    pub url: Option<String>,
    /// Server-side last-modified timestamp (needed to update).
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Counts, present when requested with `include=count.posts`.
    #[serde(default)]
    pub count: Option<TagCount>,
}

/// Tag fields sent when creating or updating a tag.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TagUpsertInput {
    /// Display name (required).
    pub name: String,
    /// Slug override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Feature image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_image: Option<String>,
    /// Visibility (`public`/`internal`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<TagVisibility>,
    /// Accent color (hex, e.g. `#7C3AED`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<String>,
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

/// Request envelope for the `tags` resource.
#[derive(Debug, Serialize)]
pub struct TagsRequest {
    /// One-element vector — Ghost takes a batch but the tool sends one.
    pub tags: Vec<TagUpsertInput>,
}

/// Response envelope for the `tags` resource.
#[derive(Debug, Deserialize)]
pub struct TagsResponse {
    /// Returned tags.
    pub tags: Vec<Tag>,
}
