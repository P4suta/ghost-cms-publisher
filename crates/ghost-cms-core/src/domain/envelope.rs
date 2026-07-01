//! Upload and error envelopes returned by Ghost.
//!
//! The Admin API wraps resources in a single-key envelope (`{"images":[…]}`)
//! and reports failures as `{"errors":[…]}`.

use serde::Deserialize;

/// One uploaded asset as returned by Ghost.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageInfo {
    /// CDN URL of the stored asset.
    pub url: String,
}

/// Response envelope for the image-upload endpoint.
#[derive(Debug, Deserialize)]
pub struct ImagesResponse {
    /// Returned images (one per upload).
    pub images: Vec<ImageInfo>,
}

/// Response envelope for the media-upload endpoint (`{"media":[…]}`).
#[derive(Debug, Deserialize)]
pub struct MediaResponse {
    /// Returned media (one per upload).
    pub media: Vec<ImageInfo>,
}

/// Response envelope for the file-upload endpoint (`{"files":[…]}`).
#[derive(Debug, Deserialize)]
pub struct FilesResponse {
    /// Returned files (one per upload).
    pub files: Vec<ImageInfo>,
}

/// One error item from Ghost's `{"errors":[…]}` body.
#[derive(Debug, Deserialize)]
pub struct GhostErrorItem {
    /// Human-readable message.
    pub message: String,
    /// Machine error type.
    #[serde(rename = "type", default)]
    pub error_type: Option<String>,
}

/// Error envelope returned by Ghost on failure.
#[derive(Debug, Deserialize)]
pub struct GhostErrors {
    /// One or more error items.
    pub errors: Vec<GhostErrorItem>,
}
