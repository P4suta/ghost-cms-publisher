//! Typed YAML front matter for `blog/posts/*.md`.

use std::path::PathBuf;

use serde::Deserialize;

use crate::domain::{PostStatus, Visibility};
use crate::error::{CoreError, Result};

/// The metadata block at the top of a post file.
///
/// Unknown keys are rejected so typos surface as errors rather than being
/// silently dropped.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontMatter {
    /// Post title (required).
    pub title: String,
    /// Stable slug used as the idempotency key (required).
    pub slug: String,
    /// Publication state; defaults to `draft`.
    #[serde(default)]
    pub status: PostStatus,
    /// Tag names.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Custom excerpt.
    #[serde(default)]
    pub excerpt: Option<String>,
    /// Feature image — a local path (uploaded on publish) or an absolute URL.
    #[serde(default)]
    pub feature_image: Option<String>,
    /// Canonical URL for SEO (often the original GitHub doc).
    #[serde(default)]
    pub canonical_url: Option<String>,
    /// SEO meta title.
    #[serde(default)]
    pub meta_title: Option<String>,
    /// SEO meta description.
    #[serde(default)]
    pub meta_description: Option<String>,
    /// Scheduled publication time (ISO 8601), required when `status: scheduled`.
    #[serde(default)]
    pub published_at: Option<String>,
    /// Whether the post is featured.
    #[serde(default)]
    pub featured: Option<bool>,
    /// Visibility: `public`, `members`, or `paid`.
    #[serde(default)]
    pub visibility: Option<Visibility>,
    /// Open Graph image — local path (uploaded) or absolute URL.
    #[serde(default)]
    pub og_image: Option<String>,
    /// Open Graph title.
    #[serde(default)]
    pub og_title: Option<String>,
    /// Open Graph description.
    #[serde(default)]
    pub og_description: Option<String>,
    /// Twitter card image — local path (uploaded) or absolute URL.
    #[serde(default)]
    pub twitter_image: Option<String>,
    /// Twitter card title.
    #[serde(default)]
    pub twitter_title: Option<String>,
    /// Twitter card description.
    #[serde(default)]
    pub twitter_description: Option<String>,
    /// HTML injected into the post's `<head>`.
    #[serde(default)]
    pub codeinjection_head: Option<String>,
    /// HTML injected before the post's closing `</body>`.
    #[serde(default)]
    pub codeinjection_foot: Option<String>,
    /// Send as email only (not published on the site).
    #[serde(default)]
    pub email_only: Option<bool>,
    /// Author emails (each must match an existing Ghost staff user).
    #[serde(default)]
    pub authors: Vec<String>,
    /// Optional provenance note: which upstream doc this was written from.
    /// Never read or modified by the tool — purely informational.
    #[serde(default)]
    pub source: Option<PathBuf>,
}

impl FrontMatter {
    /// Build front matter for a new post with the given title and slug,
    /// defaulting all other fields. Useful for inline (file-less) publishing.
    #[must_use]
    pub fn new(title: String, slug: String) -> Self {
        Self {
            title,
            slug,
            ..Self::default()
        }
    }

    /// Validate cross-field invariants.
    ///
    /// Visibility is validated during deserialization (an unknown value is a parse error).
    ///
    /// # Errors
    /// Returns [`CoreError::FrontMatter`] when `title`/`slug` are blank, or a
    /// `scheduled` post is missing `published_at`.
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(CoreError::FrontMatter("title must not be empty".to_owned()));
        }
        if self.slug.trim().is_empty() {
            return Err(CoreError::FrontMatter("slug must not be empty".to_owned()));
        }
        if self.status == PostStatus::Scheduled && self.published_at.is_none() {
            return Err(CoreError::FrontMatter(
                "status: scheduled requires published_at".to_owned(),
            ));
        }
        Ok(())
    }
}
