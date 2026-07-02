//! Ghost Admin API protocol constants.
//!
//! Fixed protocol facts and plan limits, not user-tunable settings (those live
//! in [`crate::config`]).

/// Relative path of the Admin API under a site origin.
pub(crate) const ADMIN_API_PATH: &str = "/ghost/api/admin/";

/// Header Ghost uses to negotiate the Admin API version.
pub(crate) const ACCEPT_VERSION_HEADER: &str = "Accept-Version";

/// Default Admin API version sent in the `Accept-Version` header.
pub const DEFAULT_ACCEPT_VERSION: &str = "v5.0";

/// Maximum request body size accepted by the Ghost Pro Starter plan (5 MB).
pub const MAX_PAYLOAD_BYTES: u64 = 5 * 1024 * 1024;

/// JWT lifetime in seconds. Ghost rejects Admin API tokens valid for longer
/// than 5 minutes.
pub(crate) const JWT_TTL_SECS: u64 = 5 * 60;

/// Fixed audience claim for Admin API JWTs.
pub(crate) const ADMIN_AUDIENCE: &str = "/admin/";
