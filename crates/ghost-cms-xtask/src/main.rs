//! Rehearse real Ghost publication using only an owned loopback container.

/// Source-bound verification receipts.
pub mod evidence;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use ghost_cms_core::preparation::{PrepareOptions, PreparedCollection};
use ghost_cms_core::publish::publish_prepared;
use ghost_cms_core::{Config, Ghost, PublishOptions, PublishOutcome};
use percent_encoding::percent_decode_str;
use scraper::{Html, Selector};
use serde::Deserialize;

const IMAGE: &str = "ghost@sha256:cd617a0901e659cda978b9063f449535d105f5f1a0340ffac415db2c42d22606";
const READY_ATTEMPTS: u16 = 120;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const PROGRESS_INTERVAL: usize = 10;
const REHEARSAL_DEADLINE: Duration = Duration::from_mins(15);
const LOOPBACK_HOST: &str = "127.0.0.1";
const CONTAINER_HOST: &str = "0.0.0.0";
const CONTAINER_PORT: u16 = 2368;
const SQLITE_FILE: &str = "/var/lib/ghost/content/data/ghost.db";

#[expect(
    clippy::print_stderr,
    reason = "this developer command reports progress and cleanup failures to stderr"
)]
fn progress(message: std::fmt::Arguments<'_>) {
    eprintln!("{message}");
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Refuse stale, incomplete, or unsuccessful policy proof receipts.
    VerifyEvidence,
    /// Publish to a fresh Ghost 6.72.0 container, read back, repeat, and clean up.
    Rehearse {
        /// Post files or directories containing Markdown posts.
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
        /// Loopback port acquired atomically by Docker.
        #[arg(long, default_value_t = 24689)]
        port: u16,
    },
}

fn docker(arguments: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(arguments)
        .output()
        .context("running Docker")?;
    ensure!(
        output.status.success(),
        "Docker {} failed: {}",
        arguments.first().unwrap_or(&"command"),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

struct Container {
    id: String,
}

impl Drop for Container {
    fn drop(&mut self) {
        if let Err(error) = docker(&["rm", "--force", "--volumes", &self.id]) {
            progress(format_args!(
                "Owned rehearsal container {} needs cleanup: {error}",
                self.id
            ));
        }
    }
}

struct Network {
    id: String,
}

impl Drop for Network {
    fn drop(&mut self) {
        if let Err(error) = docker(&["network", "rm", &self.id]) {
            progress(format_args!(
                "Owned rehearsal network {} needs cleanup: {error}",
                self.id
            ));
        }
    }
}

#[derive(Deserialize)]
struct Integrations {
    integrations: Vec<Integration>,
}
#[derive(Deserialize)]
struct Integration {
    api_keys: Vec<ApiKey>,
}
#[derive(Deserialize)]
struct ApiKey {
    id: String,
    secret: String,
    #[serde(rename = "type")]
    kind: String,
}

fn checked(response: reqwest::Response, phase: &str) -> Result<reqwest::Response> {
    ensure!(
        response.status().is_success(),
        "{phase}: HTTP {}",
        response.status()
    );
    Ok(response)
}

async fn bootstrap(http: &reqwest::Client, origin: &str) -> Result<Config> {
    let base = format!("{origin}/ghost/api/admin");
    let mut ready = false;
    let mut observation = String::from("no response");
    for _ in 0..READY_ATTEMPTS {
        match http
            .get(format!("{base}/authentication/setup/"))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                ready = true;
                break;
            },
            Ok(response) => observation = response.status().to_string(),
            Err(error) => observation = error.to_string(),
        }
        tokio::time::sleep(READY_POLL_INTERVAL).await;
    }
    ensure!(
        ready,
        "Ghost readiness remained inconclusive after {READY_ATTEMPTS} probes: {observation}"
    );
    let mut random = [0u8; 32];
    getrandom::fill(&mut random)
        .map_err(|e| anyhow::anyhow!("cannot generate rehearsal credentials: {e}"))?;
    let password = hex::encode(random);
    checked(http.post(format!("{base}/authentication/setup/")).header("Origin", origin)
        .json(&serde_json::json!({"setup":[{"name":"Local Review", "email":"review@example.invalid", "password":password, "blogTitle":"Publication rehearsal"}]})).send().await?, "Ghost setup")?;
    let session = checked(
        http.post(format!("{base}/session/"))
            .header("Origin", origin)
            .json(&serde_json::json!({"username":"review@example.invalid", "password":password}))
            .send()
            .await?,
        "Ghost session",
    )?;
    let cookies = session
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .map(|h| {
            h.to_str()
                .map(|s| s.split(';').next().unwrap_or_default().to_owned())
        })
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join("; ");
    ensure!(!cookies.is_empty(), "Ghost session returned no cookie");
    let response = checked(
        http.post(format!("{base}/integrations/?include=api_keys"))
            .header("Origin", origin)
            .header(reqwest::header::COOKIE, cookies)
            .json(&serde_json::json!({"integrations":[{"name":"Local publication rehearsal"}]}))
            .send()
            .await?,
        "Ghost integration",
    )?;
    let integrations: Integrations = response.json().await?;
    let key = integrations
        .integrations
        .into_iter()
        .flat_map(|i| i.api_keys)
        .find(|k| k.kind == "admin")
        .context("Ghost admin key missing")?;
    ensure!(
        key.secret.starts_with(&format!("{}:", key.id)),
        "Ghost admin key has an unexpected identity"
    );
    Ok(Config::new(
        origin.to_owned(),
        key.secret,
        "v6.0".to_owned(),
    ))
}

