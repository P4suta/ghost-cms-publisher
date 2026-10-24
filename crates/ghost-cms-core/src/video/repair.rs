use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    VideoPatch, editor_videos, patch_editor, preserved_status, remove_metadata, synchronize,
};
use crate::domain::{Post, PostStatus};
use crate::filesystem::Files;
use crate::hash::{StateCache, bytes_hash, lock};
use crate::preparation::{AssetKind, PrepareOptions, PreparedCollection, PreparedPost};
use crate::schedule::DraftBatch;
use crate::transport::HttpTransport;
use crate::{CoreError, Ghost, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Progress {
    Planned,
    Pending,
    Verified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PostRepair {
    slug: String,
    id: String,
    status: PostStatus,
    published_at: Option<String>,
    protected_hash: String,
    lexical_hash: String,
    backup_lexical: Option<String>,
    videos: usize,
    changed: bool,
    progress: Progress,
}

/// Private readback record for a destination-bound video repair.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairReport {
    version: u8,
    destination: String,
    site_url: String,
    applied: bool,
    posts: Vec<PostRepair>,
}

impl RepairReport {
    /// Whether this record describes applied and verified changes.
    #[must_use]
    pub const fn applied(&self) -> bool {
        self.applied
    }

    /// Number of inspected owned posts.
    #[must_use]
    pub const fn posts(&self) -> usize {
        self.posts.len()
    }
    /// Number of native video cards.
    #[must_use]
    pub fn videos(&self) -> usize {
        self.posts.iter().map(|post| post.videos).sum()
    }
    /// Number of posts requiring metadata changes.
    #[must_use]
    pub fn changed(&self) -> usize {
        self.posts.iter().filter(|post| post.changed).count()
    }
}

fn lexical(post: &Post) -> Result<&str> {
    post.lexical.as_deref().ok_or_else(|| {
        CoreError::Inconsistent(format!("post {} has no editor document", post.slug))
    })
}

fn protected_hash(post: &Post) -> Result<String> {
    let mut document: serde_json::Value = serde_json::from_str(lexical(post)?)?;
    remove_metadata(&mut document);
    Ok(bytes_hash(&serde_json::to_vec(&serde_json::json!({
        "id":post.id,"slug":post.slug,"title":post.title,"document":document,
        "feature_image":post.feature_image,"feature_image_alt":post.feature_image_alt,"feature_image_caption":post.feature_image_caption,
        "tags":post.tags.iter().map(|tag| &tag.name).collect::<Vec<_>>()
    }))?).to_string())
}

fn equal_documents(before: &str, after: &str) -> Result<bool> {
    Ok(serde_json::from_str::<serde_json::Value>(before)?
        == serde_json::from_str::<serde_json::Value>(after)?)
}

async fn observe<T: HttpTransport>(
    client: &Ghost<T>,
    prepared: &PreparedPost,
    id: &str,
    urls: &BTreeMap<String, String>,
) -> Result<PostRepair> {
    let before = client.posts().get(id).await?;
    if before.id != id || before.slug != prepared.front.slug || before.status == PostStatus::Unknown
    {
        return Err(CoreError::Inconsistent(
            "video repair found a different post identity or unsupported state".to_owned(),
        ));
    }
    let expected = patch_editor(lexical(&before)?, &prepared.video_patches(urls)?)?.to_string();
    let changed = !equal_documents(lexical(&before)?, &expected)?;
    Ok(PostRepair {
        slug: before.slug.clone(),
        id: before.id.clone(),
        status: before.status,
        published_at: before.published_at.clone(),
        protected_hash: protected_hash(&before)?,
        lexical_hash: bytes_hash(lexical(&before)?.as_bytes()).to_string(),
        backup_lexical: changed
            .then(|| lexical(&before).map(str::to_owned))
            .transpose()?,
        videos: editor_videos(&expected)?.len(),
        changed,
        progress: if changed {
            Progress::Planned
        } else {
            Progress::Verified
        },
    })
}

fn save(files: &Files, output: &Path, report: &RepairReport) -> Result<()> {
    files.replace_state(output, &serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

async fn destination<T: HttpTransport>(
    client: &Ghost<T>,
    namespace: &str,
    site_url: &str,
) -> Result<()> {
    if namespace != client.cache_namespace()
        || reqwest::Url::parse(site_url).map_err(|error| CoreError::Config(error.to_string()))?
            != reqwest::Url::parse(&client.site().get().await?.url)
                .map_err(|error| CoreError::Config(error.to_string()))?
    {
        return Err(CoreError::Config(
            "video repair destination differs from its ownership receipt".to_owned(),
        ));
    }
    Ok(())
}

fn known_urls(cache: &StateCache, namespace: &str) -> BTreeMap<String, String> {
    cache
        .sites
        .get(namespace)
        .map(|site| site.assets.clone())
        .unwrap_or_default()
}

async fn upload_frames<T: HttpTransport>(
    client: &Ghost<T>,
    prepared: &PreparedPost,
    cache: &mut StateCache,
    state: &Path,
) -> Result<()> {
    for asset in prepared
        .assets
        .values()
        .filter(|asset| matches!(asset.kind, AssetKind::Image) && asset.generated_thumbnail())
    {
        let bytes = asset.read_verified()?;
        if cache
            .sites
            .get(client.cache_namespace())
            .is_some_and(|site| site.assets.contains_key(&asset.key))
        {
            continue;
        }
        let filename = asset
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| CoreError::Config("thumbnail filename isn't UTF-8".to_owned()))?;
        let url = client
            .media()
            .upload_image(bytes, filename, "image/jpeg")
            .await?;
        let parsed =
            reqwest::Url::parse(&url).map_err(|error| CoreError::Config(error.to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(CoreError::Inconsistent(
                "thumbnail upload returned an invalid URL".to_owned(),
            ));
        }
        cache
            .sites
            .entry(client.cache_namespace().to_owned())
            .or_default()
            .assets
            .insert(asset.key.clone(), url);
        cache.save(state, prepared.files.as_ref())?;
    }
    Ok(())
}

/// Plan or apply metadata repairs for posts identified by an existing draft receipt.
/// # Errors
/// Refuses unknown posts, changed media, invalid documents, conflicts, and an existing report path.
pub async fn repair_owned_videos<T: HttpTransport>(
    client: &Ghost<T>,
    receipt: &Path,
    output: &Path,
    apply: bool,
    files: Files,
) -> Result<RepairReport> {
    let _report_lease = lock(output, files.as_ref())?;
    match files.metadata(output) {
        Ok(_) => {
            return Err(CoreError::Config(
                "video repair report already exists; use a fresh path".to_owned(),
            ));
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.into()),
    }
    let batch: DraftBatch = serde_json::from_slice(&files.read(receipt)?)?;
    if batch.version != 1 || batch.posts.is_empty() {
        return Err(CoreError::Config(
            "video repair needs a nonempty version-one draft receipt".to_owned(),
        ));
    }
    destination(client, &batch.destination, &batch.site_url).await?;
    let state = receipt.with_extension("publication-cache.json");
    let _lease = if apply {
        Some(lock(&state, files.as_ref())?)
    } else {
        None
    };
    let mut cache = StateCache::load(&state, files.as_ref())?;
    let collection =
        PreparedCollection::load(&batch.files, PrepareOptions::default(), files.clone())?;
    if collection.posts().len() != batch.posts.len() {
        return Err(CoreError::Config(
            "video repair source coverage differs from its ownership receipt".to_owned(),
        ));
    }
    let urls = known_urls(&cache, client.cache_namespace());
    let mut report = RepairReport {
        version: 1,
        destination: batch.destination,
        site_url: batch.site_url,
        applied: apply,
        posts: Vec::new(),
    };
    for prepared in collection.posts() {
        let owned = batch.posts.get(&prepared.front.slug).ok_or_else(|| {
            CoreError::Config("prepared post has no ownership receipt".to_owned())
        })?;
        report
            .posts
            .push(observe(client, prepared, &owned.id, &urls).await?);
    }
    save(&files, output, &report)?;
    if !apply {
        return Ok(report);
    }
    for (index, prepared) in collection.posts().iter().enumerate() {
        if !report.posts[index].changed {
            continue;
        }
        upload_frames(client, prepared, &mut cache, &state).await?;
        let patches: BTreeMap<String, VideoPatch> =
            prepared.video_patches(&known_urls(&cache, client.cache_namespace()))?;
        let before = client.posts().get(&report.posts[index].id).await?;
        if before.slug != report.posts[index].slug {
            return Err(CoreError::Inconsistent(
                "video repair post identity changed".to_owned(),
            ));
        }
        report.posts[index].backup_lexical = Some(lexical(&before)?.to_owned());
        report.posts[index].protected_hash = protected_hash(&before)?;
        report.posts[index].status = before.status;
        report.posts[index]
            .published_at
            .clone_from(&before.published_at);
        report.posts[index].progress = Progress::Pending;
        save(&files, output, &report)?;
        let after = synchronize(client, before, &patches).await?;
        report.posts[index].lexical_hash = bytes_hash(lexical(&after)?.as_bytes()).to_string();
        report.posts[index].progress = Progress::Verified;
        save(&files, output, &report)?;
    }
    verify_repair(client, output, files).await
}

/// Read back a completed repair while allowing the scheduled publication transition.
/// # Errors
/// Refuses incomplete, stale, or mismatched content and publication identities.
pub async fn verify_repair<T: HttpTransport>(
    client: &Ghost<T>,
    report_path: &Path,
    files: Files,
) -> Result<RepairReport> {
    let report: RepairReport = serde_json::from_slice(&files.read(report_path)?)?;
    if report.version != 1
        || !report.applied
        || report.posts.is_empty()
        || report
            .posts
            .iter()
            .any(|post| post.progress != Progress::Verified)
    {
        return Err(CoreError::Config(
            "video repair report isn't a completed applied repair".to_owned(),
        ));
    }
    destination(client, &report.destination, &report.site_url).await?;
    for expected in &report.posts {
        let actual = client.posts().get(&expected.id).await?;
        let status_matches = preserved_status(expected.status, actual.status);
        if actual.id != expected.id
            || actual.slug != expected.slug
            || !status_matches
            || actual.published_at != expected.published_at
            || protected_hash(&actual)? != expected.protected_hash
            || bytes_hash(lexical(&actual)?.as_bytes()).to_string() != expected.lexical_hash
            || editor_videos(lexical(&actual)?)?.len() != expected.videos
        {
            return Err(CoreError::Inconsistent(format!(
                "video repair readback differs for {}",
                expected.slug
            )));
        }
    }
    Ok(report)
}
