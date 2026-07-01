//! Error types shared across the crate.
//!
//! Ghost Admin API failures are modeled as a [`CoreError::Api`] that pairs the
//! *what* (a [`Resource`] and an [`Operation`]) with a *why* (an [`ApiError`]
//! category derived from the HTTP status). This single classification is the one
//! source of truth both the CLI and the MCP server use to render good messages.

use thiserror::Error;

use crate::domain::GhostErrors;
use crate::transport::TransportError;

/// A convenient `Result` alias for fallible `ghost-core` operations.
pub type Result<T> = std::result::Result<T, CoreError>;

/// The Ghost resource an operation acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    /// A blog post.
    Post,
    /// A tag.
    Tag,
    /// An uploaded image.
    Image,
    /// Uploaded media (audio/video).
    Media,
    /// An uploaded arbitrary file.
    File,
    /// Site metadata.
    Site,
}

impl std::fmt::Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Post => "post",
            Self::Tag => "tag",
            Self::Image => "image",
            Self::Media => "media",
            Self::File => "file",
            Self::Site => "site",
        })
    }
}

/// The kind of action attempted against a [`Resource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Create a new resource.
    Create,
    /// Update an existing resource.
    Update,
    /// Read a resource.
    Fetch,
    /// Delete a resource.
    Delete,
    /// Upload a binary asset.
    Upload,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Fetch => "fetch",
            Self::Delete => "delete",
            Self::Upload => "upload",
        })
    }
}

/// A categorized Ghost Admin API failure, derived from the HTTP status.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApiError {
    /// The token was missing, malformed, or rejected (HTTP 401/403).
    #[error("unauthorized (HTTP {status}): {message}")]
    Unauthorized {
        /// The 401 or 403 status returned.
        status: u16,
        /// Ghost's first error message, when available.
        message: String,
    },

    /// The addressed resource does not exist (HTTP 404).
    #[error("not found: {message}")]
    NotFound {
        /// Ghost's first error message, when available.
        message: String,
    },

    /// A concurrent edit was detected via `updated_at` (HTTP 409).
    #[error("conflict: {message}")]
    Conflict {
        /// Ghost's first error message, when available.
        message: String,
    },

    /// The client is being throttled (HTTP 429).
    #[error("rate limited: {message}")]
    RateLimited {
        /// Ghost's first error message, when available.
        message: String,
    },

    /// The payload was rejected as invalid (HTTP 422 and similar).
    #[error("validation failed: {message}")]
    Validation {
        /// Ghost's first error message, when available.
        message: String,
        /// Ghost's machine error `type`, when present.
        ghost_type: Option<String>,
    },

    /// The request succeeded but the expected resource was absent from the body.
    #[error("the response contained no {0}")]
    Empty(Resource),

    /// Any other non-success status.
    #[error("unexpected ghost error (HTTP {status}): {message}")]
    Unexpected {
        /// HTTP status code returned by Ghost.
        status: u16,
        /// Human-readable message (Ghost's first error message when available).
        message: String,
        /// Ghost's machine error `type`, when present.
        ghost_type: Option<String>,
    },
}

/// Everything that can go wrong while talking to Ghost or preparing a post.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreError {
    /// The Staff Access Token was not `{id}:{secret}` or the secret was not hex.
    #[error("invalid staff token: {0}")]
    InvalidToken(String),

    /// Configuration (env vars, URL) was missing or malformed.
    #[error("configuration error: {0}")]
    Config(String),

    /// A categorized Ghost Admin API failure, tagged with the resource and
    /// operation it occurred on.
    #[error("failed to {operation} {resource}: {kind}")]
    Api {
        /// The resource the operation acted on.
        resource: Resource,
        /// The action attempted.
        operation: Operation,
        /// The categorized cause.
        #[source]
        kind: ApiError,
    },

    /// A response from Ghost was internally inconsistent (e.g. an existing post
    /// missing the `updated_at` needed to update it safely).
    #[error("inconsistent response from ghost: {0}")]
    Inconsistent(String),

    /// A post's front matter was missing required fields or invalid.
    #[error("frontmatter error: {0}")]
    FrontMatter(String),

    /// A post's front matter contained a YAML syntax error. Carries a byte
    /// offset into the original file content so callers can render a span.
    #[error("frontmatter syntax error at line {line}, column {column}: {message}")]
    FrontMatterSyntax {
        /// The underlying parser message.
        message: String,
        /// Byte offset of the error within the original file content.
        offset: usize,
        /// 1-based line number of the error.
        line: usize,
        /// 1-based column number of the error.
        column: usize,
    },

    /// The payload exceeded the Starter-plan 5 MB limit.
    #[error("payload too large: {size} bytes (limit {limit})")]
    TooLarge {
        /// Size of the offending payload in bytes.
        size: u64,
        /// The enforced limit in bytes.
        limit: u64,
    },

    /// Filesystem I/O failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The underlying HTTP transport failed.
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),

    /// JWT signing failed.
    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// (De)serialization of a JSON body failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Build a categorized [`CoreError::Api`] from a failing response body.
    ///
    /// This is the single place HTTP statuses are classified into [`ApiError`]
    /// categories, so every frontend renders consistent diagnostics.
    #[must_use]
    pub(crate) fn api(resource: Resource, operation: Operation, status: u16, bytes: &[u8]) -> Self {
        let (message, ghost_type) = parse_ghost_error(bytes);
        let kind = match status {
            401 | 403 => ApiError::Unauthorized { status, message },
            404 => ApiError::NotFound { message },
            409 => ApiError::Conflict { message },
            429 => ApiError::RateLimited { message },
            422 => ApiError::Validation {
                message,
                ghost_type,
            },
            _ => ApiError::Unexpected {
                status,
                message,
                ghost_type,
            },
        };
        Self::Api {
            resource,
            operation,
            kind,
        }
    }

    /// Build an "empty response" [`CoreError::Api`] for a request that returned
    /// no resource where one was expected.
    #[must_use]
    pub(crate) const fn empty(resource: Resource, operation: Operation) -> Self {
        Self::Api {
            resource,
            operation,
            kind: ApiError::Empty(resource),
        }
    }
}

/// Extract Ghost's first error message and machine type from an error body,
/// falling back to the raw bytes as a lossy string.
fn parse_ghost_error(bytes: &[u8]) -> (String, Option<String>) {
    match serde_json::from_slice::<GhostErrors>(bytes) {
        Ok(mut errs) if !errs.errors.is_empty() => {
            let first = errs.errors.swap_remove(0);
            (first.message, first.error_type)
        },
        _ => (String::from_utf8_lossy(bytes).into_owned(), None),
    }
}
