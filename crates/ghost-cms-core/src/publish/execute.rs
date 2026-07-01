//! Side-effecting execution of a decided [`Plan`].

use super::plan::Plan;
use super::{PublishOptions, PublishOutcome};
use crate::client::Ghost;
use crate::domain::PostInput;
use crate::error::Result;
use crate::hash::{PostState, StateCache};
use crate::transport::HttpTransport;

/// Execute a non-dry-run [`Plan`], writing to Ghost and recording cache state.
///
/// # Errors
/// Propagates transport and API errors.
pub(super) async fn execute<T: HttpTransport>(
    client: &Ghost<T>,
    plan: Plan,
    input: &PostInput,
    slug: &str,
    content_hash: String,
    opts: &PublishOptions,
) -> Result<PublishOutcome> {
    match plan {
        Plan::Skip { id } => Ok(PublishOutcome::SkippedUnchanged { id }),
        Plan::Create => {
            let post = client.posts().create(input).await?;
            record_state(
                opts,
                slug,
                PostState {
                    id: post.id.clone(),
                    content_hash,
                    updated_at: post.updated_at.clone(),
                },
            );
            Ok(PublishOutcome::Created {
                id: post.id,
                url: post.url,
            })
        },
        Plan::Update { id, updated_at } => {
            let post = client.posts().update(&id, input, &updated_at).await?;
            record_state(
                opts,
                slug,
                PostState {
                    id: post.id.clone(),
                    content_hash,
                    updated_at: post.updated_at.clone(),
                },
            );
            Ok(PublishOutcome::Updated {
                id: post.id,
                url: post.url,
            })
        },
    }
}

/// Record the published state for `slug` in the cache file, if one is set.
///
/// Cache failures are logged but never fail the publish — Ghost is the source
/// of truth and a stale cache only costs an extra lookup next time.
fn record_state(opts: &PublishOptions, slug: &str, state: PostState) {
    let Some(path) = opts.state_path.as_deref() else {
        return;
    };
    let mut cache = StateCache::load(path).unwrap_or_else(|e| {
        tracing::warn!("ignoring unreadable publish state cache: {e}");
        StateCache::default()
    });
    cache.posts.insert(slug.to_owned(), state);
    if let Err(e) = cache.save(path) {
        tracing::warn!("failed to persist publish state cache: {e}");
    }
}
