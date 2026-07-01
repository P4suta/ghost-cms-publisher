//! Runtime configuration, sourced from environment variables.

use crate::error::{CoreError, Result};

pub use crate::constants::{DEFAULT_ACCEPT_VERSION, MAX_PAYLOAD_BYTES};

/// Connection settings for a Ghost site.
#[derive(Debug, Clone)]
pub struct Config {
    /// Site origin, e.g. `https://example.ghost.io` (no trailing path).
    pub api_url: String,
    /// Staff Access Token in `{id}:{secret}` form.
    pub token: String,
    /// `Accept-Version` header value (e.g. `v5.0`).
    pub accept_version: String,
}

impl Config {
    /// Build a [`Config`] from explicit values.
    #[must_use]
    pub const fn new(api_url: String, token: String, accept_version: String) -> Self {
        Self {
            api_url,
            token,
            accept_version,
        }
    }

    /// Build a [`Config`] from the `GHOST_ADMIN_API_URL`, `GHOST_STAFF_TOKEN`
    /// and optional `GHOST_ACCEPT_VERSION` environment variables.
    ///
    /// # Errors
    /// Returns [`CoreError::Config`] if a required variable is unset.
    pub fn from_env() -> Result<Self> {
        let api_url = std::env::var("GHOST_ADMIN_API_URL")
            .map_err(|_| CoreError::Config("GHOST_ADMIN_API_URL is not set".to_owned()))?;
        let token = std::env::var("GHOST_STAFF_TOKEN")
            .map_err(|_| CoreError::Config("GHOST_STAFF_TOKEN is not set".to_owned()))?;
        let accept_version = std::env::var("GHOST_ACCEPT_VERSION")
            .unwrap_or_else(|_| DEFAULT_ACCEPT_VERSION.to_owned());
        Ok(Self {
            api_url,
            token,
            accept_version,
        })
    }
}