#[derive(Default)]
struct Content {
    text: String,
    code: Vec<String>,
    images: Vec<String>,
    media: Vec<(String, String)>,
    posters: Vec<String>,
    captions: Vec<String>,
    bookmarks: usize,
    tables: usize,
    ids: BTreeSet<String>,
    links: Vec<String>,
}

fn content(html: &str) -> Result<Content> {
    let document = Html::parse_fragment(html);
    let selector =
        |s| Selector::parse(s).map_err(|e| anyhow::anyhow!("invalid verification selector: {e}"));
    let text = document
        .root_element()
        .text()
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    Ok(Content {
        text,
        code: document
            .select(&selector("pre code")?)
            .map(|e| e.text().collect())
            .collect(),
        images: document
            .select(&selector("img[src]")?)
            .filter_map(|e| e.attr("src").map(str::to_owned))
            .collect(),
        media: document
            .select(&selector("video[src], audio[src]")?)
            .filter_map(|e| {
                e.attr("src")
                    .map(|src| (e.value().name().to_owned(), src.to_owned()))
            })
            .collect(),
        posters: document
            .select(&selector("video[poster]")?)
            .filter_map(|e| e.attr("poster").map(str::to_owned))
            .collect(),
        captions: document
            .select(&selector("figcaption")?)
            .map(|e| e.text().collect())
            .collect(),
        bookmarks: document.select(&selector(".kg-bookmark-card")?).count(),
        tables: document.select(&selector("table")?).count(),
        ids: document
            .select(&selector("[id]")?)
            .filter_map(|e| e.attr("id").map(str::to_owned))
            .collect(),
        links: document
            .select(&selector("a[href]")?)
            .filter_map(|e| e.attr("href").map(str::to_owned))
            .collect(),
    })
}

struct Environment {
    container: Container,
    _network: Network,
    origin: String,
}

fn create_environment(name: &str, port: u16) -> Result<Environment> {
    let network = Network {
        id: docker(&["network", "create", name])?,
    };
    let origin = format!("http://{LOOPBACK_HOST}:{port}");
    let binding = format!("{LOOPBACK_HOST}:{port}:{CONTAINER_PORT}");
    let container = Container {
        id: docker(&[
            "create",
            "--name",
            name,
            "--network",
            &network.id,
            "--cpus",
            "2",
            "--memory",
            "1g",
            "--memory-swap",
            "1g",
            "--pids-limit",
            "256",
            "--publish",
            &binding,
            "--env",
            "NODE_ENV=development",
            "--env",
            &format!("server__host={CONTAINER_HOST}"),
            "--env",
            &format!("url={origin}"),
            "--env",
            "database__client=sqlite3",
            "--env",
            &format!("database__connection__filename={SQLITE_FILE}"),
            "--env",
            "mail__transport=SMTP",
            "--env",
            &format!("mail__options__host={LOOPBACK_HOST}"),
            "--env",
            "mail__options__port=1",
            "--env",
            "security__staffDeviceVerification=false",
            IMAGE,
        ])?,
    };
    docker(&["start", &container.id])?;
    Ok(Environment {
        container,
        _network: network,
        origin,
    })
}

