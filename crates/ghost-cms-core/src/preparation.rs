//! Offline collection validation and rendering shared by all publishers.

mod preview;
mod rich;
pub use rich::RenderReport;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use comrak::nodes::NodeValue;
use comrak::{Arena, Options, format_html, parse_document};
use percent_encoding::percent_decode_str;
use serde::Serialize;

use crate::constants::MAX_PAYLOAD_BYTES;
use crate::domain::PostStatus;
use crate::error::{CoreError, Result};
use crate::filesystem::Files;
use crate::frontmatter::FrontMatter;
use crate::hash::{bytes_hash, content_hash};
use crate::markdown;
use crate::media;

const ASSET_PREFIX: &str = "https://ghost-cms.invalid/assets/";
const ROUTE_SLUG: &str = concat!("{", "slug", "}");

/// Options used consistently by local previews and publication.
#[derive(Debug, Clone)]
pub struct PrepareOptions {
    /// Permit explicit HTML instead of rejecting it.
    pub allow_raw_html: bool,
    /// Override each file's publication status.
    pub status_override: Option<PostStatus>,
    /// Ghost's public route, with one `{slug}` placeholder.
    pub route_template: String,
}

impl Default for PrepareOptions {
    fn default() -> Self {
        Self {
            allow_raw_html: false,
            status_override: None,
            route_template: "/{slug}/".to_owned(),
        }
    }
}

/// A file-specific validation failure.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Input file containing the failure.
    pub file: PathBuf,
    /// Actionable explanation.
    pub message: String,
}

/// A post's offline inventory.
#[derive(Debug, Clone, Serialize)]
pub struct PostReport {
    /// Markdown input path.
    pub file: PathBuf,
    /// Publication title.
    pub title: String,
    /// Unique publication slug.
    pub slug: String,
    /// Effective publication status.
    pub status: PostStatus,
    /// Rendered HTML size before replacing local media URLs.
    pub html_bytes: usize,
    /// Number of distinct local media objects.
    pub local_assets: usize,
    /// References to websites outside the collection, without availability checks.
    pub external_references: BTreeSet<String>,
    /// Visible captions, missing captions, and standalone link cards.
    pub rendering: RenderReport,
}

/// Complete local validation results, including every failed input.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CheckReport {
    /// Successfully parsed posts.
    pub posts: Vec<PostReport>,
    /// Blocking failures.
    pub errors: Vec<Diagnostic>,
}

/// A collection that failed offline preparation.
#[derive(Debug, thiserror::Error)]
#[error("publication validation failed: {details}")]
pub struct PreparationError {
    /// Structured diagnostics for all inputs.
    pub report: CheckReport,
    details: String,
}

/// Upload endpoint associated with an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetKind {
    Image,
    Media,
    File,
}

/// Content identity of a validated local asset.
#[derive(Debug, Clone)]
pub(crate) struct Asset {
    pub(crate) path: PathBuf,
    pub(crate) key: String,
    pub(crate) digest: String,
    pub(crate) mime: String,
    pub(crate) kind: AssetKind,
    pub(crate) size: u64,
    files: Files,
}

impl Asset {
    pub(crate) fn read_verified(&self) -> Result<Vec<u8>> {
        let bytes = self.files.read(&self.path)?;
        if bytes.len() as u64 != self.size || bytes_hash(&bytes).to_string() != self.digest {
            return Err(CoreError::FrontMatter(format!(
                "media changed after validation: {}",
                self.path.display()
            )));
        }
        Ok(bytes)
    }
}

/// A locally checked post with immutable Markdown and content identities.
#[derive(Debug)]
pub struct PreparedPost {
    pub(crate) files: Files,
    pub(crate) front: FrontMatter,
    pub(crate) body_md: String,
    pub(crate) allow_raw_html: bool,
    pub(crate) rewrites: BTreeMap<String, String>,
    pub(crate) assets: BTreeMap<String, Asset>,
    pub(crate) source_hash: String,
    pub(crate) html: String,
    path: PathBuf,
    headings: BTreeSet<String>,
    external: BTreeSet<String>,
    rendering: RenderReport,
}

