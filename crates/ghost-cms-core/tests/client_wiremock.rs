//! End-to-end client tests against a mocked Ghost Admin API.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "integration tests panic on failure by design"
)]

use ghost_cms_core::Ghost;
use ghost_cms_core::config::Config;
use ghost_cms_core::domain::{PostInput, PostStatus, TagUpsertInput};
use wiremock::matchers::{body_string_contains, header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "64abc:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn client(server: &MockServer) -> Ghost {
    let cfg = Config::new(server.uri(), TOKEN.to_owned(), "v5.0".to_owned());
    Ghost::new(&cfg).expect("client builds")
}

fn sample_input() -> PostInput {
    PostInput {
        title: "Hello".to_owned(),
        html: Some("<p>hi</p>".to_owned()),
        status: PostStatus::Draft,
        slug: Some("hello".to_owned()),
        ..PostInput::default()
    }
}

#[tokio::test]
async fn site_sends_auth_and_version_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/site/"))
        .and(header_exists("authorization"))
        .and(header_exists("accept-version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "site": { "title": "Example", "url": "https://example.ghost.io", "version": "6.46" }
        })))
        .mount(&server)
        .await;

    let site = client(&server).site().get().await.unwrap();
    assert_eq!(site.title, "Example");
}

#[tokio::test]
async fn find_by_slug_returns_none_when_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("filter", "slug:missing"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "posts": [] })))
        .mount(&server)
        .await;

    assert!(
        client(&server)
            .posts()
            .find_by_slug("missing")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn create_post_hits_source_html_and_returns_post() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("source", "html"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "posts": [{ "id": "p1", "slug": "hello", "status": "draft",
                        "url": "https://example.ghost.io/p/hello/", "updated_at": "2026-06-26T00:00:00.000Z" }]
        })))
        .mount(&server)
        .await;

    let post = client(&server)
        .posts()
        .create(&sample_input())
        .await
        .unwrap();
    assert_eq!(post.id, "p1");
}

#[tokio::test]
async fn update_post_sends_updated_at_for_conflict_detection() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ghost/api/admin/posts/p1/"))
        // The body must carry the updated_at we passed (conflict token).
        .and(body_string_contains("2026-06-26T00:00:00.000Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{ "id": "p1", "slug": "hello", "status": "draft",
                        "updated_at": "2026-06-26T00:05:00.000Z" }]
        })))
        .mount(&server)
        .await;

    let post = client(&server)
        .posts()
        .update("p1", &sample_input(), "2026-06-26T00:00:00.000Z")
        .await
        .unwrap();
    assert_eq!(post.id, "p1");
}

#[tokio::test]
async fn api_error_is_mapped_from_ghost_errors_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/site/"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errors": [{ "message": "Unknown Admin API Key", "type": "UnauthorizedError" }]
        })))
        .mount(&server)
        .await;

    let err = client(&server).site().get().await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("401"), "got: {msg}");
    assert!(msg.contains("Unknown Admin API Key"), "got: {msg}");
}

#[tokio::test]
async fn upload_image_posts_multipart_and_returns_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ghost/api/admin/images/upload/"))
        // A multipart upload sets a content-type with a boundary.
        .and(header_exists("content-type"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "images": [{ "url": "https://example.ghost.io/content/images/x.png" }]
        })))
        .mount(&server)
        .await;

    let url = client(&server)
        .media()
        .upload_image(b"\x89PNG\r\n".to_vec(), "x.png", "image/png")
        .await
        .unwrap();
    assert_eq!(url, "https://example.ghost.io/content/images/x.png");
}

#[tokio::test]
async fn upload_media_uses_media_endpoint_and_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ghost/api/admin/media/upload/"))
        .and(header_exists("content-type"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "media": [{ "url": "https://example.ghost.io/content/media/clip.mp4" }]
        })))
        .mount(&server)
        .await;

    let url = client(&server)
        .media()
        .upload_media(b"\x00\x00".to_vec(), "clip.mp4", "video/mp4")
        .await
        .unwrap();
    assert_eq!(url, "https://example.ghost.io/content/media/clip.mp4");
}

#[tokio::test]
async fn upload_file_uses_files_endpoint_and_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ghost/api/admin/files/upload/"))
        .and(header_exists("content-type"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "files": [{ "url": "https://example.ghost.io/content/files/doc.pdf" }]
        })))
        .mount(&server)
        .await;

    let url = client(&server)
        .media()
        .upload_file(b"%PDF".to_vec(), "doc.pdf", "application/pdf")
        .await
        .unwrap();
    assert_eq!(url, "https://example.ghost.io/content/files/doc.pdf");
}

#[tokio::test]
async fn list_tags_requests_count_and_returns_tags() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/tags/"))
        .and(query_param("include", "count.posts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [{ "id": "t1", "name": "Rust", "slug": "rust",
                       "count": { "posts": 3 } }]
        })))
        .mount(&server)
        .await;

    let tags = client(&server).tags().list(20, 1).await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].slug, "rust");
    assert_eq!(tags[0].count.as_ref().and_then(|c| c.posts), Some(3));
}

#[tokio::test]
async fn update_tag_sends_updated_at_for_conflict_detection() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ghost/api/admin/tags/t1/"))
        .and(body_string_contains("2026-06-26T00:00:00.000Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [{ "id": "t1", "name": "Rust", "slug": "rust" }]
        })))
        .mount(&server)
        .await;

    let input = TagUpsertInput {
        name: "Rust".to_owned(),
        description: Some("Systems programming".to_owned()),
        ..TagUpsertInput::default()
    };
    let tag = client(&server)
        .tags()
        .update("t1", &input, "2026-06-26T00:00:00.000Z")
        .await
        .unwrap();
    assert_eq!(tag.id, "t1");
}
