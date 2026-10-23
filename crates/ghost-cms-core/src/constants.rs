//! Ghost Admin API protocol constants.
//!
//! Fixed protocol facts and plan limits.
//! User-tunable settings live in [`crate::config`].

use std::time::Duration;

pub(crate) const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Relative path of the Admin API under a site origin.
pub(crate) const ADMIN_API_PATH: &str = "/ghost/api/admin/";

/// Header Ghost uses to negotiate the Admin API version.
pub(crate) const ACCEPT_VERSION_HEADER: &str = "Accept-Version";

/// Default Admin API version sent in the `Accept-Version` header.
pub const DEFAULT_ACCEPT_VERSION: &str = "v5.0";

/// Conservative client ceiling for Starter-compatible content and files.
pub const MAX_PAYLOAD_BYTES: u64 = 5_000_000;

/// JWT lifetime in seconds.
/// Ghost rejects Admin API tokens valid for longer than five minutes.
pub(crate) const JWT_TTL_SECS: u64 = 5 * 60;

/// Fixed audience claim for Admin API JWTs.
pub(crate) const ADMIN_AUDIENCE: &str = "/admin/";
