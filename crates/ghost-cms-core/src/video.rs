//! Verified local video facts and native editor card contracts.

mod repair;
pub use repair::{RepairReport, repair_owned_videos, verify_repair};

use std::collections::{BTreeMap, BTreeSet};

use ghost_cms_policy::VideoShape;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CoreError, Result};

pub(crate) const MILLISECONDS_PER_SECOND: u32 = 1_000;
pub(crate) const SECONDS_PER_MINUTE: u32 = 60;
const JPEG_START: &[u8] = &[0xff, 0xd8];
const JPEG_END: &[u8] = &[0xff, 0xd9];

/// An editor document whose video metadata passed validation and content preservation checks.
#[derive(Debug, Clone)]
pub struct PatchedEditor(String);

impl std::fmt::Display for PatchedEditor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Measured video metadata and a decoded frame encoded as JPEG.
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub(crate) shape: VideoShape,
    pub(crate) thumbnail: Vec<u8>,
    pub(crate) thumbnail_width: u32,
    pub(crate) thumbnail_height: u32,
}

impl VideoMetadata {
    /// Validate measurements before preparing a native video card.
    /// # Errors
    /// Refuses empty measurements, invalid JPEG boundaries, and oversized thumbnails.
    pub fn new(
        width: u32,
        height: u32,
        duration_ms: u32,
        thumbnail_width: u32,
        thumbnail_height: u32,
        thumbnail: Vec<u8>,
    ) -> Result<Self> {
        let shape = VideoShape::new(width, height, duration_ms).ok_or_else(|| {
            CoreError::Config("video dimensions and duration must be positive".to_owned())
        })?;
        if thumbnail_width == 0
            || thumbnail_height == 0
            || thumbnail.len() as u64 > crate::constants::MAX_PAYLOAD_BYTES
            || !thumbnail.starts_with(JPEG_START)
            || !thumbnail.ends_with(JPEG_END)
        {
            return Err(CoreError::Config(
                "video thumbnail must be a nonempty measured JPEG within the upload limit"
                    .to_owned(),
            ));
        }
        Ok(Self {
            shape,
            thumbnail,
            thumbnail_width,
            thumbnail_height,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Presentation {
    pub(crate) metadata: VideoMetadata,
    pub(crate) thumbnail: String,
}

/// Video fields required to populate the native Ghost editor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoCard {
    /// Uploaded video URL.
    pub src: String,
    /// Automatically generated frame URL.
    #[serde(default)]
    pub thumbnail_src: String,
    /// An authored poster URL.
    #[serde(default)]
    pub custom_thumbnail_src: String,
    /// Display width in pixels.
    pub width: Option<u32>,
    /// Display height in pixels.
    pub height: Option<u32>,
    /// Measured duration in seconds.
    #[serde(default)]
    pub duration: f64,
}

/// Source-bound metadata to fill a native video card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoPatch {
    /// Existing uploaded video URL.
    pub src: String,
    /// Uploaded frame thumbnail URL.
    pub thumbnail: String,
    /// Measured display width.
    pub width: u32,
    /// Measured display height.
    pub height: u32,
    /// Measured duration in milliseconds.
    pub duration_ms: u32,
    /// Generated frame width.
    pub thumbnail_width: u32,
    /// Generated frame height.
    pub thumbnail_height: u32,
    /// Original uploaded filename.
    pub file_name: String,
    /// Video MIME type.
    pub mime_type: String,
}

impl VideoPatch {
    fn validate(&self) -> Result<()> {
        VideoCard {
            src: self.src.clone(),
            thumbnail_src: self.thumbnail.clone(),
            custom_thumbnail_src: String::new(),
            width: Some(self.width),
            height: Some(self.height),
            duration: f64::from(self.duration_ms) / f64::from(MILLISECONDS_PER_SECOND),
        }
        .validate()?;
        if self.thumbnail_width == 0
            || self.thumbnail_height == 0
            || self.file_name.trim().is_empty()
            || !self.mime_type.starts_with("video/")
        {
            return Err(CoreError::Config(
                "video patch has incomplete frame or file metadata".to_owned(),
            ));
        }
        Ok(())
    }
}

const METADATA_FIELDS: &[&str] = &[
    "thumbnailSrc",
    "width",
    "height",
    "duration",
    "thumbnailWidth",
    "thumbnailHeight",
    "fileName",
    "mimeType",
];

/// Fill video metadata without changing source URLs, authored posters, captions, or other content.
/// # Errors
/// Refuses unknown videos, unused metadata, invalid fields, and malformed documents.
pub fn patch_editor(
    lexical: &str,
    patches: &BTreeMap<String, VideoPatch>,
) -> Result<PatchedEditor> {
    for (src, patch) in patches {
        patch.validate()?;
        if *src != patch.src {
            return Err(CoreError::Config(
                "video patch key differs from its source URL".to_owned(),
            ));
        }
    }
    let mut document: Value = serde_json::from_str(lexical)?;
    let mut matched = BTreeSet::new();
    patch_node(&mut document, patches, &mut matched)?;
    if matched.len() != patches.len() {
        return Err(CoreError::Inconsistent(
            "video metadata doesn't match every saved video".to_owned(),
        ));
    }
    let result = serde_json::to_string(&document)?;
    editor_videos(&result)?;
    if !same_content(lexical, &result)? {
        return Err(CoreError::Inconsistent(
            "video metadata patch changed protected editor content".to_owned(),
        ));
    }
    Ok(PatchedEditor(result))
}

fn patch_node(
    node: &mut Value,
    patches: &BTreeMap<String, VideoPatch>,
    matched: &mut BTreeSet<String>,
) -> Result<()> {
    if node.get("type").and_then(Value::as_str) == Some("video") {
        let src = node
            .get("src")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::Inconsistent("video card has no source URL".to_owned()))?
            .to_owned();
        let patch = patches.get(&src).ok_or_else(|| {
            CoreError::Inconsistent("saved video has no matching local source identity".to_owned())
        })?;
        let object = node
            .as_object_mut()
            .ok_or_else(|| CoreError::Inconsistent("video card isn't an object".to_owned()))?;
        let duration = if patch.duration_ms.is_multiple_of(MILLISECONDS_PER_SECOND) {
            serde_json::json!(patch.duration_ms / MILLISECONDS_PER_SECOND)
        } else {
            serde_json::json!(f64::from(patch.duration_ms) / f64::from(MILLISECONDS_PER_SECOND))
        };
        let metadata = serde_json::json!({
            "thumbnailSrc":patch.thumbnail, "width":patch.width, "height":patch.height,
            "duration":duration, "fileName":patch.file_name, "mimeType":patch.mime_type
        });
        let metadata = metadata
            .as_object()
            .ok_or_else(|| CoreError::Inconsistent("video metadata isn't an object".to_owned()))?;
        object.extend(metadata.clone());
        if object
            .get("customThumbnailSrc")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            object.insert("thumbnailWidth".to_owned(), patch.thumbnail_width.into());
            object.insert("thumbnailHeight".to_owned(), patch.thumbnail_height.into());
        }
        matched.insert(src);
    }
    if let Some(root) = node.get_mut("root") {
        patch_node(root, patches, matched)?;
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            patch_node(child, patches, matched)?;
        }
    }
    Ok(())
}

