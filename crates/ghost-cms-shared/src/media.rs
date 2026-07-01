//! File-type helpers shared by the frontends.
//!
//! The MIME tables and local-vs-remote detection live in `ghost-cms-core`; this
//! module re-exports them and adds the frontend-facing [`UploadKind`].

use std::path::Path;

pub use ghost_cms_core::media::{content_type, extension, image_mime, is_local};

/// Which upload endpoint to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UploadKind {
    /// Route by file extension (image → images, audio/video → media, else files).
    #[default]
    Auto,
    /// Force the images endpoint.
    Image,
    /// Force the media endpoint (audio/video).
    Media,
    /// Force the files endpoint (any file).
    File,
}

impl UploadKind {
    /// Parse a kind string (`auto`/`image`/`media`/`file`), defaulting to
    /// [`UploadKind::Auto`] for anything unrecognized.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "image" => Self::Image,
            "media" => Self::Media,
            "file" => Self::File,
            _ => Self::Auto,
        }
    }

    /// Resolve to a concrete endpoint, detecting from the path when [`Auto`].
    ///
    /// [`Auto`]: UploadKind::Auto
    #[must_use]
    pub fn resolve(self, path: &Path) -> Self {
        match self {
            Self::Auto => detect(path),
            other => other,
        }
    }
}

/// Pick a concrete endpoint from a file extension.
#[must_use]
pub fn detect(path: &Path) -> UploadKind {
    match extension(path).as_deref() {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico") => UploadKind::Image,
        Some("mp4" | "mov" | "webm" | "m4v" | "mkv" | "mp3" | "wav" | "ogg" | "m4a" | "flac") => {
            UploadKind::Media
        },
        _ => UploadKind::File,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{UploadKind, detect};

    #[test]
    fn detect_routes_by_extension() {
        assert_eq!(detect(Path::new("a.png")), UploadKind::Image);
        assert_eq!(detect(Path::new("a.mp4")), UploadKind::Media);
        assert_eq!(detect(Path::new("a.bin")), UploadKind::File);
    }

    #[test]
    fn parse_defaults_to_auto() {
        assert_eq!(UploadKind::parse("wat"), UploadKind::Auto);
        assert_eq!(UploadKind::parse("media"), UploadKind::Media);
    }

    #[test]
    fn auto_resolves_via_detect() {
        assert_eq!(
            UploadKind::Auto.resolve(Path::new("a.mp3")),
            UploadKind::Media
        );
        assert_eq!(
            UploadKind::Image.resolve(Path::new("a.mp3")),
            UploadKind::Image
        );
    }
}
