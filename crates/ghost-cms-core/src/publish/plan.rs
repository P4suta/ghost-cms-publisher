//! Pure decision logic: given the current state, what should publishing do?

use super::{PlannedAction, PublishOptions};
use crate::domain::Post;
use crate::error::{CoreError, Result};
use crate::hash::StateCache;

/// The decided action for a publish run, carrying what `execute` needs.
pub(super) enum Plan {
    /// Create a new post.
    Create,
    /// Update an existing post (conflict-checked with `updated_at`).
    Update {
        /// Existing post id.
        id: String,
        /// Last-known `updated_at`, for conflict detection.
        updated_at: String,
    },
    /// Skip: the content hash matches the cache.
    Skip {
        /// Existing post id.
        id: String,
    },
}

impl Plan {
    /// The user-facing action this plan represents.
    pub(super) const fn action(&self) -> PlannedAction {
        match self {
            Self::Create => PlannedAction::Create,
            Self::Update { .. } => PlannedAction::Update,
            Self::Skip { .. } => PlannedAction::Skip,
        }
    }
}

/// Decide what to do from the current Ghost state and cached hash.
///
/// Pure: all I/O happens in the caller, keeping the decision testable.
///
/// # Errors
/// Returns [`CoreError::Inconsistent`] if an existing post lacks the
/// `updated_at` needed to update it safely.
pub(super) fn decide(
    existing: Option<Post>,
    content_hash: &str,
    cached_hash: Option<&str>,
    force: bool,
) -> Result<Plan> {
    let Some(post) = existing else {
        return Ok(Plan::Create);
    };
    if !force && cached_hash == Some(content_hash) {
        return Ok(Plan::Skip { id: post.id });
    }
    let updated_at = post.updated_at.ok_or_else(|| {
        CoreError::Inconsistent(
            "existing post is missing updated_at; cannot update safely".to_owned(),
        )
    })?;
    Ok(Plan::Update {
        id: post.id,
        updated_at,
    })
}

/// Load the cached content hash for `slug`, if a cache is configured.
///
/// A corrupt cache is logged and treated as empty.
pub(super) fn load_cached_hash(opts: &PublishOptions, slug: &str) -> Option<String> {
    let path = opts.state_path.as_deref()?;
    let cache = StateCache::load(path).unwrap_or_else(|e| {
        tracing::warn!("ignoring unreadable publish state cache: {e}");
        StateCache::default()
    });
    cache.posts.get(slug).map(|s| s.content_hash.clone())
}

#[cfg(test)]
mod tests {
    use super::{Plan, decide};
    use crate::domain::{Post, PostStatus};

    fn post(updated_at: Option<&str>) -> Post {
        Post {
            id: "id1".to_owned(),
            title: None,
            slug: "s".to_owned(),
            status: PostStatus::Draft,
            url: None,
            updated_at: updated_at.map(str::to_owned),
            html: None,
        }
    }

    #[test]
    fn absent_post_creates() {
        assert!(matches!(decide(None, "h", None, false), Ok(Plan::Create)));
    }

    #[test]
    fn unchanged_hash_skips() {
        let plan = decide(Some(post(Some("t"))), "h", Some("h"), false);
        assert!(matches!(plan, Ok(Plan::Skip { .. })));
    }

    #[test]
    fn force_overrides_skip() {
        let plan = decide(Some(post(Some("t"))), "h", Some("h"), true);
        assert!(matches!(plan, Ok(Plan::Update { .. })));
    }

    #[test]
    fn changed_hash_updates() {
        let plan = decide(Some(post(Some("t"))), "h2", Some("h1"), false);
        assert!(matches!(plan, Ok(Plan::Update { .. })));
    }

    #[test]
    fn missing_updated_at_is_inconsistent() {
        assert!(decide(Some(post(None)), "h2", Some("h1"), false).is_err());
    }
}
