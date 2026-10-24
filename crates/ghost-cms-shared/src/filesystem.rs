//! Native filesystem adapter for local preparation and publication.

mod video;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ghost_cms_core::filesystem::{FileMetadata, FileSystem, Files, StateLease};
use ghost_cms_core::preparation::{PrepareOptions, PreparedCollection};
use ghost_cms_core::publish::{PublishOptions, PublishOutcome};
use ghost_cms_core::transport::HttpTransport;
use ghost_cms_core::{Ghost, Result};

#[derive(Debug)]
struct LocalFiles;

struct LocalLease {
    _file: std::fs::File,
}

impl StateLease for LocalLease {}

impl FileSystem for LocalFiles {
    fn prepare_video(&self, path: &Path) -> Result<ghost_cms_core::video::VideoMetadata> {
        video::prepare(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn read_text(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        let metadata = std::fs::metadata(path)?;
        Ok(FileMetadata {
            regular: metadata.is_file(),
            bytes: metadata.len(),
        })
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        std::fs::write(path, bytes)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn replace_state(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
        Ok(())
    }

    fn lock_state(&self, path: &Path) -> io::Result<Box<dyn StateLease>> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("json.lock"))?;
        file.try_lock().map_err(io::Error::other)?;
        Ok(Box::new(LocalLease { _file: file }))
    }
}

/// Native file operations without reading environment configuration.
#[must_use]
pub fn local_files() -> Files {
    Arc::new(LocalFiles)
}

/// Prepare a collection using native local files.
/// # Errors
/// Returns all preparation failures together.
pub fn prepare_collection(
    paths: &[PathBuf],
    options: PrepareOptions,
) -> Result<PreparedCollection> {
    PreparedCollection::load(paths, options, local_files())
}

/// Publish a local file through the shared preparation and state boundary.
/// # Errors
/// Returns preparation, persistence, upload, or API errors.
pub async fn publish_file<T: HttpTransport>(
    client: &Ghost<T>,
    path: &Path,
    options: &PublishOptions,
) -> Result<PublishOutcome> {
    ghost_cms_core::publish::publish_file(client, path, options, local_files()).await
}
