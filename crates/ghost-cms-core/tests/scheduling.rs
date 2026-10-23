//! Collection scheduling and recovery at the filesystem and HTTP boundaries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ghost_cms_core::preparation::PrepareOptions;
use ghost_cms_core::schedule::{
    apply_release_plan, create_release_plan, stage_drafts, verify_release_plan,
};
use ghost_cms_core::{Config, Ghost};
use ghost_cms_shared::filesystem::{local_files, prepare_collection};
use serde_json::{Value, json};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

#[derive(Default)]
struct Store {
    posts: BTreeMap<String, Value>,
    pages: BTreeSet<String>,
    writes: Vec<Value>,
    fail: Option<String>,
    response_lost: bool,
}

#[derive(Clone)]
struct Server {
    store: Arc<Mutex<Store>>,
    origin: String,
}

impl Respond for Server {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let Ok(mut store) = self.store.lock() else {
            return ResponseTemplate::new(500);
        };
        let path = request.url.path();
        if path.ends_with("/site/") {
            return ResponseTemplate::new(200)
                .set_body_json(json!({"site":{"title":"Test", "url":self.origin}}));
        }
        if path.ends_with("/pages/") {
            let slug = request
                .url
                .query_pairs()
                .find(|(key, _)| key == "filter")
                .and_then(|(_, value)| value.strip_prefix("slug:").map(str::to_owned));
            let pages = slug
                .filter(|slug| store.pages.contains(slug))
                .map(|slug| json!({"id":"page", "slug":slug}))
                .into_iter()
                .collect::<Vec<_>>();
            return ResponseTemplate::new(200).set_body_json(json!({"pages":pages}));
        }
        if request.method == "GET" {
            let slug = request
                .url
                .query_pairs()
                .find(|(k, _)| k == "filter")
                .and_then(|(_, v)| v.strip_prefix("slug:").map(str::to_owned))
                .unwrap_or_else(|| {
                    path.trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned()
                });
            let posts: Vec<_> = store.posts.get(&slug).cloned().into_iter().collect();
            return ResponseTemplate::new(200).set_body_json(json!({"posts":posts}));
        }
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return ResponseTemplate::new(422);
        };
        let input = body["posts"][0].clone();
        let slug = input["slug"].as_str().map_or_else(
            || {
                path.trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            },
            str::to_owned,
        );
        let should_fail = store.fail.as_deref() == Some(&slug);
        if should_fail && !store.response_lost {
            store.fail = None;
            return ResponseTemplate::new(500);
        }
        let mut post = store
            .posts
            .get(&slug)
            .cloned()
            .unwrap_or_else(|| json!({"id":slug, "slug":slug}));
        let Some(object) = post.as_object_mut() else {
            return ResponseTemplate::new(500);
        };
        object.insert(
            "lexical".to_owned(),
            json!(r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"Saved story."}]}]}}"#),
        );
        if let Some(fields) = input.as_object() {
            object.extend(fields.clone());
        }
        object.insert(
            "updated_at".to_owned(),
            json!(format!("revision-{}", store.writes.len() + 1)),
        );
        store.writes.push(input);
        store.posts.insert(slug, post.clone());
        if should_fail {
            store.fail = None;
            return ResponseTemplate::new(500);
        }
        ResponseTemplate::new(200).set_body_json(json!({"posts":[post]}))
    }
}

async fn fixture() -> Result<(MockServer, Server, Ghost), Box<dyn std::error::Error>> {
    let mock = MockServer::start().await;
    let server = Server {
        store: Arc::default(),
        origin: mock.uri(),
    };
    Mock::given(wiremock::matchers::path_regex("/ghost/api/admin/.*"))
        .respond_with(server.clone())
        .mount(&mock)
        .await;
    let client = Ghost::new(&Config::new(
        mock.uri(),
        "64abc:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
        "v5.0".to_owned(),
    ))?;
    Ok((mock, server, client))
}

fn collection(
    root: &std::path::Path,
) -> Result<ghost_cms_core::preparation::PreparedCollection, Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    for slug in ["first", "second"] {
        let path = root.join(format!("{slug}.md"));
        std::fs::write(
            &path,
            format!(
                "---\ntitle: {slug}\nslug: {slug}\nfeature_image: https://example.com/cover.jpg\nfeature_image_alt: A view\nfeature_image_caption: A view\n---\n\nStory.\n"
            ),
        )?;
        paths.push(path);
    }
    Ok(prepare_collection(&paths, PrepareOptions::default())?)
}