impl PreparedPost {
    /// Prepared HTML with content identities in place of uploaded media URLs.
    #[must_use]
    pub fn rendered_html(&self) -> &str {
        &self.html
    }
    /// Original Markdown path used for diagnostics.
    #[must_use]
    pub fn source_path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn from_parts(
        path: PathBuf,
        mut front: FrontMatter,
        body_md: String,
        options: &PrepareOptions,
        files: Files,
    ) -> Result<Self> {
        if let Some(status) = options.status_override {
            front.status = status;
        }
        front.validate()?;
        if body_md.trim().is_empty() {
            return Err(CoreError::FrontMatter(
                "post body must not be empty".to_owned(),
            ));
        }
        if options.route_template.matches(ROUTE_SLUG).count() != 1
            || !options.route_template.starts_with('/')
            || options.route_template.starts_with("//")
            || options.route_template.chars().any(char::is_control)
            || options.route_template.contains(['?', '#', '\\'])
        {
            return Err(CoreError::FrontMatter(
                "route template must be an absolute site path with one {slug} placeholder"
                    .to_owned(),
            ));
        }
        let arena = Arena::new();
        let parse_options = comrak_options(options.allow_raw_html);
        let root = parse_document(&arena, &body_md, &parse_options);
        let mut anchorizer = comrak::Anchorizer::new();
        let mut headings = BTreeSet::new();
        for node in root.descendants() {
            match &node.data().value {
                NodeValue::Heading(_) => {
                    headings.insert(anchorizer.anchorize(&comrak::html::collect_text(node)));
                },
                NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) if !options.allow_raw_html => {
                    return Err(CoreError::FrontMatter(
                        "raw HTML requires --allow-raw-html; put literal examples in code"
                            .to_owned(),
                    ));
                },
                NodeValue::HtmlBlock(block) => validate_raw_html(&block.literal)?,
                NodeValue::HtmlInline(html) => validate_raw_html(html)?,
                _ => {},
            }
        }
        Ok(Self {
            files,
            front,
            body_md,
            allow_raw_html: options.allow_raw_html,
            rewrites: BTreeMap::new(),
            assets: BTreeMap::new(),
            source_hash: String::new(),
            html: String::new(),
            path,
            headings,
            external: BTreeSet::new(),
            rendering: RenderReport::default(),
        })
    }

    pub(crate) fn resolve(
        &mut self,
        index: &BTreeMap<PathBuf, (String, BTreeSet<String>)>,
        options: &PrepareOptions,
    ) -> Result<()> {
        let arena = Arena::new();
        let parse_options = comrak_options(self.allow_raw_html);
        let root = parse_document(&arena, &self.body_md, &parse_options);
        let mut references = Vec::new();
        for node in root.descendants() {
            match &node.data().value {
                NodeValue::Link(link) => references.push((link.url.clone(), false)),
                NodeValue::Image(link) => references.push((link.url.clone(), true)),
                _ => {},
            }
        }
        for (url, image) in references {
            let replacement = self.resolve_reference(&url, image, index, options)?;
            self.rewrites.insert(url, replacement);
        }
        for value in [
            self.front.feature_image.clone(),
            self.front.og_image.clone(),
            self.front.twitter_image.clone(),
        ]
        .into_iter()
        .flatten()
        {
            let replacement = self.resolve_reference(&value, true, index, options)?;
            self.rewrites.insert(value, replacement);
        }
        self.html = self.render(&BTreeMap::new())?;
        self.rendering = rich::inventory(&self.html)?;
        let input = self.input(&self.html, &BTreeMap::new());
        let serialized = serde_json::to_string(&input)?;
        let payload = crate::domain::PostsRequest { posts: vec![input] };
        let size = serde_json::to_vec(&payload)?.len() as u64;
        if size > MAX_PAYLOAD_BYTES {
            return Err(CoreError::TooLarge {
                size,
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        let identities = self
            .assets
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        self.source_hash = content_hash(&[&serialized, &identities]).to_string();
        Ok(())
    }

    fn input(&self, html: &str, urls: &BTreeMap<String, String>) -> crate::domain::PostInput {
        let field = |value: Option<&str>| {
            value.map(|s| replace_asset(self.rewrites.get(s).map_or(s, String::as_str), urls))
        };
        crate::publish::build_input(
            &self.front,
            crate::images::ResolvedContent {
                html: format!("<!--kg-card-begin: html-->\n{html}\n<!--kg-card-end: html-->"),
                feature_image: field(self.front.feature_image.as_deref()),
                og_image: field(self.front.og_image.as_deref()),
                twitter_image: field(self.front.twitter_image.as_deref()),
            },
            self.front.status,
        )
    }

    fn resolve_reference(
        &mut self,
        url: &str,
        image: bool,
        index: &BTreeMap<PathBuf, (String, BTreeSet<String>)>,
        options: &PrepareOptions,
    ) -> Result<String> {
        if let Some(remote) = url.strip_prefix("//") {
            let remote = format!("https://{remote}");
            self.external.insert(remote.clone());
            return Ok(remote);
        }
        if let Ok(remote) = reqwest::Url::parse(url) {
            match remote.scheme() {
                "http" | "https" | "mailto" | "tel" => {
                    self.external.insert(url.to_owned());
                    return Ok(url.to_owned());
                },
                "data" if image => return Ok(url.to_owned()),
                _ => {
                    return Err(CoreError::FrontMatter(format!(
                        "unsupported reference URL: {url}"
                    )));
                },
            }
        }
        if url.starts_with('/') && !image {
            return Ok(url.to_owned());
        }
        let (without_fragment, fragment) = url
            .split_once('#')
            .map_or((url, None), |(p, f)| (p, Some(f)));
        let (path_part, _) = without_fragment
            .split_once('?')
            .unwrap_or((without_fragment, ""));
        let decoded = decode(path_part)?;
        if decoded.is_empty() && !image {
            validate_fragment(fragment, &self.headings, url)?;
            return Ok(url.to_owned());
        }
        let base = self.path.parent().unwrap_or_else(|| Path::new("."));
        let target = self.files.canonicalize(&base.join(&decoded)).map_err(|e| {
            CoreError::FrontMatter(format!("cannot read local reference '{url}': {e}"))
        })?;
        if target
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            && !image
        {
            let (slug, headings) = index.get(&target).ok_or_else(|| {
                CoreError::FrontMatter(format!(
                    "local article '{url}' is not in this publication collection"
                ))
            })?;
            validate_fragment(fragment, headings, url)?;
            let route = options.route_template.replace(ROUTE_SLUG, slug);
            return Ok(
                fragment.map_or_else(|| route.clone(), |fragment| format!("{route}#{fragment}"))
            );
        }
        let metadata = self.files.metadata(&target)?;
        if !metadata.regular || metadata.bytes == 0 {
            return Err(CoreError::FrontMatter(format!(
                "local media must be a nonempty regular file: {url}"
            )));
        }
        if metadata.bytes > MAX_PAYLOAD_BYTES {
            return Err(CoreError::TooLarge {
                size: metadata.bytes,
                limit: MAX_PAYLOAD_BYTES,
            });
        }
        let mime = media::content_type(&target).to_owned();
        let kind = if mime.starts_with("image/") {
            AssetKind::Image
        } else if mime.starts_with("audio/") || mime.starts_with("video/") {
            AssetKind::Media
        } else {
            AssetKind::File
        };
        if image && kind != AssetKind::Image {
            return Err(CoreError::FrontMatter(format!(
                "unsupported image format: {url}"
            )));
        }
        let digest = bytes_hash(&self.files.read(&target)?).to_string();
        let key = content_hash(&[&digest, &mime]).to_string();
        self.assets.insert(
            key.clone(),
            Asset {
                path: target,
                key: key.clone(),
                digest,
                mime,
                kind,
                size: metadata.bytes,
                files: self.files.clone(),
            },
        );
        let suffix = &url[path_part.len()..];
        Ok(format!("{ASSET_PREFIX}{key}{suffix}"))
    }
    /// Validated post metadata.
    #[must_use]
    pub const fn front(&self) -> &FrontMatter {
        &self.front
    }

    /// Render publication HTML with uploaded asset URLs.
    fn render(&self, urls: &BTreeMap<String, String>) -> Result<String> {
        render_with_urls(&self.body_md, self.allow_raw_html, |original| {
            replace_asset(
                self.rewrites.get(original).map_or(original, String::as_str),
                urls,
            )
        })
    }

    pub(crate) fn publication(
        &self,
        urls: &BTreeMap<String, String>,
    ) -> Result<crate::domain::PostInput> {
        Ok(self.input(&self.render(urls)?, urls))
    }

    pub(crate) fn report(&self) -> PostReport {
        PostReport {
            file: self.path.clone(),
            title: self.front.title.clone(),
            slug: self.front.slug.clone(),
            status: self.front.status,
            html_bytes: self.html.len(),
            local_assets: self.assets.len(),
            external_references: self.external.clone(),
            rendering: self.rendering.clone(),
        }
    }
}

fn validate_raw_html(html: &str) -> Result<()> {
    if html.contains("<!--kg-card-") {
        return Err(CoreError::FrontMatter(
            "Ghost card markers are reserved for the publisher".to_owned(),
        ));
    }
    let document = scraper::Html::parse_fragment(html);
    let selector = scraper::Selector::parse("*")
        .map_err(|e| CoreError::FrontMatter(format!("cannot inspect raw HTML: {e}")))?;
    for node in document.select(&selector) {
        for (name, value) in node.value().attrs() {
            if name == "srcset"
                || (matches!(
                    name,
                    "src" | "href" | "poster" | "data" | "action" | "xlink:href"
                ) && !(value.starts_with("//") || reqwest::Url::parse(value).is_ok()))
            {
                return Err(CoreError::FrontMatter("use native Markdown for local resources and srcset; raw HTML resources require absolute URLs".to_owned()));
            }
        }
    }
    Ok(())
}

fn decode(value: &str) -> Result<String> {
    percent_decode_str(value)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|e| CoreError::FrontMatter(format!("invalid UTF-8 in reference '{value}': {e}")))
}

fn validate_fragment(fragment: Option<&str>, headings: &BTreeSet<String>, url: &str) -> Result<()> {
    if let Some(fragment) = fragment.filter(|s| !s.is_empty()) {
        let fragment = decode(fragment)?;
        if !headings.contains(&fragment) {
            return Err(CoreError::FrontMatter(format!(
                "missing heading fragment in '{url}'"
            )));
        }
    }
    Ok(())
}

/// A collection whose metadata, local references, media and sizes passed validation.
#[derive(Debug)]
pub struct PreparedCollection {
    pub(crate) posts: Vec<PreparedPost>,
    options: PrepareOptions,
    files: Files,
}

impl PreparedCollection {
    /// Validate all inputs without reading the environment or using the network.
    ///
    /// # Errors
    /// Returns all file diagnostics together when any input fails.
    pub fn load(paths: &[PathBuf], options: PrepareOptions, files: Files) -> Result<Self> {
        let mut posts = Vec::new();
        let mut errors = Vec::new();
        let mut seen = BTreeSet::new();
        for path in paths {
            match files
                .canonicalize(path)
                .map_err(CoreError::from)
                .and_then(|canonical| {
                    if !seen.insert(canonical.clone()) {
                        return Ok(None);
                    }
                    let text = files.read_text(&canonical)?;
                    let parsed = markdown::parse(&text)?;
                    PreparedPost::from_parts(
                        canonical,
                        parsed.front,
                        parsed.body_md,
                        &options,
                        files.clone(),
                    )
                    .map(Some)
                }) {
                Ok(Some(post)) => posts.push(post),
                Ok(None) => {},
                Err(error) => errors.push(Diagnostic {
                    file: path.clone(),
                    message: error.to_string(),
                }),
            }
        }
        if paths.is_empty() {
            errors.push(Diagnostic {
                file: PathBuf::new(),
                message: "no Markdown files to prepare".to_owned(),
            });
        }
        let mut slugs = BTreeMap::new();
        for post in &posts {
            if let Some(previous) = slugs.insert(post.front.slug.clone(), post.path.clone()) {
                errors.push(Diagnostic {
                    file: post.path.clone(),
                    message: format!(
                        "duplicate slug '{}' also used by {}",
                        post.front.slug,
                        previous.display()
                    ),
                });
            }
        }
        let index: BTreeMap<_, _> = posts
            .iter()
            .map(|p| (p.path.clone(), (p.front.slug.clone(), p.headings.clone())))
            .collect();
        for post in &mut posts {
            if let Err(error) = post.resolve(&index, &options) {
                errors.push(Diagnostic {
                    file: post.path.clone(),
                    message: error.to_string(),
                });
            }
        }
        if !errors.is_empty() {
            let details = errors
                .iter()
                .map(|e| format!("{}: {}", e.file.display(), e.message))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(PreparationError {
                report: CheckReport {
                    posts: posts.iter().map(PreparedPost::report).collect(),
                    errors,
                },
                details,
            }
            .into());
        }
        Ok(Self {
            posts,
            options,
            files,
        })
    }

    /// Validated post and dependency inventory.
    #[must_use]
    pub fn report(&self) -> CheckReport {
        CheckReport {
            posts: self.posts.iter().map(PreparedPost::report).collect(),
            errors: Vec::new(),
        }
    }

    /// Validated posts in deterministic input order.
    #[must_use]
    pub fn posts(&self) -> &[PreparedPost] {
        &self.posts
    }
}

fn comrak_options(allow_raw_html: bool) -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.header_id_prefix = Some(String::new());
    options.render.r#unsafe = allow_raw_html;
    options
}

