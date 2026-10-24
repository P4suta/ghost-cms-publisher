use std::path::Path;

use anyhow::{Context, Result, ensure};
use ghost_cms_core::Ghost;
use ghost_cms_core::domain::{Post, PostInput, PostStatus};
use ghost_cms_core::video::{editor_videos, repair_owned_videos, verify_repair};
use serde_json::Value;

use super::{current_time, progress};

const SCHEDULE_AHEAD_SECONDS: u64 = 3_600;

fn lexical(post: &Post) -> Result<&str> {
    post.lexical
        .as_deref()
        .context("fixture editor document missing")
}

fn empty_videos(node: &mut Value) {
    if node.get("type").and_then(Value::as_str) == Some("video") {
        node["thumbnailSrc"] = "".into();
        node["customThumbnailSrc"] = "".into();
        node["width"] = Value::Null;
        node["height"] = Value::Null;
        node["duration"] = 0.into();
        node["fileName"] = "".into();
        node["mimeType"] = "".into();
    }
    if let Some(root) = node.get_mut("root") {
        empty_videos(root);
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            empty_videos(child);
        }
    }
}

async fn fixture_state(client: &Ghost, post: &Post, scheduled: bool) -> Result<Post> {
    let revision = post
        .updated_at
        .as_deref()
        .context("fixture revision missing")?;
    if scheduled {
        let at = time::OffsetDateTime::from_unix_timestamp(i64::try_from(
            current_time()? + SCHEDULE_AHEAD_SECONDS,
        )?)?;
        let at = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            at.year(),
            u8::from(at.month()),
            at.day(),
            at.hour(),
            at.minute(),
            at.second()
        );
        client.posts().schedule(&post.id, revision, &at).await?;
    } else {
        client
            .posts()
            .update(
                &post.id,
                &PostInput {
                    title: post.title.clone().context("fixture title missing")?,
                    html: post.html.clone(),
                    status: PostStatus::Published,
                    ..PostInput::default()
                },
                revision,
            )
            .await?;
    }
    let post = client.posts().get(&post.id).await?;
    let mut document: Value = serde_json::from_str(lexical(&post)?)?;
    empty_videos(&mut document);
    client
        .posts()
        .update_editor(
            &post.id,
            post.updated_at
                .as_deref()
                .context("fixture revision missing")?,
            &document.to_string(),
        )
        .await?;
    let post = client.posts().get(&post.id).await?;
    ensure!(
        post.editor_blocks().is_err(),
        "empty native video fixture wasn't rejected"
    );
    Ok(post)
}

async fn reject_changed_caption(
    client: &Ghost,
    first: &Post,
    report_path: &Path,
    files: ghost_cms_core::filesystem::Files,
) -> Result<()> {
    let mut changed: Value = serde_json::from_str(lexical(first)?)?;
    let node = changed["root"]["children"]
        .as_array_mut()
        .context("fixture children missing")?
        .iter_mut()
        .find(|node| node["type"] == "video")
        .context("video fixture missing")?;
    node["caption"] = "An intentional changed caption.".into();
    client
        .posts()
        .update_editor(
            &first.id,
            first
                .updated_at
                .as_deref()
                .context("fixture revision missing")?,
            &changed.to_string(),
        )
        .await?;
    ensure!(
        verify_repair(client, report_path, files).await.is_err(),
        "changed caption passed repair verification"
    );
    Ok(())
}

/// Exercise metadata-only video repairs with the owned local Ghost instance.
/// # Errors
/// Returns preparation, API, and boundary-check failures.
pub async fn rehearse(client: &Ghost, receipt: &Path, directory: &Path) -> Result<()> {
    let batch: Value = serde_json::from_slice(&std::fs::read(receipt)?)?;
    let posts = batch["posts"]
        .as_object()
        .context("draft ownership missing")?;
    let mut broken = Vec::new();
    for owned in posts.values() {
        let post = client
            .posts()
            .get(owned["id"].as_str().context("owned ID missing")?)
            .await?;
        if editor_videos(lexical(&post)?)?.is_empty() {
            continue;
        }
        broken.push(fixture_state(client, &post, broken.len().is_multiple_of(2)).await?);
    }
    ensure!(
        broken.len() >= 2,
        "video repair rehearsal needs at least two video posts"
    );
    let files = ghost_cms_shared::filesystem::local_files();
    let preview = repair_owned_videos(
        client,
        receipt,
        &directory.join("video-preview.json"),
        false,
        files.clone(),
    )
    .await?;
    ensure!(
        preview.changed() == broken.len(),
        "repair preview missed empty cards"
    );
    for before in &broken {
        let after = client.posts().get(&before.id).await?;
        ensure!(
            after.updated_at == before.updated_at && lexical(&after)? == lexical(before)?,
            "repair preview changed Ghost"
        );
    }
    let report_path = directory.join("video-repair.json");
    let report = repair_owned_videos(client, receipt, &report_path, true, files.clone()).await?;
    ensure!(
        report.changed() == broken.len(),
        "repair didn't update every broken video post"
    );
    verify_repair(client, &report_path, files.clone()).await?;
    let mut repaired = Vec::new();
    for before in &broken {
        let after = client.posts().get(&before.id).await?;
        ensure!(
            after.status == before.status && after.published_at == before.published_at,
            "repair changed publication state or time"
        );
        after.editor_blocks()?;
        repaired.push(after);
    }
    let repeat = repair_owned_videos(
        client,
        receipt,
        &directory.join("video-repeat.json"),
        true,
        files.clone(),
    )
    .await?;
    ensure!(
        repeat.changed() == 0,
        "second video repair wasn't unchanged"
    );
    for before in &repaired {
        ensure!(
            client.posts().get(&before.id).await?.updated_at == before.updated_at,
            "second repair wrote an unchanged post"
        );
    }
    reject_changed_caption(
        client,
        repaired.first().context("repaired fixture missing")?,
        &report_path,
        files,
    )
    .await?;
    progress(format_args!(
        "Verified {} video cards, a read-only preview, preserved scheduled/published state, an unchanged repeat, and rejection of changed captions.",
        report.videos()
    ));
    Ok(())
}
