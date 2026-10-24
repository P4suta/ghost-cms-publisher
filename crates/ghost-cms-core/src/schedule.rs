//! Destination-bound draft preparation and resumable web publication schedules.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ghost_cms_policy::{
    RegistrationDecision, RegistrationRemote, RegistrationState, registration_decision,
};
use serde::{Deserialize, Serialize};

use crate::domain::{Post, PostStatus};
use crate::filesystem::Files;
use crate::hash::{StateCache, bytes_hash, lock};
use crate::preparation::{PrepareOptions, PreparedCollection};
use crate::publish::{PublishOptions, publish_prepared};
use crate::transport::HttpTransport;
use crate::{CoreError, Ghost, Result};

const RFC3339_MAX_YEAR: i32 = 9_999;
const GHOST_SCHEDULING_LEAD_SECONDS: u64 = 120;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Draft {
    pub(crate) id: String,
    revision: String,
    remote_hash: String,
    feature_image: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DraftBatch {
    pub(crate) version: u8,
    pub(crate) destination: String,
    pub(crate) site_url: String,
    pub(crate) files: Vec<PathBuf>,
    sources: BTreeMap<String, String>,
    pub(crate) posts: BTreeMap<String, Draft>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum Progress {
    Planned,
    Pending,
    Registered,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    slug: String,
    group: u32,
    draft: Draft,
    published_at: String,
    progress: Progress,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePlan {
    version: u8,
    destination: String,
    site_url: String,
    files: Vec<PathBuf>,
    sources: BTreeMap<String, String>,
    entries: Vec<Entry>,
}

/// Readback of one scheduled or published post.
#[derive(Debug, Serialize)]
pub struct ReleasePost {
    /// Stable article slug.
    pub slug: String,
    /// Ghost post identity.
    pub id: String,
    /// Expected publication time in UTC.
    pub published_at: String,
    /// Current remote status.
    pub status: PostStatus,
    /// Uploaded cover URL.
    pub feature_image: Option<String>,
    /// Cover description returned by Ghost.
    pub feature_image_alt: Option<String>,
    /// Cover caption and attribution returned by Ghost.
    pub feature_image_caption: Option<String>,
    /// Tag names returned by Ghost.
    pub tags: Vec<String>,
    /// Saved top-level editor block types and counts.
    pub editor_blocks: BTreeMap<String, usize>,
}

/// Current readback of the frozen release plan.
#[derive(Debug, Serialize)]
pub struct ReleaseReport {
    /// All posts in release order.
    pub posts: Vec<ReleasePost>,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::Config(message.into())
}

fn save(files: &Files, path: &Path, value: &impl Serialize) -> Result<()> {
    files.replace_state(path, &serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn revision(post: &Post) -> Result<&str> {
    post.updated_at
        .as_deref()
        .filter(|r| !r.is_empty())
        .ok_or_else(|| invalid(format!("post {} has no revision", post.slug)))
}

fn fingerprint(post: &Post) -> Result<String> {
    Ok(bytes_hash(&serde_json::to_vec(&(
        &post.title,
        &post.html,
        &post.feature_image,
        &post.feature_image_alt,
        &post.feature_image_caption,
        post.tags
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
    ))?)
    .to_string())
}

fn snapshot(post: &Post) -> Result<Draft> {
    if post.id.is_empty() || post.html.is_none() {
        return Err(invalid(format!(
            "post {} has incomplete readback",
            post.slug
        )));
    }
    Ok(Draft {
        id: post.id.clone(),
        revision: revision(post)?.to_owned(),
        remote_hash: fingerprint(post)?,
        feature_image: post.feature_image.clone(),
    })
}

async fn destination<T: HttpTransport>(client: &Ghost<T>, expected: &str) -> Result<()> {
    let expected = reqwest::Url::parse(expected).map_err(|e| invalid(e.to_string()))?;
    let actual =
        reqwest::Url::parse(&client.site().get().await?.url).map_err(|e| invalid(e.to_string()))?;
    if expected != actual {
        return Err(invalid(format!(
            "site URL mismatch: expected {expected}, received {actual}"
        )));
    }
    Ok(())
}

fn source_hashes(collection: &PreparedCollection) -> BTreeMap<String, String> {
    collection
        .posts()
        .iter()
        .map(|p| (p.front.slug.clone(), p.source_hash.clone()))
        .collect()
}

fn validate_sources(
    files: &Files,
    paths: &[PathBuf],
    sources: &BTreeMap<String, String>,
) -> Result<()> {
    let collection = PreparedCollection::load(paths, PrepareOptions::default(), files.clone())?;
    if source_hashes(&collection) != *sources {
        return Err(invalid(
            "release source or media changed; prepare a new collection",
        ));
    }
    Ok(())
}

fn same_content(post: &Post, draft: &Draft) -> Result<bool> {
    Ok(post.id == draft.id && fingerprint(post)? == draft.remote_hash)
}

fn validate_drafts(collection: &PreparedCollection, require_feature_images: bool) -> Result<()> {
    for post in collection.posts() {
        if post.front.status != PostStatus::Draft || post.front.email_only == Some(true) {
            return Err(invalid("draft preparation requires web drafts"));
        }
        if require_feature_images
            && [
                post.front.feature_image.as_deref(),
                post.front.feature_image_alt.as_deref(),
                post.front.feature_image_caption.as_deref(),
            ]
            .iter()
            .any(|field| field.is_none_or(|v| v.trim().is_empty()))
        {
            return Err(invalid(format!(
                "post {} needs a cover, description and caption",
                post.front.slug
            )));
        }
    }
    Ok(())
}

async fn preflight_drafts<T: HttpTransport>(
    client: &Ghost<T>,
    collection: &PreparedCollection,
    batch: &DraftBatch,
    cache: &StateCache,
) -> Result<()> {
    for post in collection.posts() {
        let slug = &post.front.slug;
        if client.posts().page_slug_exists(slug).await? {
            return Err(invalid(format!("a static page owns slug {slug}")));
        }
        let remote = client.posts().find_by_slug(slug).await?;
        if let Some(remote) = remote {
            let expected_source = &post.source_hash;
            let owned = match batch.posts.get(slug) {
                Some(draft) if remote.updated_at.as_deref() == Some(&draft.revision) => {
                    same_content(&remote, draft)?
                },
                _ => cache
                    .sites
                    .get(client.cache_namespace())
                    .and_then(|s| s.posts.get(slug))
                    .is_some_and(|p| {
                        p.id == remote.id
                            && p.updated_at == remote.updated_at
                            && p.content_hash == *expected_source
                    }),
            };
            if !owned || remote.status != PostStatus::Draft {
                return Err(invalid(format!(
                    "unrecorded or changed remote slug {slug}; no drafts were written"
                )));
            }
        } else if batch.posts.contains_key(slug) {
            return Err(invalid(format!("recorded draft {slug} was deleted")));
        }
    }
    Ok(())
}

/// Create or resume drafts after checking every slug and the expected public site.
/// # Errors
/// Refuses foreign slugs, changed collection membership, remote edits, missing covers when required, and incomplete readback.
pub async fn stage_drafts<T: HttpTransport>(
    client: &Ghost<T>,
    collection: &PreparedCollection,
    state: &Path,
    expected_site_url: &str,
    require_feature_images: bool,
) -> Result<usize> {
    let files = collection.files.clone();
    let _lease = lock(state, files.as_ref())?;
    let sources = source_hashes(collection);
    validate_drafts(collection, require_feature_images)?;
    let mut batch: DraftBatch = match files.read(state) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DraftBatch {
            version: 1,
            destination: client.cache_namespace().to_owned(),
            site_url: expected_site_url.to_owned(),
            files: collection
                .posts()
                .iter()
                .map(|p| p.source_path().to_owned())
                .collect(),
            sources: sources.clone(),
            posts: BTreeMap::new(),
        },
        Err(error) => return Err(error.into()),
    };
    if batch.version != 1
        || batch.destination != client.cache_namespace()
        || !batch.sources.keys().eq(sources.keys())
        || batch.site_url != expected_site_url
    {
        return Err(invalid(
            "draft receipt belongs to another destination or collection",
        ));
    }
    destination(client, expected_site_url).await?;
    let cache_path = state.with_extension("publication-cache.json");
    let cache = StateCache::load(&cache_path, files.as_ref())?;
    preflight_drafts(client, collection, &batch, &cache).await?;
    batch.sources = sources;
    batch.files = collection
        .posts()
        .iter()
        .map(|p| p.source_path().to_owned())
        .collect();
    save(&files, state, &batch)?;
    let options = PublishOptions {
        state_path: Some(cache_path),
        ..PublishOptions::default()
    };
    for post in collection.posts() {
        publish_prepared(client, post, &options).await?;
        let remote = client
            .posts()
            .find_by_slug(&post.front.slug)
            .await?
            .ok_or_else(|| invalid("created draft missing from readback"))?;
        if remote.editor_blocks()?.keys().all(|kind| kind == "html") {
            return Err(invalid(format!(
                "draft {} is entirely an HTML card",
                remote.slug
            )));
        }
        if remote.status != PostStatus::Draft
            || remote.title.as_deref() != Some(&post.front.title)
            || remote.feature_image_alt != post.front.feature_image_alt
            || remote.feature_image_caption != post.front.feature_image_caption
            || (post.front.tags.is_empty() && !remote.tags.is_empty())
            || (post.front.feature_image.is_some()
                && remote.feature_image.as_deref().is_none_or(str::is_empty))
        {
            return Err(invalid(format!(
                "draft metadata differs for {}",
                post.front.slug
            )));
        }
        batch
            .posts
            .insert(post.front.slug.clone(), snapshot(&remote)?);
        save(&files, state, &batch)?;
        tracing::info!(slug = %post.front.slug, "draft readback recorded");
    }
    Ok(batch.posts.len())
}

fn timestamp(seconds: u64) -> Result<String> {
    let seconds = i64::try_from(seconds).map_err(|e| invalid(e.to_string()))?;
    let date =
        time::OffsetDateTime::from_unix_timestamp(seconds).map_err(|e| invalid(e.to_string()))?;
    if date.year() > RFC3339_MAX_YEAR {
        return Err(invalid("publication date exceeds RFC 3339"));
    }
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.year(),
        u8::from(date.month()),
        date.day(),
        date.hour(),
        date.minute(),
        date.second()
    ))
}

fn instant(value: &str) -> Result<time::OffsetDateTime> {
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|e| invalid(e.to_string()))
}

fn seconds(value: &str) -> Result<u64> {
    let date = instant(value)?;
    u64::try_from(date.unix_timestamp()).map_err(|e| invalid(e.to_string()))
}

async fn remote<T: HttpTransport>(client: &Ghost<T>, slug: &str) -> Result<Post> {
    client
        .posts()
        .find_by_slug(slug)
        .await?
        .ok_or_else(|| invalid(format!("remote post {slug} is missing")))
}

/// Freeze a verified ordered set of drafts into whole-hour and ten-minute release slots.
/// # Errors
/// Refuses incomplete orders, changed sources, stale draft revisions, and an existing output.
pub async fn create_release_plan<T: HttpTransport>(
    client: &Ghost<T>,
    drafts: &Path,
    groups: &[Vec<String>],
    output: &Path,
    now: u64,
    files: Files,
) -> Result<ReleaseReport> {
    let _lease = lock(output, files.as_ref())?;
    match files.metadata(output) {
        Ok(_) => return Err(invalid("release plan already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    let batch: DraftBatch = serde_json::from_slice(&files.read(drafts)?)?;
    if batch.version != 1
        || batch.destination != client.cache_namespace()
        || batch.posts.len() != batch.sources.len()
    {
        return Err(invalid(
            "draft collection is incomplete or belongs to another destination",
        ));
    }
    validate_sources(&files, &batch.files, &batch.sources)?;
    let order: BTreeSet<_> = groups.iter().flatten().cloned().collect();
    if groups.iter().any(Vec::is_empty)
        || order.len() != groups.iter().map(Vec::len).sum::<usize>()
        || order != batch.sources.keys().cloned().collect()
    {
        return Err(invalid(
            "release order must contain every draft exactly once",
        ));
    }
    destination(client, &batch.site_url).await?;
    let first = ghost_cms_policy::first_release_slot(now, None)
        .map_err(|e| invalid(format!("invalid release start: {e:?}")))?;
    let mut entries = Vec::new();
    for (group, slugs) in groups.iter().enumerate() {
        let group = u32::try_from(group).map_err(|e| invalid(e.to_string()))?;
        let at = timestamp(
            ghost_cms_policy::release_slot(first, group)
                .map_err(|e| invalid(format!("invalid release slot: {e:?}")))?
                .seconds(),
        )?;
        for slug in slugs {
            let draft = batch
                .posts
                .get(slug)
                .ok_or_else(|| invalid("draft missing from order"))?;
            let post = remote(client, slug).await?;
            if post.status != PostStatus::Draft
                || revision(&post)? != draft.revision
                || !same_content(&post, draft)?
            {
                return Err(invalid(format!("draft {slug} changed after preparation")));
            }
            entries.push(Entry {
                slug: slug.clone(),
                group,
                draft: draft.clone(),
                published_at: at.clone(),
                progress: Progress::Planned,
            });
        }
    }
    let plan = ReleasePlan {
        version: 1,
        destination: batch.destination,
        site_url: batch.site_url,
        files: batch.files,
        sources: batch.sources,
        entries,
    };
    save(&files, output, &plan)?;
    inspect(client, &plan, false).await
}

fn load_plan(
    client: &Ghost<impl HttpTransport>,
    files: &Files,
    path: &Path,
) -> Result<ReleasePlan> {
    let plan: ReleasePlan = serde_json::from_slice(&files.read(path)?)?;
    let slugs: BTreeSet<_> = plan.entries.iter().map(|e| e.slug.clone()).collect();
    if plan.version != 1
        || plan.destination != client.cache_namespace()
        || plan.entries.is_empty()
        || slugs.len() != plan.entries.len()
        || slugs != plan.sources.keys().cloned().collect()
    {
        return Err(invalid("invalid release inventory or destination"));
    }
    for entry in &plan.entries {
        seconds(&entry.published_at)?;
    }
    validate_sources(files, &plan.files, &plan.sources)?;
    Ok(plan)
}

fn is_registered(entry: &Entry, post: &Post) -> Result<bool> {
    Ok(
        matches!(post.status, PostStatus::Scheduled | PostStatus::Published)
            && post.published_at.as_deref().map(instant).transpose()?
                == Some(instant(&entry.published_at)?)
            && same_content(post, &entry.draft)?,
    )
}

async fn inspect<T: HttpTransport>(
    client: &Ghost<T>,
    plan: &ReleasePlan,
    require_registered: bool,
) -> Result<ReleaseReport> {
    let mut posts = Vec::new();
    for entry in &plan.entries {
        let post = remote(client, &entry.slug).await?;
        let registered = is_registered(entry, &post)?;
        let state = match entry.progress {
            Progress::Planned => RegistrationState::Planned,
            Progress::Pending => RegistrationState::Pending,
            Progress::Registered => RegistrationState::Registered,
        };
        let observed = if registered {
            RegistrationRemote::Scheduled
        } else if post.status == PostStatus::Draft
            && revision(&post)? == entry.draft.revision
            && same_content(&post, &entry.draft)?
        {
            RegistrationRemote::Draft
        } else {
            RegistrationRemote::Changed
        };
        let decision = registration_decision(state, observed);
        if decision == RegistrationDecision::Conflict
            || (require_registered && decision != RegistrationDecision::Keep)
        {
            return Err(invalid(format!(
                "release readback differs for {}",
                entry.slug
            )));
        }
        posts.push(ReleasePost {
            editor_blocks: post.editor_blocks()?,
            slug: entry.slug.clone(),
            id: post.id,
            published_at: entry.published_at.clone(),
            status: post.status,
            feature_image: post.feature_image,
            feature_image_alt: post.feature_image_alt,
            feature_image_caption: post.feature_image_caption,
            tags: post.tags.into_iter().map(|tag| tag.name).collect(),
        });
    }
    Ok(ReleaseReport { posts })
}

/// Register or resume a frozen schedule, retaining every already registered time.
/// # Errors
/// Refuses remote edits and source changes, and preserves pending write intent after an uncertain response.
pub async fn apply_release_plan<T: HttpTransport>(
    client: &Ghost<T>,
    path: &Path,
    now: u64,
    files: Files,
) -> Result<ReleaseReport> {
    let _lease = lock(path, files.as_ref())?;
    let mut plan = load_plan(client, &files, path)?;
    destination(client, &plan.site_url).await?;
    let observed = inspect(client, &plan, false).await?;
    for (entry, post) in plan.entries.iter_mut().zip(&observed.posts) {
        if matches!(post.status, PostStatus::Scheduled | PostStatus::Published) {
            entry.progress = Progress::Registered;
        }
    }
    let safety = now
        .checked_add(GHOST_SCHEDULING_LEAD_SECONDS)
        .ok_or_else(|| invalid("clock overflow"))?;
    let expired = plan.entries.iter().any(|e| {
        !matches!(e.progress, Progress::Registered)
            && seconds(&e.published_at).is_ok_and(|t| t <= safety)
    });
    if expired {
        let last = plan
            .entries
            .iter()
            .filter(|e| matches!(e.progress, Progress::Registered))
            .map(|e| seconds(&e.published_at))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max();
        let first = ghost_cms_policy::first_release_slot(now, last)
            .map_err(|e| invalid(format!("invalid resume time: {e:?}")))?;
        let mut group_slots = BTreeMap::new();
        for entry in &mut plan.entries {
            if matches!(entry.progress, Progress::Registered) {
                continue;
            }
            let next = u32::try_from(group_slots.len()).map_err(|e| invalid(e.to_string()))?;
            let slot = *group_slots.entry(entry.group).or_insert(next);
            entry.published_at = timestamp(
                ghost_cms_policy::release_slot(first, slot)
                    .map_err(|e| invalid(format!("invalid resumed slot: {e:?}")))?
                    .seconds(),
            )?;
            entry.progress = Progress::Planned;
        }
    }
    save(&files, path, &plan)?;
    for index in 0..plan.entries.len() {
        if matches!(plan.entries[index].progress, Progress::Registered) {
            continue;
        }
        plan.entries[index].progress = Progress::Pending;
        save(&files, path, &plan)?;
        let entry = &plan.entries[index];
        client
            .posts()
            .schedule(&entry.draft.id, &entry.draft.revision, &entry.published_at)
            .await?;
        let post = remote(client, &entry.slug).await?;
        if !is_registered(entry, &post)? {
            return Err(invalid(format!(
                "scheduled post {} differs from the plan",
                entry.slug
            )));
        }
        tracing::info!(slug = %entry.slug, published_at = %entry.published_at, "scheduled post read back");
        plan.entries[index].progress = Progress::Registered;
        save(&files, path, &plan)?;
    }
    inspect(client, &plan, true).await
}

/// Verify all planned identities, metadata, content, statuses, and times without remote mutation.
/// # Errors
/// Refuses pending registrations, remote edits, source changes, and a different destination.
pub async fn verify_release_plan<T: HttpTransport>(
    client: &Ghost<T>,
    path: &Path,
    files: Files,
) -> Result<ReleaseReport> {
    let plan = load_plan(client, &files, path)?;
    destination(client, &plan.site_url).await?;
    inspect(client, &plan, true).await
}
