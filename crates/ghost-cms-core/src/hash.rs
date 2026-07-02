//! Content hashing and the local publish-state cache.
//!
//! The cache only lets `publish` skip re-uploading unchanged content; Ghost is
//! the source of truth, so a missing or corrupt cache just costs a `find_by_slug`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;

/// Hash an ordered set of string parts into a hex digest.
#[must_use]
pub(crate) fn content_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]); // domain separator between parts
    }
    hex::encode(hasher.finalize())
}

/// Recorded state for one published post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PostState {
    /// Ghost object id.
    pub(crate) id: String,
    /// Hash of the content last published.
    pub(crate) content_hash: String,
    /// Last `updated_at` observed from Ghost.
    pub(crate) updated_at: Option<String>,
}

/// The on-disk cache, keyed by slug.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct StateCache {
    /// Map of slug → recorded state.
    pub(crate) posts: BTreeMap<String, PostState>,
}

impl StateCache {
    /// Load the cache from `path`; a missing file yields an empty cache.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read or parsed.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Persist the cache, creating the parent directory if needed.
    ///
    /// # Errors
    /// Returns an error if the directory or file cannot be written.
    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}