#[tokio::test]
async fn foreign_slug_collision_stops_the_whole_batch_before_writes()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .posts
        .insert(
            "second".to_owned(),
            json!({"id":"foreign", "slug":"second", "status":"draft", "updated_at":"other"}),
        );
    let temp = tempfile::tempdir()?;
    let result = stage_drafts(
        &client,
        &collection(temp.path())?,
        &temp.path().join("drafts.json"),
        &server.origin,
        true,
    )
    .await;
    assert!(result.is_err());
    let writes = server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .writes
        .clone();
    assert_eq!(writes, Vec::<Value>::new());
    Ok(())
}

#[tokio::test]
async fn corrected_owned_drafts_can_be_restaged_and_recovered_before_freezing()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    stage_drafts(
        &client,
        &collection(temp.path())?,
        &drafts,
        &server.origin,
        true,
    )
    .await?;
    let previous_receipt = std::fs::read(&drafts)?;
    let first = temp.path().join("first.md");
    let text = std::fs::read_to_string(&first)?;
    std::fs::write(&first, text.replace("Story.", "Corrected story."))?;
    let corrected = prepare_collection(
        &[first, temp.path().join("second.md")],
        PrepareOptions::default(),
    )?;
    assert_eq!(
        stage_drafts(&client, &corrected, &drafts, &server.origin, true).await?,
        2
    );
    {
        let store = server.store.lock().map_err(|_| "store poisoned")?;
        assert_eq!(store.writes.len(), 3);
        assert!(
            store.posts["first"]["html"]
                .as_str()
                .ok_or("HTML missing")?
                .contains("Corrected story.")
        );
        drop(store);
    }
    std::fs::write(&drafts, previous_receipt)?;
    stage_drafts(&client, &corrected, &drafts, &server.origin, true).await?;
    let count = server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .writes
        .len();
    assert_eq!(count, 3);
    create_release_plan(
        &client,
        &drafts,
        &[vec!["first".to_owned()], vec!["second".to_owned()]],
        &temp.path().join("release.json"),
        2_000_000_000,
        local_files(),
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn restaging_refuses_changed_remote_content_before_any_write()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    let collection = collection(temp.path())?;
    stage_drafts(&client, &collection, &drafts, &server.origin, true).await?;
    server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .posts
        .get_mut("second")
        .ok_or("second missing")?["html"] = json!("A remote edit.");
    assert!(
        stage_drafts(&client, &collection, &drafts, &server.origin, true)
            .await
            .is_err()
    );
    let count = server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .writes
        .len();
    assert_eq!(count, 2);
    Ok(())
}

#[tokio::test]
async fn expired_remainder_moves_forward_without_rescheduling_completed_posts()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    let plan = temp.path().join("release.json");
    stage_drafts(
        &client,
        &collection(temp.path())?,
        &drafts,
        &server.origin,
        true,
    )
    .await?;
    let now = 2_000_000_000;
    create_release_plan(
        &client,
        &drafts,
        &[vec!["first".to_owned()], vec!["second".to_owned()]],
        &plan,
        now,
        local_files(),
    )
    .await?;
    server.store.lock().map_err(|_| "store poisoned")?.fail = Some("second".to_owned());
    assert!(
        apply_release_plan(&client, &plan, now, local_files())
            .await
            .is_err()
    );
    let first_time = {
        let mut store = server.store.lock().map_err(|_| "store poisoned")?;
        let first = store.posts.get_mut("first").ok_or("first missing")?;
        first["status"] = json!("published");
        first["updated_at"] = json!("publication-revision");
        let at = first["published_at"].clone();
        drop(store);
        at
    };
    let resumed = now + 86_400;
    apply_release_plan(&client, &plan, resumed, local_files()).await?;
    verify_release_plan(&client, &plan, local_files()).await?;
    let store = server.store.lock().map_err(|_| "store poisoned")?;
    assert_eq!(store.posts["first"]["published_at"], first_time);
    assert_eq!(
        store
            .writes
            .iter()
            .filter(|w| w["status"] == "scheduled")
            .count(),
        2
    );
    let second = store.posts["second"]["published_at"]
        .as_str()
        .ok_or("time missing")?;
    let second_time =
        time::OffsetDateTime::parse(second, &time::format_description::well_known::Rfc3339)?
            .unix_timestamp();
    assert!(second_time >= i64::try_from(resumed + 1_800)?);
    assert!(
        store
            .writes
            .iter()
            .filter(|w| w["status"] == "scheduled")
            .all(|w| w.get("html").is_none() && w.get("title").is_none())
    );
    drop(store);
    Ok(())
}

