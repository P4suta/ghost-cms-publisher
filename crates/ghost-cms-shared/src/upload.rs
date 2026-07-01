//! Upload orchestration: read a file, pick the endpoint, and call the client.

use std::path::Path;

use ghost_cms_core::Ghost;
use ghost_cms_core::transport::HttpTransport;

use crate::error::{Error, Result};
use crate::media::{UploadKind, content_type, image_mime, is_local};
use crate::paths::resolve;

/// Read a file, tagging failures with the path.
fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })
}

/// The file name of `path`, or `fallback` when it has none.
fn filename_of(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(fallback)
        .to_owned()
}

/// Upload `path` to the endpoint chosen by `kind` and return the CDN URL.
///
/// `kind` is resolved against the file extension when [`UploadKind::Auto`].
///
/// # Errors
/// Returns [`Error::Read`] if the file cannot be read, or a core error if the
/// upload fails.
pub async fn upload<T: HttpTransport>(
    client: &Ghost<T>,
    path: &Path,
    kind: UploadKind,
) -> Result<String> {
    let bytes = read(path)?;
    let filename = filename_of(path, "file");
    let content_type = content_type(path);
    let media = client.media();
    let url = match kind.resolve(path) {
        UploadKind::Image => media.upload_image(bytes, &filename, content_type).await?,
        UploadKind::Media => media.upload_media(bytes, &filename, content_type).await?,
        UploadKind::File | UploadKind::Auto => {
            media.upload_file(bytes, &filename, content_type).await?
        },
    };
    Ok(url)
}

/// Upload an image field if it is a local path (resolved against `base_dir`);
/// otherwise pass the value through unchanged.
///
/// # Errors
/// Returns [`Error::Read`] if a local file cannot be read, or a core error if
/// the upload fails.
pub async fn upload_if_local<T: HttpTransport>(
    client: &Ghost<T>,
    base_dir: &Path,
    value: Option<String>,
) -> Result<Option<String>> {
    match value {
        Some(p) if is_local(&p) => {
            let path = resolve(base_dir, &p);
            let bytes = read(&path)?;
            let filename = filename_of(&path, "image");
            let url = client
                .media()
                .upload_image(bytes, &filename, image_mime(&path))
                .await?;
            Ok(Some(url))
        },
        other => Ok(other),
    }
}
