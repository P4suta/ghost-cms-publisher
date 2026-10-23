//! End-to-end CLI tests: static help/error UX (no network) and rendered output
//! against a mocked Ghost Admin API. We assert on `--color never` output so the
//! snapshots and substring checks are free of ANSI escapes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "integration tests panic on failure by design"
)]

use std::process::Output;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A well-formed Staff Access Token (`{id}:{hex-secret}`) the client accepts.
const TOKEN: &str = "64abc:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Build the CLI command with a clean, deterministic environment.
fn cli() -> Command {
    let mut cmd = Command::cargo_bin("ghost-cms").unwrap();
    cmd.env_remove("GHOST_ADMIN_API_URL")
        .env_remove("GHOST_STAFF_TOKEN")
        .env_remove("GHOST_ACCEPT_VERSION")
        .env_remove("GHOST_BLOG_DIR")
        .env_remove("GHOST_LOG")
        .env_remove("NO_COLOR")
        .arg("--color")
        .arg("never");
    cmd
}

/// Run the CLI against `server` with the given args, returning captured output.
async fn run(server: &MockServer, args: &[&str]) -> Output {
    let uri = server.uri();
    let owned: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    tokio::task::spawn_blocking(move || {
        cli()
            .args(["--api-url", &uri, "--token", TOKEN])
            .args(&owned)
            .output()
            .unwrap()
    })
    .await
    .unwrap()
}

/// Mount a single post returned by a `slug:<slug>` lookup.
async fn mount_post(server: &MockServer, slug: &str) {
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("filter", format!("slug:{slug}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{
                "id": "p1", "slug": slug, "status": "published",
                "title": "Hello World", "url": "https://example.ghost.io/p/hello/",
                "updated_at": "2026-06-26T00:00:00.000Z"
            }]
        })))
        .mount(server)
        .await;
}

/// Normalize captured stdout to `\n` line endings for stable snapshots.
fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

// --- static UX (no network) ---------------------------------------------------

#[test]
fn top_help_lists_worked_examples() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Examples:"))
        .stdout(contains("ghost-cms publish blog/posts/"))
        .stdout(contains("Auto-publish files as you save them"));
}

#[tokio::test]
async fn invalid_batch_rejects_every_post_before_network() {
    let server = MockServer::start().await;
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("first.md"),
        "---\ntitle: First\nslug: first-post\n---\nText.\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("second.md"),
        "---\ntitle: Second\nslug: second-post\n---\n![Missing](missing.png)\n",
    )
    .unwrap();
    let result = run(&server, &["publish", temp.path().to_str().unwrap()]).await;
    assert!(!result.status.success());
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn new_preserves_quoted_unicode_metadata_and_rejects_escaping_or_existing_paths() {
    let temp = tempfile::tempdir().unwrap();
    let blog = temp.path().join("blog with spaces");
    let title = "日本旅行: \"中文\"\nA second line";
    let tag = "Travel, [Japan]";
    cli()
        .args([
            "--blog-dir",
            blog.to_str().unwrap(),
            "new",
            title,
            "--slug",
            "quoted-japan-trip",
            "--tag",
            tag,
            "--no-edit",
        ])
        .assert()
        .success();
    let file = blog.join("posts/quoted-japan-trip.md");
    let before = std::fs::read_to_string(&file).unwrap();
    let post = ghost_cms_core::markdown::parse(&before).unwrap();
    assert_eq!(post.front.title, title);
    assert_eq!(post.front.tags, vec![tag.to_owned()]);
    for slug in ["../../outside", "quoted-japan-trip"] {
        cli()
            .args([
                "--blog-dir",
                blog.to_str().unwrap(),
                "new",
                "Another title",
                "--slug",
                slug,
                "--no-edit",
            ])
            .assert()
            .failure();
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), before);
    assert!(!temp.path().join("outside.md").exists());
    assert_eq!(std::fs::read_dir(blog.join("posts")).unwrap().count(), 1);
}

