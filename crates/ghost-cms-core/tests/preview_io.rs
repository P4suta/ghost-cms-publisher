//! Failure propagation across the preview filesystem boundary.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ghost_cms_core::filesystem::{FileMetadata, FileSystem, Files, StateLease};
use ghost_cms_core::preparation::{PrepareOptions, PreparedCollection};

#[derive(Debug)]
struct FailedPreviewIdentity(Files);

impl FileSystem for FailedPreviewIdentity {
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        if path.file_name().is_some_and(|name| name == "index.html") {
            return Err(io::Error::other("injected preview identity failure"));
        }
        self.0.canonicalize(path)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.0.read(path)
    }

    fn read_text(&self, path: &Path) -> io::Result<String> {
        self.0.read_text(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        self.0.metadata(path)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        self.0.create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.0.create_dir_all(path)
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.write(path, bytes)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.0.remove_dir_all(path)
    }

    fn replace_state(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.replace_state(path, bytes)
    }

    fn lock_state(&self, path: &Path) -> io::Result<Box<dyn StateLease>> {
        self.0.lock_state(path)
    }
}

#[test]
fn final_preview_failure_removes_owned_output() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let post = temp.path().join("post.md");
    std::fs::write(
        &post,
        "---\ntitle: Preview\nslug: preview-failure\n---\nText.\n",
    )?;
    let collection = PreparedCollection::load(
        &[post],
        PrepareOptions::default(),
        Arc::new(FailedPreviewIdentity(
            ghost_cms_shared::filesystem::local_files(),
        )),
    )?;
    let output = temp.path().join("preview");
    let error = collection
        .write_preview(&output)
        .expect_err("injected failure must propagate");
    assert!(
        error
            .to_string()
            .contains("injected preview identity failure")
    );
    assert!(
        !output.exists(),
        "a failed preview must remove its owned directory"
    );
    Ok(())
}
