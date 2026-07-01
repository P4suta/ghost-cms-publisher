//! Tool argument schemas (deserialized from MCP tool-call params).

use schemars::JsonSchema;
use serde::Deserialize;

/// Arguments for publishing a Markdown file.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PublishMarkdownArgs {
    /// Path to the post file (relative paths resolve against the blog directory).
    pub(crate) file: String,
    /// Plan only; perform no writes.
    #[serde(default)]
    pub(crate) dry_run: bool,
    /// Publish even if content is unchanged.
    #[serde(default)]
    pub(crate) force: bool,
    /// Optional status override: "draft", "published", or "scheduled".
    #[serde(default)]
    pub(crate) status: Option<String>,
}

/// Arguments for publishing inline Markdown (no file needed).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PublishInlineArgs {
    /// Post title.
    pub(crate) title: String,
    /// Stable slug (idempotency key).
    pub(crate) slug: String,
    /// Markdown body.
    pub(crate) markdown: String,
    /// Tag names.
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    /// Optional status: "draft" (default), "published", or "scheduled".
    #[serde(default)]
    pub(crate) status: Option<String>,
    /// Plan only; perform no writes.
    #[serde(default)]
    pub(crate) dry_run: bool,
}

/// Arguments addressing a post by slug.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SlugArgs {
    /// Post slug.
    pub(crate) slug: String,
}

/// Arguments for listing posts or tags.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListArgs {
    /// Maximum items to return.
    #[serde(default = "default_limit")]
    pub(crate) limit: u32,
    /// Page number (1-based).
    #[serde(default = "default_page")]
    pub(crate) page: u32,
}

/// Arguments for uploading an image.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UploadArgs {
    /// Path to the image (relative paths resolve against the blog directory).
    pub(crate) path: String,
}

/// Arguments for a generic upload.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UploadAnyArgs {
    /// Path to the file (relative paths resolve against the blog directory).
    pub(crate) path: String,
    /// Endpoint: "auto" (default), "image", "media", or "file".
    #[serde(default)]
    pub(crate) kind: Option<String>,
}

/// Arguments for upserting a tag's metadata (upsert by slug).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SetTagArgs {
    /// Tag slug (idempotency key).
    pub(crate) slug: String,
    /// Display name.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// Description.
    #[serde(default)]
    pub(crate) description: Option<String>,
    /// Feature image (local path is uploaded).
    #[serde(default)]
    pub(crate) feature_image: Option<String>,
    /// Accent color (hex).
    #[serde(default)]
    pub(crate) accent_color: Option<String>,
    /// Visibility (public/internal).
    #[serde(default)]
    pub(crate) visibility: Option<String>,
    /// Canonical URL.
    #[serde(default)]
    pub(crate) canonical_url: Option<String>,
    /// SEO meta title.
    #[serde(default)]
    pub(crate) meta_title: Option<String>,
    /// SEO meta description.
    #[serde(default)]
    pub(crate) meta_description: Option<String>,
    /// Open Graph image (local path is uploaded).
    #[serde(default)]
    pub(crate) og_image: Option<String>,
    /// Open Graph title.
    #[serde(default)]
    pub(crate) og_title: Option<String>,
    /// Open Graph description.
    #[serde(default)]
    pub(crate) og_description: Option<String>,
    /// Twitter image (local path is uploaded).
    #[serde(default)]
    pub(crate) twitter_image: Option<String>,
    /// Twitter title.
    #[serde(default)]
    pub(crate) twitter_title: Option<String>,
    /// Twitter description.
    #[serde(default)]
    pub(crate) twitter_description: Option<String>,
}

const fn default_limit() -> u32 {
    20
}

const fn default_page() -> u32 {
    1
}