#[test]
fn publish_help_lists_examples() {
    cli()
        .args(["publish", "--help"])
        .assert()
        .success()
        .stdout(contains("Examples:"))
        .stdout(contains("--dry-run"));
}

#[test]
fn missing_token_is_a_friendly_error() {
    // The token is never read from a config file, so removing it from the
    // environment guarantees the "set a token" remediation regardless of host.
    cli()
        .args(["--api-url", "https://example.ghost.io", "get", "hello"])
        .assert()
        .failure()
        .stderr(contains("GHOST_STAFF_TOKEN").or(contains("login")));
}

#[test]
fn check_and_preview_work_without_a_site_or_token() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let post = temp.path().join("post.md");
    std::fs::write(
        &post,
        "---\ntitle: 中国語の記事\nslug: chinese-post\n---\n\n## 道順\n\n[戻る](#道順)\n",
    )?;
    cli().arg("check").arg(&post).assert().success();
    let preview = temp.path().join("preview");
    cli()
        .arg("preview")
        .arg(&post)
        .arg("--output")
        .arg(&preview)
        .assert()
        .success();
    assert!(preview.join("index.html").is_file());
    let rendered = std::fs::read_to_string(preview.join("chinese-post/index.html"))?;
    assert!(rendered.contains("中国語の記事"));
    assert!(rendered.contains("id=\"道順\""));
    cli()
        .arg("preview")
        .arg(&post)
        .arg("--output")
        .arg(&preview)
        .assert()
        .failure();
    Ok(())
}

// --- rendered output (mocked API) --------------------------------------------

#[tokio::test]
async fn get_renders_a_detail_block() {
    let server = MockServer::start().await;
    mount_post(&server, "hello").await;
    let out = run(&server, &["get", "hello"]).await;
    assert!(out.status.success());
    insta::assert_snapshot!("get_detail", stdout(&out));
}

#[tokio::test]
async fn get_json_is_machine_readable() {
    let server = MockServer::start().await;
    mount_post(&server, "hello").await;
    let out = run(&server, &["--json", "get", "hello"]).await;
    assert!(out.status.success());
    insta::assert_snapshot!("get_json", stdout(&out));
}

#[tokio::test]
async fn list_renders_a_table_with_header_and_rows() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("limit", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{
                "id": "p1", "slug": "hello", "status": "published",
                "title": "Hello World", "updated_at": "2026-06-26T00:00:00.000Z"
            }]
        })))
        .mount(&server)
        .await;
    let out = run(&server, &["list"]).await;
    assert!(out.status.success());
    let stdout = stdout(&out);
    assert!(stdout.contains("slug"), "header missing: {stdout}");
    assert!(stdout.contains("hello"), "row missing: {stdout}");
}

#[tokio::test]
async fn tags_list_renders_counts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/tags/"))
        .and(query_param("include", "count.posts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tags": [{ "id": "t1", "name": "Rust", "slug": "rust", "count": { "posts": 3 } }]
        })))
        .mount(&server)
        .await;
    let out = run(&server, &["tags", "list"]).await;
    assert!(out.status.success());
    let stdout = stdout(&out);
    assert!(stdout.contains("rust"), "tag row missing: {stdout}");
    assert!(stdout.contains('3'), "count missing: {stdout}");
}

#[tokio::test]
async fn get_unknown_slug_suggests_a_near_miss() {
    let server = MockServer::start().await;
    // The exact lookup misses…
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("filter", "slug:helo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "posts": [] })))
        .mount(&server)
        .await;
    // …and the candidate scan offers `hello`.
    Mock::given(method("GET"))
        .and(path("/ghost/api/admin/posts/"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "posts": [{ "id": "p1", "slug": "hello", "status": "published" }]
        })))
        .mount(&server)
        .await;
    let out = run(&server, &["get", "helo"]).await;
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("did you mean `hello`?"),
        "no suggestion in: {stderr}"
    );
}
