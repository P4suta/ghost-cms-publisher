//! Publication contracts at the filesystem and HTTP boundaries.

use ghost_cms_core::domain::PostStatus;
use ghost_cms_core::frontmatter::FrontMatter;
use ghost_cms_core::{Config, Ghost, PublishOptions};
use ghost_cms_shared::filesystem::publish_file;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "64abc:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn native_editor_readback_ignores_empty_text_containers() -> Result<(), Box<dyn std::error::Error>>
{
    use serde_json::json;
    let post =
        |children: serde_json::Value| -> Result<ghost_cms_core::domain::Post, serde_json::Error> {
            serde_json::from_value(json!({
                "id":"post", "slug":"readback", "status":"draft",
                "lexical":json!({"root":{"type":"root","children":children}}).to_string()
            }))
        };
    let empty = json!({"type":"paragraph","children":[{"type":"text","text":" \n"}]});
    let html = json!({"type":"html","html":"<p>A story.</p>"});
    assert!(post(json!([empty]))?.editor_blocks().is_err());
    let blocks = post(json!([html, empty, {
        "type":"quote","children":[empty]
    }]))?
    .editor_blocks()?;
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks.get("html"), Some(&1));
    let paragraph = json!({"type":"paragraph","children":[{
        "type":"link","children":[{"type":"extended-text","text":"A story."}]
    }]});
    assert_eq!(post(json!([html, paragraph]))?.editor_blocks()?.len(), 2);
    let cards = post(json!([
        {"type":"image","src":"https://example.com/photo.jpg"},
        {"type":"codeblock","code":"let answer = 42;"}
    ]))?
    .editor_blocks()?;
    assert_eq!(cards.len(), 2);
    Ok(())
}

#[tokio::test]
async fn feature_metadata_survives_preview_and_metadata_only_updates()
-> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("post.md");
    let source = |caption: &str| {
        format!(
            "---\ntitle: Hello\nslug: hello\nfeature_image: https://example.com/cover.jpg\nfeature_image_alt: 富士山与湖畔\nfeature_image_caption: '{caption}'\n---\n\nHello.\n"
        )
    };
    std::fs::write(&file, source("湖畔眺望富士山 · Photo by A & B"))?;
    let collection = ghost_cms_shared::filesystem::prepare_collection(
        std::slice::from_ref(&file),
        ghost_cms_core::preparation::PrepareOptions::default(),
    )?;
    let preview = temp.path().join("preview");
    collection.write_preview(&preview)?;
    let html = std::fs::read_to_string(preview.join("hello/index.html"))?;
    assert!(html.contains("alt=\"富士山与湖畔\""));
    assert!(html.contains("湖畔眺望富士山 · Photo by A &amp; B"));
    existing(&server, "t1").await;
    updates(&server, 2).await;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let client = client(&server)?;
    publish_file(&client, &file, &options).await?;
    std::fs::write(&file, source("湖畔眺望富士山 · Photo by C"))?;
    publish_file(&client, &file, &options).await?;
    let requests = server.received_requests().await.ok_or("no request log")?;
    let payloads = requests
        .iter()
        .filter(|r| r.method == "PUT")
        .map(|r| serde_json::from_slice::<serde_json::Value>(&r.body))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(payloads.len(), 2);
    let imported_html = payloads[0]["posts"][0]["html"]
        .as_str()
        .ok_or("HTML payload missing")?;
    assert!(!imported_html.contains("kg-card-begin: html"));
    assert_eq!(payloads[0]["posts"][0]["feature_image_alt"], "富士山与湖畔");
    assert_eq!(
        payloads[1]["posts"][0]["feature_image_caption"],
        "湖畔眺望富士山 · Photo by C"
    );
    Ok(())
}

fn client(server: &MockServer) -> ghost_cms_core::Result<Ghost> {
    Ghost::new(&Config::new(
        server.uri(),
        TOKEN.to_owned(),
        "v5.0".to_owned(),
    ))
}

async fn existing(server: &MockServer, revision: &str) {
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{"id":"p1", "slug":"hello", "status":"draft", "updated_at":revision}]
        })))
        .mount(server)
        .await;
}

async fn updates(server: &MockServer, count: u64) {
    Mock::given(method("PUT"))
        .and(path("/ghost/api/admin/posts/p1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{"id":"p1", "slug":"hello", "status":"draft", "updated_at":"t1"}]
        })))
        .expect(count)
        .mount(server)
        .await;
}

fn write_post(dir: &std::path::Path, body: &str) -> std::io::Result<std::path::PathBuf> {
    let file = dir.join("post.md");
    std::fs::write(
        &file,
        format!("---\ntitle: Hello\nslug: hello\n---\n\n{body}\n"),
    )?;
    Ok(file)
}

