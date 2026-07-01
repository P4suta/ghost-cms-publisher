//! A unified frontend error type and its diagnosis.
//!
//! Both frontends share one [`Error`] type and one [`diagnose`] mapping, so a
//! 401 / 404 / 409 / oversized-payload / bad-frontmatter failure produces the
//! same summary and the same remediation category everywhere. Each frontend
//! then renders that category in its own idiom (miette help text, MCP error
//! codes).

use ghost_cms_core::domain::{Post, Tag};
use ghost_cms_core::error::ApiError;
use ghost_cms_core::transport::HttpTransport;
use ghost_cms_core::{CoreError, Ghost};

use crate::config::ConfigError;
use crate::text::nearest as nearest_slug;

/// How many existing slugs to scan when suggesting a "did you mean" alternative.
const SUGGEST_LIMIT: u32 = 100;

/// A frontend-level error: a core failure, a config problem, or an I/O or
/// lookup failure that happens above the core library.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A failure from the core library (API, transport, frontmatter, …).
    #[error(transparent)]
    Core(#[from] CoreError),

    /// Configuration was missing or unusable.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// A file could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The path that could not be read.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// A directory could not be read.
    #[error("cannot read directory {path}: {source}")]
    ReadDir {
        /// The directory that could not be read.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// No resource matched the requested slug.
    #[error("no {resource} with slug `{slug}`")]
    NotFound {
        /// The resource kind (`post` or `tag`).
        resource: &'static str,
        /// The slug that was looked up.
        slug: String,
        /// The closest existing slug, when the input looks like a typo of it.
        suggestion: Option<String>,
    },

    /// A post exists but has no public URL yet.
    #[error("post `{slug}` has no public URL yet")]
    NoUrl {
        /// The post's slug.
        slug: String,
    },

    /// A user-supplied value was not valid for its field.
    #[error("invalid {field}: `{value}`")]
    InvalidValue {
        /// The field name.
        field: &'static str,
        /// The offending value.
        value: String,
    },
}

/// A convenient `Result` alias for fallible frontend operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The suggested next step for an error, rendered per-frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remediation {
    /// Nothing actionable.
    None,
    /// Configure the site URL.
    ConfigureSite,
    /// Set the Staff Access Token.
    SetToken,
    /// Reduce the payload size.
    ReducePayload,
    /// Check the network / site URL.
    CheckNetwork,
    /// Fix the post's front matter.
    FixFrontmatter,
    /// List posts to find a valid slug.
    ListPosts,
    /// List tags to find a valid slug.
    ListTags,
    /// Correct an invalid input value.
    FixInput,
}

impl Remediation {
    /// A generic one-line hint, or `None` when there is nothing to suggest.
    #[must_use]
    pub const fn hint(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::ConfigureSite => Some("Run `ghost-cms init` (or set GHOST_ADMIN_API_URL)."),
            Self::SetToken => Some("Run `ghost-cms login` (or set GHOST_STAFF_TOKEN)."),
            Self::ReducePayload => {
                Some("Ghost's Starter plan caps requests at 5 MB; split or shrink the content.")
            },
            Self::CheckNetwork => Some("Check the site URL and your network connection."),
            Self::FixFrontmatter => Some("Fix the YAML in the `---` block and try again."),
            Self::ListPosts => Some("Run `ghost-cms list` to see available slugs."),
            Self::ListTags => Some("Run `ghost-cms tags list` to see available slugs."),
            Self::FixInput => Some("Check the value and try again."),
        }
    }
}

/// A presentation-neutral diagnosis of an [`Error`].
#[derive(Debug, Clone)]
pub struct Diagnosis {
    /// A human-readable one-line summary of what went wrong.
    pub summary: String,
    /// The suggested next step.
    pub remediation: Remediation,
    /// Whether this is the caller's fault (bad input/config) rather than an
    /// internal failure — used by MCP to pick `invalid_params` vs `internal`.
    pub user_error: bool,
}

/// Classify an [`Error`] into a summary plus a remediation category.
#[must_use]
pub fn diagnose(error: &Error) -> Diagnosis {
    let (remediation, user_error) = match error {
        Error::Config(ConfigError::MissingSiteUrl) | Error::Core(CoreError::Config(_)) => {
            (Remediation::ConfigureSite, true)
        },
        Error::Config(ConfigError::MissingToken) | Error::Core(CoreError::InvalidToken(_)) => {
            (Remediation::SetToken, true)
        },
        Error::Core(CoreError::Api { kind, .. }) => api_remediation(kind),
        Error::Core(CoreError::TooLarge { .. }) => (Remediation::ReducePayload, true),
        Error::Core(CoreError::Transport(_)) => (Remediation::CheckNetwork, false),
        Error::Core(CoreError::FrontMatter(_) | CoreError::FrontMatterSyntax { .. }) => {
            (Remediation::FixFrontmatter, true)
        },
        Error::NotFound {
            resource: "tag", ..
        } => (Remediation::ListTags, true),
        Error::NotFound { .. } => (Remediation::ListPosts, true),
        Error::InvalidValue { .. } => (Remediation::FixInput, true),
        Error::NoUrl { .. } => (Remediation::None, true),
        _ => (Remediation::None, false),
    };
    Diagnosis {
        summary: summarize(error),
        remediation,
        user_error,
    }
}

/// The one-line summary for an error, enriched with a "did you mean" hint when a
/// near-miss slug was found.
fn summarize(error: &Error) -> String {
    match error {
        Error::NotFound {
            suggestion: Some(suggestion),
            ..
        } => format!("{error} — did you mean `{suggestion}`?"),
        _ => error.to_string(),
    }
}

/// Map an [`ApiError`] category to a remediation and fault attribution.
const fn api_remediation(kind: &ApiError) -> (Remediation, bool) {
    match kind {
        ApiError::Unauthorized { .. } => (Remediation::SetToken, true),
        ApiError::NotFound { .. } => (Remediation::None, true),
        ApiError::Validation { .. } => (Remediation::FixInput, true),
        ApiError::RateLimited { .. } => (Remediation::CheckNetwork, false),
        _ => (Remediation::None, false),
    }
}

/// Fetch a post by slug, returning [`Error::NotFound`] when absent.
///
/// # Errors
/// Propagates lookup errors and returns [`Error::NotFound`] if no post matches.
pub async fn require_post_by_slug<T: HttpTransport>(client: &Ghost<T>, slug: &str) -> Result<Post> {
    if let Some(post) = client.posts().find_by_slug(slug).await? {
        return Ok(post);
    }
    let suggestion = client
        .posts()
        .list(SUGGEST_LIMIT, 1)
        .await
        .ok()
        .and_then(|posts| nearest_slug(posts.iter().map(|p| p.slug.as_str()), slug));
    Err(Error::NotFound {
        resource: "post",
        slug: slug.to_owned(),
        suggestion,
    })
}

/// Fetch a tag by slug, returning [`Error::NotFound`] when absent.
///
/// # Errors
/// Propagates lookup errors and returns [`Error::NotFound`] if no tag matches.
pub async fn require_tag_by_slug<T: HttpTransport>(client: &Ghost<T>, slug: &str) -> Result<Tag> {
    if let Some(tag) = client.tags().find_by_slug(slug).await? {
        return Ok(tag);
    }
    let suggestion = client
        .tags()
        .list(SUGGEST_LIMIT, 1)
        .await
        .ok()
        .and_then(|tags| nearest_slug(tags.iter().map(|t| t.slug.as_str()), slug));
    Err(Error::NotFound {
        resource: "tag",
        slug: slug.to_owned(),
        suggestion,
    })
}