fn render_with_urls(
    body: &str,
    raw: bool,
    mut rewrite: impl FnMut(&str) -> String,
) -> Result<String> {
    let arena = Arena::new();
    let mut options = comrak_options(raw);
    let root = parse_document(&arena, body, &options);
    let mut media_types = BTreeMap::new();
    for node in root.descendants() {
        match &mut node.data_mut().value {
            NodeValue::Link(link) => {
                let kind = rich::classify(&link.url);
                link.url = rewrite(&link.url);
                if let Some(kind) = kind {
                    media_types.insert(link.url.clone(), kind);
                }
            },
            NodeValue::Image(link) => link.url = rewrite(&link.url),
            _ => {},
        }
    }
    rich::render(root, &options, &media_types)?;
    options.render.r#unsafe = true;
    let mut html = String::new();
    format_html(root, &options, &mut html)
        .map_err(|e| CoreError::FrontMatter(format!("cannot render Markdown: {e}")))?;
    Ok(html)
}

fn replace_asset(value: &str, urls: &BTreeMap<String, String>) -> String {
    let Some(rest) = value.strip_prefix(ASSET_PREFIX) else {
        return value.to_owned();
    };
    let end = rest.find(['?', '#']).unwrap_or(rest.len());
    urls.get(&rest[..end])
        .map_or_else(|| value.to_owned(), |url| format!("{url}{}", &rest[end..]))
}
