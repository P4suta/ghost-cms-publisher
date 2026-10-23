//! Content identities and destination-scoped publication receipts.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};
use crate::filesystem::{FileSystem, StateLease};

#[derive(Debug)]
pub(crate) struct ContentDigest(String);

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn content_hash(parts: &[&str]) -> ContentDigest {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    ContentDigest(hex::encode(hasher.finalize()))
}

pub(crate) fn bytes_hash(bytes: &[u8]) -> ContentDigest {
    ContentDigest(hex::encode(Sha256::digest(bytes)))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PostState {
    pub(crate) id: String,
    pub(crate) content_hash: String,
    pub(crate) updated_at: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SiteState {
    #[serde(default)]
    pub(crate) posts: BTreeMap<String, PostState>,
    #[serde(default)]
    pub(crate) assets: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateCache {
    version: u8,
    pub(crate) sites: BTreeMap<String, SiteState>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    unscoped_legacy: BTreeMap<String, PostState>,
}

impl Default for StateCache {
    fn default() -> Self {
        Self {
            version: 2,
            sites: BTreeMap::new(),
            unscoped_legacy: BTreeMap::new(),
        }
    }
}

impl StateCache {
    pub(crate) fn load(path: &Path, files: &dyn FileSystem) -> Result<Self> {
        let bytes = match files.read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            },
            Err(error) => return Err(error.into()),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        if value.get("version").is_none() {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Legacy {
                posts: BTreeMap<String, PostState>,
            }
            let legacy: Legacy = serde_json::from_value(value)?;
            return Ok(Self {
                unscoped_legacy: legacy.posts,
                ..Self::default()
            });
        }
        let cache: Self = serde_json::from_value(value)?;
        if cache.version != 2 {
            return Err(CoreError::Config(
                "unsupported publish state version".to_owned(),
            ));
        }
        Ok(cache)
    }

    pub(crate) fn save(&self, path: &Path, files: &dyn FileSystem) -> Result<()> {
        files.replace_state(path, &serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

pub(crate) fn lock(path: &Path, files: &dyn FileSystem) -> Result<Box<dyn StateLease>> {
    files
        .lock_state(path)
        .map_err(|e| CoreError::Config(format!("cannot acquire publication state lock: {e}")))
}