async fn rehearse(files: Vec<PathBuf>, port: u16) -> Result<()> {
    ensure!(port != 0, "a fixed nonzero loopback port is required");
    let files = ghost_cms_shared::paths::expand_inputs(&files)?;
    let collection =
        ghost_cms_shared::filesystem::prepare_collection(&files, PrepareOptions::default())?;
    let temp = tempfile::tempdir()?;
    let name = format!(
        "ghost-cms-rehearsal-{}",
        temp.path()
            .file_name()
            .context("rehearsal identity")?
            .to_string_lossy()
            .trim_start_matches('.')
    );
    let environment = create_environment(&name, port)?;
    let origin = &environment.origin;
    progress(format_args!(
        "Rehearsing {} posts against Ghost 6.72.0 on loopback.",
        collection.posts().len()
    ));
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(HTTP_TIMEOUT)
        .build()?;
    let config = bootstrap(&http, origin).await.map_err(|error| {
        let logs = docker(&["logs", "--tail", "60", &environment.container.id]).unwrap_or_else(
            |diagnostic| format!("Container diagnostics unavailable: {diagnostic}"),
        );
        error.context(logs)
    })?;
    let client = Ghost::new(&config)?;
    let options = PublishOptions {
        state_path: Some(temp.path().join("state.json")),
        ..PublishOptions::default()
    };
    let pages = publish_and_readback(&client, &collection, &options).await?;
    let checked_media = verify_pages(&http, origin, &pages).await?;
    for prepared in collection.posts() {
        ensure!(
            matches!(
                publish_prepared(&client, prepared, &options).await?,
                PublishOutcome::SkippedUnchanged { .. }
            ),
            "second rehearsal changed {}",
            prepared.front().slug
        );
    }
    progress(format_args!(
        "Verified {} posts, {checked_media} local media URLs, preserved text/code/tables/fragments, and an unchanged second publication.",
        pages.len()
    ));
    Ok(())
}

async fn publish_and_readback(
    client: &Ghost,
    collection: &PreparedCollection,
    options: &PublishOptions,
) -> Result<BTreeMap<String, Content>> {
    let mut pages = BTreeMap::new();
    for prepared in collection.posts() {
        let outcome = publish_prepared(client, prepared, options).await?;
        ensure!(
            matches!(outcome, PublishOutcome::Created { .. }),
            "first rehearsal did not create {}",
            prepared.front().slug
        );
        let post = client
            .posts()
            .find_by_slug(&prepared.front().slug)
            .await?
            .context("created post missing")?;
        let actual = content(post.html.as_deref().context("Ghost returned no HTML")?)?;
        let expected = content(prepared.rendered_html())?;
        ensure!(
            actual.text == expected.text,
            "Ghost changed text in {}",
            post.slug
        );
        ensure!(
            actual.code == expected.code,
            "Ghost changed code in {}",
            post.slug
        );
        ensure!(
            actual.images.len() == expected.images.len(),
            "Ghost lost images in {}",
            post.slug
        );
        ensure!(
            actual.tables == expected.tables,
            "Ghost lost tables in {}",
            post.slug
        );
        ensure!(
            actual.captions == expected.captions && actual.bookmarks == expected.bookmarks,
            "Ghost changed captions or link cards in {}",
            post.slug
        );
        ensure!(
            actual
                .media
                .iter()
                .map(|(kind, _)| kind)
                .eq(expected.media.iter().map(|(kind, _)| kind))
                && actual.posters.len() == expected.posters.len(),
            "Ghost changed media players or posters in {}",
            post.slug
        );
        pages.insert(post.slug, actual);
        if pages.len().is_multiple_of(PROGRESS_INTERVAL) || pages.len() == collection.posts().len()
        {
            progress(format_args!(
                "Read back {}/{} posts.",
                pages.len(),
                collection.posts().len()
            ));
        }
    }
    Ok(pages)
}

async fn verify_pages(
    http: &reqwest::Client,
    origin: &str,
    pages: &BTreeMap<String, Content>,
) -> Result<usize> {
    let mut checked_media = BTreeSet::new();
    for (slug, page) in pages {
        for link in &page.links {
            let url = reqwest::Url::parse(&format!("{origin}/{slug}/"))?.join(link)?;
            if url.origin().ascii_serialization() != origin {
                continue;
            }
            let target_slug = url.path().trim_matches('/');
            if url.path().starts_with("/content/") && checked_media.insert(url.to_string()) {
                checked(
                    http.head(url.clone()).send().await?,
                    "Ghost attachment readback",
                )?;
            }
            if let (Some(target), Some(fragment)) = (
                pages.get(target_slug),
                url.fragment().filter(|f| !f.is_empty()),
            ) {
                let decoded = percent_decode_str(fragment).decode_utf8()?;
                ensure!(
                    target.ids.contains(decoded.as_ref()),
                    "Ghost lost heading #{decoded} in {target_slug}, linked from {slug}"
                );
            }
        }
        for image in page
            .images
            .iter()
            .chain(&page.posters)
            .chain(page.media.iter().map(|(_, url)| url))
        {
            if image.starts_with(origin) && checked_media.insert(image.clone()) {
                checked(http.head(image).send().await?, "Ghost image readback")?;
            }
        }
    }
    Ok(checked_media.len())
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Action::VerifyEvidence => evidence::run(&std::env::current_dir()?),
        Action::Rehearse { files, port } => tokio::select! {
            result = rehearse(files, port) => result,
            signal = tokio::signal::ctrl_c() => { signal?; anyhow::bail!("rehearsal interrupted; verification is inconclusive") },
            () = tokio::time::sleep(REHEARSAL_DEADLINE) => anyhow::bail!("rehearsal exceeded its diagnostic deadline; verification is inconclusive"),
        },
    }
}
