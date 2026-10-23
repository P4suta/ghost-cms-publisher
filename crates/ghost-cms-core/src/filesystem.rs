//! Filesystem facts and operations supplied by application adapters.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Shared filesystem capability for one prepared collection.
pub type Files = Arc<dyn FileSystem>;

/// File properties needed by preparation.
#[derive(Debug, Clone, Copy)]
pub struct FileMetadata {
    /// Whether the target is a regular file.
    pub regular: bool,
    /// Content length in bytes.
    pub bytes: u64,
}

/// Exclusive publication-state ownership, released on drop.
pub trait StateLease: Send {}

/// Local I/O boundary shared by preparation, preview, and publication.
pub trait FileSystem: std::fmt::Debug + Send + Sync {
    /// Resolve a path to its canonical identity.
    /// # Errors
    /// Returns path resolution errors.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    /// Read a file's bytes.
    /// # Errors
    /// Returns file read errors.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Read a UTF-8 source file.
    /// # Errors
    /// Returns read or encoding errors.
    fn read_text(&self, path: &Path) -> io::Result<String>;
    /// Inspect the referenced file.
    /// # Errors
    /// Returns metadata errors.
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata>;
    /// Create a new directory without replacing an existing target.
    /// # Errors
    /// Returns creation errors, including an existing target.
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    /// Create missing directory ancestors.
    /// # Errors
    /// Returns directory creation errors.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    /// Write bytes inside an owned output directory.
    /// # Errors
    /// Returns file write errors.
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Remove an owned incomplete output directory.
    /// # Errors
    /// Returns cleanup errors.
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    /// Atomically replace a state file after syncing its new bytes.
    /// # Errors
    /// Returns creation, write, sync, or replacement errors.
    fn replace_state(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Acquire an exclusive state lease without waiting.
    /// # Errors
    /// Returns creation errors or an occupied lease.
    fn lock_state(&self, path: &Path) -> io::Result<Box<dyn StateLease>>;
}