#[tokio::test]
async fn unchanged_local_images_skip_uploads_and_updates() -> Result<(), Box<dyn std::error::Error>>
{
    let server = MockServer::start().await;
    existing(&server, "t1").await;
    updates(&server, 1).await;
    Mock::given(method("POST"))
        .and(path("/ghost/api/admin/images/upload/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "images":[{"url":"https://example.com/content/images/picture.png"}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("picture.png"), b"image bytes")?;
    let post = write_post(temp.path(), "![Picture](picture.png)")?;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let client = client(&server)?;
    publish_file(&client, &post, &options).await?;
    let outcome = publish_file(&client, &post, &options).await?;
    assert!(matches!(
        outcome,
        ghost_cms_core::PublishOutcome::SkippedUnchanged { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn dry_run_rejects_a_missing_image_before_network() -> Result<(), Box<dyn std::error::Error>>
{
    let server = MockServer::start().await;
    let temp = tempfile::tempdir()?;
    let post = write_post(temp.path(), "![Missing](missing.png)")?;
    let options = PublishOptions {
        dry_run: true,
        ..PublishOptions::default()
    };
    let result = publish_file(&client(&server)?, &post, &options).await;
    assert!(result.is_err());
    assert!(
        server
            .received_requests()
            .await
            .ok_or("no request log")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn cache_receipts_are_scoped_to_the_destination_site()
-> Result<(), Box<dyn std::error::Error>> {
    let first = MockServer::start().await;
    let second = MockServer::start().await;
    for server in [&first, &second] {
        existing(server, "t1").await;
        updates(server, 1).await;
    }
    let temp = tempfile::tempdir()?;
    let post = write_post(temp.path(), "Hello.")?;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    publish_file(&client(&first)?, &post, &options).await?;
    publish_file(&client(&second)?, &post, &options).await?;
    Ok(())
}

#[tokio::test]
async fn a_remote_edit_requires_an_explicit_force() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    existing(&server, "t1").await;
    updates(&server, 1).await;
    let temp = tempfile::tempdir()?;
    let post = write_post(temp.path(), "Hello.")?;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let client = client(&server)?;
    publish_file(&client, &post, &options).await?;
    server.reset().await;
    existing(&server, "remote-edit").await;
    let result = publish_file(&client, &post, &options).await;
    assert!(
        result.is_err(),
        "a changed remote revision must not produce an unchanged skip"
    );
    assert!(
        server
            .received_requests()
            .await
            .ok_or("no request log")?
            .iter()
            .all(|r| r.method == "GET")
    );
    Ok(())
}

#[test]
fn unknown_publication_status_is_invalid_input() {
    let mut front = FrontMatter::new("Hello".to_owned(), "hello".to_owned());
    front.status = PostStatus::Unknown;
    assert!(front.validate().is_err());
}

#[tokio::test]
async fn corrupt_or_locked_state_refuses_network_writes() -> Result<(), Box<dyn std::error::Error>>
{
    let server = MockServer::start().await;
    let temp = tempfile::tempdir()?;
    let post = write_post(temp.path(), "Hello.")?;
    let state = temp.path().join("state.json");
    let options = PublishOptions {
        state_path: Some(state.clone()),
        ..PublishOptions::default()
    };
    std::fs::write(&state, b"{broken")?;
    assert!(
        publish_file(&client(&server)?, &post, &options)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&state)?, b"{broken");
    std::fs::write(&state, br#"{"version":2,"sites":{}}"#)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(state.with_extension("json.lock"))?;
    lock.try_lock()?;
    assert!(
        publish_file(&client(&server)?, &post, &options)
            .await
            .is_err()
    );
    assert!(
        server
            .received_requests()
            .await
            .ok_or("no request log")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn a_failed_post_write_reuses_completed_media_uploads()
-> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    existing(&server, "t1").await;
    Mock::given(method("POST")).and(path("/ghost/api/admin/images/upload/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"images":[{"url":"https://example.com/content/images/picture.png"}]})))
        .expect(1).mount(&server).await;
    Mock::given(method("PUT"))
        .and(path("/ghost/api/admin/posts/p1/"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("picture.png"), b"image bytes")?;
    let post = write_post(temp.path(), "![Picture](picture.png)")?;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let client = client(&server)?;
    assert!(publish_file(&client, &post, &options).await.is_err());
    server.verify().await;
    server.reset().await;
    existing(&server, "t1").await;
    updates(&server, 1).await;
    assert!(matches!(
        publish_file(&client, &post, &options).await?,
        ghost_cms_core::PublishOutcome::Updated { .. }
    ));
    assert!(
        server
            .received_requests()
            .await
            .ok_or("no request log")?
            .iter()
            .all(|r| r.method != "POST")
    );
    Ok(())
}

#[tokio::test]
async fn explicit_force_updates_a_changed_remote_revision() -> Result<(), Box<dyn std::error::Error>>
{
    let server = MockServer::start().await;
    existing(&server, "t1").await;
    updates(&server, 1).await;
    let temp = tempfile::tempdir()?;
    let post = write_post(temp.path(), "Hello.")?;
    let mut options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let client = client(&server)?;
    publish_file(&client, &post, &options).await?;
    server.reset().await;
    existing(&server, "remote-edit").await;
    updates(&server, 1).await;
    options.force = true;
    assert!(matches!(
        publish_file(&client, &post, &options).await?,
        ghost_cms_core::PublishOutcome::Updated { .. }
    ));
    let requests = server.received_requests().await.ok_or("no request log")?;
    let update = requests
        .iter()
        .find(|r| r.method == "PUT")
        .ok_or("missing update")?;
    let body: serde_json::Value = serde_json::from_slice(&update.body)?;
    assert_eq!(body["posts"][0]["updated_at"], "remote-edit");
    Ok(())
}
