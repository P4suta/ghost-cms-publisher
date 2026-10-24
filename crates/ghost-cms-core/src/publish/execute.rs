use super::plan::Plan;
use super::{PublishOptions, PublishOutcome};
use crate::client::Ghost;
use crate::error::{CoreError, Result};
use crate::hash::{PostState, StateCache};
use crate::preparation::PreparedPost;
use crate::transport::HttpTransport;

pub(super) async fn execute<T: HttpTransport>(
    client: &Ghost<T>,
    plan: Plan,
    prepared: &PreparedPost,
    cache: &mut StateCache,
    opts: &PublishOptions,
) -> Result<PublishOutcome> {
    if let Plan::Skip { id } = plan {
        return Ok(PublishOutcome::SkippedUnchanged { id });
    }
    let urls = crate::images::resolve(client, prepared, cache, opts.state_path.as_deref()).await?;
    let input = prepared.publication(&urls)?;
    let (post, created) = match plan {
        Plan::Create => (client.posts().create(&input).await?, true),
        Plan::Update { id, updated_at } => (
            client.posts().update(&id, &input, &updated_at).await?,
            false,
        ),
        Plan::Skip { id } => return Ok(PublishOutcome::SkippedUnchanged { id }),
    };
    if post.slug != prepared.front.slug
        || post.id.is_empty()
        || post.updated_at.as_ref().is_none_or(String::is_empty)
        || post.status != prepared.front.status
    {
        return Err(CoreError::Inconsistent("post write returned an unexpected identity, revision or status; inspect Ghost before retrying".to_owned()));
    }
    let patches = prepared.video_patches(&urls)?;
    let post = if patches.is_empty() {
        post
    } else {
        crate::video::synchronize(client, post, &patches).await?
    };
    cache
        .sites
        .entry(client.cache_namespace().to_owned())
        .or_default()
        .posts
        .insert(
            prepared.front.slug.clone(),
            PostState {
                id: post.id.clone(),
                content_hash: prepared.source_hash.clone(),
                updated_at: post.updated_at.clone(),
            },
        );
    if let Some(path) = opts.state_path.as_deref() {
        cache.save(path, prepared.files.as_ref())?;
    }
    if created {
        Ok(PublishOutcome::Created {
            id: post.id,
            url: post.url,
        })
    } else {
        Ok(PublishOutcome::Updated {
            id: post.id,
            url: post.url,
        })
    }
}
