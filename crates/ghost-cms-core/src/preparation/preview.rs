use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{PreparedCollection, PreparedPost, ROUTE_SLUG, render_with_urls, replace_asset};
use crate::filesystem::Files;
use crate::{CoreError, Result};

const STYLE: &str = include_str!("preview.css");

struct OutputDirectory {
    path: PathBuf,
    complete: bool,
    files: Files,
}

impl Drop for OutputDirectory {
    fn drop(&mut self) {
        if !self.complete
            && let Err(error) = self.files.remove_dir_all(&self.path)
        {
            tracing::warn!(
                "cannot clean incomplete preview {}: {error}",
                self.path.display()
            );
        }
    }
}

impl PreparedCollection {
    /// Write a self-contained local preview to a new directory.
    ///
    /// # Errors
    /// Refuses an existing output and removes owned partial output after a failure.
    pub fn write_preview(&self, output: &Path) -> Result<PathBuf> {
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            self.files.create_dir_all(parent)?;
        }
        self.files.create_dir(output)?;
        let mut directory = OutputDirectory {
            path: output.to_owned(),
            complete: false,
            files: self.files.clone(),
        };
        self.files.create_dir(&output.join("assets"))?;
        self.files
            .write(&output.join("style.css"), STYLE.as_bytes())?;
        let routes: BTreeMap<_, _> = self
            .posts
            .iter()
            .map(|p| {
                (
                    self.options
                        .route_template
                        .replace(ROUTE_SLUG, &p.front.slug),
                    format!("../{}/index.html", p.front.slug),
                )
            })
            .collect();
        let mut assets = BTreeMap::new();
        for post in &self.posts {
            for asset in post.assets.values() {
                let extension = asset
                    .path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("bin")
                    .to_ascii_lowercase();
                let filename = format!("{}.{}", asset.key, extension);
                if !assets.contains_key(&asset.key) {
                    self.files.write(
                        &output.join("assets").join(&filename),
                        &asset.read_verified()?,
                    )?;
                    assets.insert(asset.key.clone(), format!("../assets/{filename}"));
                }
            }
        }
        let mut index = String::from(
            "<div class=\"eyebrow\">PUBLICATION COLLECTION</div><h1>Ready to review.</h1><p class=\"lead\">Read the prepared articles, follow their links, and check their media before connecting to Ghost.</p><div class=\"cards\">",
        );
        for (number, post) in self.posts.iter().enumerate() {
            let folder = output.join(&post.front.slug);
            self.files.create_dir(&folder)?;
            let html = preview_post(post, &assets, &routes)?;
            self.files.write(
                &folder.join("index.html"),
                page(
                    &post.front.title,
                    post.front.language.as_deref().unwrap_or("en"),
                    "../style.css",
                    &html,
                )
                .as_bytes(),
            )?;
            write!(index, "<a class=\"card\" href=\"{}/index.html\"><span class=\"number\">{:03}</span><div><span class=\"status\">{}</span><h2>{}</h2><p>{} · {} local assets</p></div><span aria-hidden=\"true\">↗</span></a>", escape(&post.front.slug), number + 1, post.front.status, escape(&post.front.title), escape(&post.front.slug), post.assets.len())
                .map_err(|e| CoreError::FrontMatter(e.to_string()))?;
        }
        index.push_str("</div><footer>Local preview · Remote links and media depend on their original hosts.</footer>");
        self.files.write(
            &output.join("report.json"),
            &serde_json::to_vec_pretty(&self.report())?,
        )?;
        self.files.write(
            &output.join("index.html"),
            page("Publication review", "en", "style.css", &index).as_bytes(),
        )?;
        let entry = self.files.canonicalize(&output.join("index.html"))?;
        directory.complete = true;
        Ok(entry)
    }
}

fn preview_post(
    post: &PreparedPost,
    assets: &BTreeMap<String, String>,
    routes: &BTreeMap<String, String>,
) -> Result<String> {
    let html = render_with_urls(&post.body_md, post.allow_raw_html, |original| {
        let target = post.rewrites.get(original).map_or(original, String::as_str);
        let (route, fragment) = target.split_once('#').map_or((target, ""), |(p, f)| (p, f));
        routes.get(route).map_or_else(
            || replace_asset(target, assets),
            |replacement| {
                if fragment.is_empty() {
                    replacement.clone()
                } else {
                    format!("{replacement}#{fragment}")
                }
            },
        )
    })?;
    let mut tags = String::new();
    for tag in &post.front.tags {
        tags.push_str("<span>");
        tags.push_str(&escape(tag));
        tags.push_str("</span>");
    }
    let feature = post
        .front
        .feature_image
        .as_deref()
        .map(|value| {
            let value = post.rewrites.get(value).map_or(value, String::as_str);
            format!(
                "<figure><img class=\"feature\" src=\"{}\" alt=\"{}\">{}</figure>",
                escape(&replace_asset(value, assets)),
                escape(post.front.feature_image_alt.as_deref().unwrap_or_default()),
                post.front.feature_image_caption.as_deref().map_or_else(
                    String::new,
                    |caption| format!("<figcaption>{}</figcaption>", escape(caption))
                )
            )
        })
        .unwrap_or_default();
    Ok(format!(
        "<nav><a href=\"../index.html\">← All articles</a><span>{}</span></nav><article><header><div class=\"eyebrow\">{} · LOCAL PREVIEW</div><h1>{}</h1><div class=\"tags\">{tags}</div></header>{feature}<div class=\"prose\">{html}</div></article><footer>Local preview · Publication status: {}</footer>",
        escape(&post.front.slug),
        post.front.status,
        escape(&post.front.title),
        post.front.status
    ))
}

fn page(title: &str, language: &str, stylesheet: &str, content: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"robots\" content=\"noindex,nofollow\"><title>{}</title><link rel=\"stylesheet\" href=\"{stylesheet}\"></head><body><main>{content}</main></body></html>\n",
        escape(language),
        escape(title)
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