/// Compare editor content while excluding only the metadata fields this repair owns.
/// # Errors
/// Returns document parsing errors.
pub fn same_content(before: &str, after: &str) -> Result<bool> {
    let mut before: Value = serde_json::from_str(before)?;
    let mut after: Value = serde_json::from_str(after)?;
    remove_metadata(&mut before);
    remove_metadata(&mut after);
    Ok(before == after)
}

pub(super) fn remove_metadata(node: &mut Value) {
    if node.get("type").and_then(Value::as_str) == Some("video")
        && let Some(object) = node.as_object_mut()
    {
        for field in METADATA_FIELDS {
            object.remove(*field);
        }
    }
    if let Some(root) = node.get_mut("root") {
        remove_metadata(root);
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            remove_metadata(child);
        }
    }
}

pub(super) fn preserved_status(
    before: crate::domain::PostStatus,
    after: crate::domain::PostStatus,
) -> bool {
    let state = |status| match status {
        crate::domain::PostStatus::Draft => ghost_cms_policy::MetadataPublicationState::Draft,
        crate::domain::PostStatus::Scheduled => {
            ghost_cms_policy::MetadataPublicationState::Scheduled
        },
        crate::domain::PostStatus::Published => {
            ghost_cms_policy::MetadataPublicationState::Published
        },
        crate::domain::PostStatus::Unknown => ghost_cms_policy::MetadataPublicationState::Unknown,
    };
    ghost_cms_policy::metadata_preserves_publication(state(before), state(after))
}

