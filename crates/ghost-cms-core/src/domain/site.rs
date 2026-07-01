//! Site metadata types.

use serde::Deserialize;

/// Public site metadata (used by `whoami`).
#[derive(Debug, Clone, Deserialize)]
pub struct SiteInfo {
    /// Site title.
    pub title: String,
    /// Site URL.
    pub url: String,
    /// Ghost version string, if exposed.
    #[serde(default)]
    pub version: Option<String>,
}

/// Response envelope for the `site` resource.
#[derive(Debug, Deserialize)]
pub struct SiteResponse {
    /// Site metadata.
    pub site: SiteInfo,
}
