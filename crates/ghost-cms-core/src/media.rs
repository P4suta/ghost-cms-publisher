//! File-type helpers: MIME guessing and local-vs-remote `src` detection.
//!
//! These live in `core` so the image resolver can use them, and are re-exported
//! by `ghost-cms-shared` so the CLI and MCP frontends share one MIME table.

use std::path::Path;

/// Lowercased file extension of `path`, if any.
#[must_use]
pub fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

/// Whether a `src`/path is a local file rather than a remote or inline URL.
#[must_use]
pub fn is_local(src: &str) -> bool {
    !(src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("//")
        || src.starts_with("data:"))
}

/// Guess an image MIME type from a file extension, defaulting to `image/png`.
///
/// Used for image uploads, where the asset is assumed to be an image.
#[must_use]
pub fn image_mime(path: &Path) -> &'static str {
    match extension(path).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "image/png",
    }
}

/// Guess a content type from a file extension, defaulting to
/// `application/octet-stream`.
///
/// Used for generic uploads (images, media, and arbitrary files).
#[must_use]
pub fn content_type(path: &Path) -> &'static str {
    match extension(path).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("flac") => "audio/flac",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{content_type, image_mime, is_local};

    #[test]
    fn local_vs_remote() {
        assert!(is_local("assets/x.png"));
        assert!(!is_local("https://cdn/x.png"));
        assert!(!is_local("data:image/png;base64,AAAA"));
    }

    #[test]
    fn image_mime_defaults_to_png() {
        assert_eq!(image_mime(Path::new("a.jpg")), "image/jpeg");
        assert_eq!(image_mime(Path::new("a.unknown")), "image/png");
    }

    #[test]
    fn content_type_defaults_to_octet_stream() {
        assert_eq!(content_type(Path::new("a.mp4")), "video/mp4");
        assert_eq!(content_type(Path::new("a.bin")), "application/octet-stream");
    }
}