pub(crate) async fn synchronize<T: crate::transport::HttpTransport>(
    client: &crate::Ghost<T>,
    before: crate::domain::Post,
    patches: &BTreeMap<String, VideoPatch>,
) -> Result<crate::domain::Post> {
    let lexical = before
        .lexical
        .as_deref()
        .ok_or_else(|| CoreError::Inconsistent("post has no saved editor document".to_owned()))?;
    let expected = patch_editor(lexical, patches)?.to_string();
    if serde_json::from_str::<Value>(lexical)? == serde_json::from_str::<Value>(&expected)? {
        return Ok(before);
    }
    client
        .posts()
        .update_editor(
            &before.id,
            before
                .updated_at
                .as_deref()
                .ok_or_else(|| CoreError::Inconsistent("post has no revision".to_owned()))?,
            &expected,
        )
        .await?;
    let after = client.posts().get(&before.id).await?;
    verify_update(&before, &after, &expected)?;
    Ok(after)
}

pub(crate) fn verify_update(
    before: &crate::domain::Post,
    after: &crate::domain::Post,
    expected: &str,
) -> Result<()> {
    let actual = after
        .lexical
        .as_deref()
        .ok_or_else(|| CoreError::Inconsistent("updated post has no editor document".to_owned()))?;
    let status_matches = preserved_status(before.status, after.status);
    if before.id != after.id
        || before.slug != after.slug
        || before.title != after.title
        || !status_matches
        || before.published_at != after.published_at
        || before.feature_image != after.feature_image
        || before.feature_image_alt != after.feature_image_alt
        || before.feature_image_caption != after.feature_image_caption
        || !before
            .tags
            .iter()
            .map(|tag| &tag.name)
            .eq(after.tags.iter().map(|tag| &tag.name))
        || serde_json::from_str::<Value>(actual)? != serde_json::from_str::<Value>(expected)?
    {
        return Err(CoreError::Inconsistent(
            "video readback changed protected post content or publication state".to_owned(),
        ));
    }
    editor_videos(actual)?;
    if !same_content(
        before.lexical.as_deref().ok_or_else(|| {
            CoreError::Inconsistent("original post has no editor document".to_owned())
        })?,
        actual,
    )? {
        return Err(CoreError::Inconsistent(
            "video readback changed content outside media metadata".to_owned(),
        ));
    }
    Ok(())
}

impl VideoCard {
    /// Check that the editor can show a populated card with its measured proportions.
    /// # Errors
    /// Refuses missing URLs, empty dimensions, and invalid duration.
    pub fn validate(&self) -> Result<()> {
        let valid_url = |value: &str| {
            reqwest::Url::parse(value).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
            })
        };
        let thumbnail = if self.custom_thumbnail_src.is_empty() {
            &self.thumbnail_src
        } else {
            &self.custom_thumbnail_src
        };
        if !valid_url(&self.src)
            || !valid_url(thumbnail)
            || self.width.is_none_or(|v| v == 0)
            || self.height.is_none_or(|v| v == 0)
            || !self.duration.is_finite()
            || self.duration <= 0.0
        {
            return Err(CoreError::Inconsistent(
                "native video card has no valid thumbnail, dimensions, or duration".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Read all native video cards from a saved editor document.
/// # Errors
/// Refuses a missing or malformed document and incomplete video fields.
pub fn editor_videos(lexical: &str) -> Result<Vec<VideoCard>> {
    let document: Value = serde_json::from_str(lexical)?;
    let children = document
        .get("root")
        .filter(|root| root.get("type").and_then(Value::as_str) == Some("root"))
        .and_then(|root| root.get("children"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::Inconsistent("saved editor document has no root children".to_owned())
        })?;
    let mut videos = Vec::new();
    collect_videos(children, &mut videos)?;
    Ok(videos)
}

fn collect_videos(nodes: &[Value], videos: &mut Vec<VideoCard>) -> Result<()> {
    for node in nodes {
        if node.get("type").and_then(Value::as_str) == Some("video") {
            let card: VideoCard = serde_json::from_value(node.clone())?;
            card.validate()?;
            videos.push(card);
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            collect_videos(children, videos)?;
        }
    }
    Ok(())
}
