//! Shared metadata value types reused by posts and tags.
//!
//! The SEO/Open Graph/Twitter/code-injection blocks are `#[serde(flatten)]`ed
//! into the DTOs, keeping the flat wire shape Ghost expects.

use serde::{Deserialize, Serialize};

/// Post visibility tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Visible to everyone.
    Public,
    /// Visible to signed-in members.
    Members,
    /// Visible to paying members.
    Paid,
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Public => "public",
            Self::Members => "members",
            Self::Paid => "paid",
        })
    }
}

impl std::str::FromStr for Visibility {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Self::Public),
            "members" => Ok(Self::Members),
            "paid" => Ok(Self::Paid),
            _ => Err(()),
        }
    }
}

/// Tag visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagVisibility {
    /// A normal, publicly listed tag.
    Public,
    /// An internal tag (name starts with `#`), hidden from listings.
    Internal,
}

impl std::fmt::Display for TagVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Public => "public",
            Self::Internal => "internal",
        })
    }
}

impl std::str::FromStr for TagVisibility {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "public" => Ok(Self::Public),
            "internal" => Ok(Self::Internal),
            _ => Err(()),
        }
    }
}

/// SEO metadata common to posts and tags.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SeoMeta {
    /// SEO meta title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_title: Option<String>,
    /// SEO meta description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_description: Option<String>,
    /// Canonical URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
}

/// Open Graph (Facebook) card metadata.
#[derive(Debug, Clone, Default, Serialize)]
#[allow(
    clippy::struct_field_names,
    reason = "the og_ prefix matches Ghost's flat wire field names"
)]
pub struct OpenGraph {
    /// Open Graph image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub og_image: Option<String>,
    /// Open Graph title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub og_title: Option<String>,
    /// Open Graph description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub og_description: Option<String>,
}

/// Twitter card metadata.
#[derive(Debug, Clone, Default, Serialize)]
#[allow(
    clippy::struct_field_names,
    reason = "the twitter_ prefix matches Ghost's flat wire field names"
)]
pub struct TwitterCard {
    /// Twitter card image URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_image: Option<String>,
    /// Twitter card title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_title: Option<String>,
    /// Twitter card description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_description: Option<String>,
}

/// HTML injected into the rendered page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CodeInjection {
    /// HTML injected into `<head>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codeinjection_head: Option<String>,
    /// HTML injected before `</body>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codeinjection_foot: Option<String>,
}