#[tokio::test]
async fn a_lost_response_is_adopted_without_repeating_the_write()
-> Result<(), Box<dyn std::error::Error>> {
    let (mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    let plan = temp.path().join("release.json");
    stage_drafts(
        &client,
        &collection(temp.path())?,
        &drafts,
        &server.origin,
        true,
    )
    .await?;
    let now = 2_000_000_000;
    create_release_plan(
        &client,
        &drafts,
        &[vec!["first".to_owned(), "second".to_owned()]],
        &plan,
        now,
        local_files(),
    )
    .await?;
    {
        let mut store = server.store.lock().map_err(|_| "store poisoned")?;
        store.fail = Some("first".to_owned());
        store.response_lost = true;
    }
    assert!(
        apply_release_plan(&client, &plan, now, local_files())
            .await
            .is_err()
    );
    apply_release_plan(&client, &plan, now, local_files()).await?;
    {
        let store = server.store.lock().map_err(|_| "store poisoned")?;
        assert_eq!(
            store
                .writes
                .iter()
                .filter(|w| w["status"] == "scheduled")
                .count(),
            2
        );
        assert_eq!(
            store.posts["first"]["published_at"],
            store.posts["second"]["published_at"]
        );
        drop(store);
    }
    let requests = mock
        .received_requests()
        .await
        .ok_or("request log missing")?;
    assert!(requests.iter().all(|r| {
        r.url
            .query_pairs()
            .all(|(key, _)| key != "newsletter" && key != "email_segment")
    }));
    Ok(())
}

#[tokio::test]
async fn page_collisions_and_wrong_sites_refuse_draft_creation()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let collection = collection(temp.path())?;
    let drafts = temp.path().join("drafts.json");
    assert!(
        stage_drafts(
            &client,
            &collection,
            &drafts,
            "https://different.example/",
            true
        )
        .await
        .is_err()
    );
    server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .pages
        .insert("second".to_owned());
    assert!(
        stage_drafts(&client, &collection, &drafts, &server.origin, true)
            .await
            .is_err()
    );
    let count = server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .writes
        .len();
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn changed_metadata_or_local_source_stops_before_any_schedule_write()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    let plan = temp.path().join("release.json");
    stage_drafts(
        &client,
        &collection(temp.path())?,
        &drafts,
        &server.origin,
        true,
    )
    .await?;
    let now = 2_000_000_000;
    create_release_plan(
        &client,
        &drafts,
        &[vec!["first".to_owned()], vec!["second".to_owned()]],
        &plan,
        now,
        local_files(),
    )
    .await?;
    server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .posts
        .get_mut("second")
        .ok_or("second missing")?["feature_image_alt"] = json!("Remote edit");
    assert!(
        apply_release_plan(&client, &plan, now, local_files())
            .await
            .is_err()
    );
    let file = temp.path().join("first.md");
    let original = std::fs::read_to_string(&file)?;
    std::fs::write(&file, original.replace("Story.", "A local edit."))?;
    assert!(
        apply_release_plan(&client, &plan, now, local_files())
            .await
            .is_err()
    );
    let count = server
        .store
        .lock()
        .map_err(|_| "store poisoned")?
        .writes
        .len();
    assert_eq!(count, 2);
    Ok(())
}

#[tokio::test]
async fn readback_rejects_even_a_fractional_change_to_the_frozen_time()
-> Result<(), Box<dyn std::error::Error>> {
    let (_mock, server, client) = fixture().await?;
    let temp = tempfile::tempdir()?;
    let drafts = temp.path().join("drafts.json");
    let plan = temp.path().join("release.json");
    stage_drafts(
        &client,
        &collection(temp.path())?,
        &drafts,
        &server.origin,
        true,
    )
    .await?;
    let now = 2_000_000_000;
    create_release_plan(
        &client,
        &drafts,
        &[vec!["first".to_owned()], vec!["second".to_owned()]],
        &plan,
        now,
        local_files(),
    )
    .await?;
    apply_release_plan(&client, &plan, now, local_files()).await?;
    {
        let mut store = server.store.lock().map_err(|_| "store poisoned")?;
        let post = store.posts.get_mut("first").ok_or("first missing")?;
        let date = post["published_at"].as_str().ok_or("time missing")?;
        post["published_at"] = json!(date.replace('Z', ".001Z"));
        drop(store);
    }
    assert!(
        verify_release_plan(&client, &plan, local_files())
            .await
            .is_err()
    );
    Ok(())
}
