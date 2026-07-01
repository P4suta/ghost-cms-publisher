//! Upload local images referenced by a post and rewrite their `src` URLs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::client::Ghost;
use crate::error::{CoreError, Result};
use crate::media::{image_mime, is_local};
use crate::transport::HttpTransport;

/// A post's content after local images have been uploaded and rewritten.
pub(crate) struct ResolvedContent {
    /// Body HTML with local image `src`s rewritten to CDN URLs.
    pub(crate) html: String,
    /// Feature image URL after any upload.
    pub(crate) feature_image: Option<String>,
    /// Open Graph image URL after any upload.
    pub(crate) og_image: Option<String>,
    /// Twitter card image URL after any upload.
    pub(crate) twitter_image: Option<String>,
}

/// Matches an `src="…"` / `src='…'` attribute, capturing the quoted value.
const SRC_PATTERN: &str = r#"src\s*=\s*(?:"([^"]*)"|'([^']*)')"#;

/// The compiled, process-wide `src` attribute regex.
fn src_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        #[allow(
            clippy::expect_used,
            reason = "SRC_PATTERN is a compile-time constant known to be a valid regex"
        )]
        Regex::new(SRC_PATTERN).expect("static src regex must compile")
    })
}

/// Byte ranges of every `src` attribute value inside `html`, in order.
fn src_value_spans(html: &str) -> Vec<(usize, usize)> {
    src_regex()
        .captures_iter(html)
        .filter_map(|caps| caps.get(1).or_else(|| caps.get(2)))
        .map(|m| (m.start(), m.end()))
        .collect()
}

/// Resolve a `src` against the post's directory.
fn resolve_path(base_dir: &Path, src: &str) -> PathBuf {
    let trimmed = src.strip_prefix("./").unwrap_or(src);
    let candidate = Path::new(trimmed);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_dir.join(candidate)
    }
}

/// Uploads local images for one post, memoizing by `src` so the same image is
/// only uploaded once across the body, feature, OG, and Twitter fields.
pub(crate) struct ImageResolver<'a, T: HttpTransport> {
    client: &'a Ghost<T>,
    base_dir: &'a Path,
    cache: HashMap<String, String>,
}

impl<'a, T: HttpTransport> ImageResolver<'a, T> {
    /// Create a resolver bound to a client and the post's directory.
    #[must_use]
    pub(crate) fn new(client: &'a Ghost<T>, base_dir: &'a Path) -> Self {
        Self {
            client,
            base_dir,
            cache: HashMap::new(),
        }
    }

    /// Upload a single local image `src` (memoized) and return its CDN URL.
    async fn upload(&mut self, src: &str) -> Result<String> {
        if let Some(url) = self.cache.get(src) {
            return Ok(url.clone());
        }
        let path = resolve_path(self.base_dir, src);
        let bytes = std::fs::read(&path).map_err(|e| {
            CoreError::FrontMatter(format!("cannot read image {}: {e}", path.display()))
        })?;
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("image")
            .to_owned();
        let url = self
            .client
            .media()
            .upload_image(bytes, &filename, image_mime(&path))
            .await?;
        self.cache.insert(src.to_owned(), url.clone());
        Ok(url)
    }

    /// Resolve an optional image field: upload it if it is a local path,
    /// otherwise pass it through unchanged.
    ///
    /// # Errors
    /// Propagates read and upload failures.
    pub(crate) async fn field(&mut self, value: Option<&str>) -> Result<Option<String>> {
        match value {
            Some(src) if is_local(src) => Ok(Some(self.upload(src).await?)),
            Some(src) => Ok(Some(src.to_owned())),
            None => Ok(None),
        }
    }

    /// Rewrite every local `<img src>` in `html` to its uploaded CDN URL.
    ///
    /// # Errors
    /// Propagates read and upload failures.
    pub(crate) async fn html(&mut self, html: &str) -> Result<String> {
        let spans = src_value_spans(html);
        if spans.is_empty() {
            return Ok(html.to_owned());
        }
        let mut out = String::with_capacity(html.len());
        let mut last = 0usize;
        for (start, end) in spans {
            out.push_str(&html[last..start]);
            let src = &html[start..end];
            if is_local(src) {
                out.push_str(&self.upload(src).await?);
            } else {
                out.push_str(src);
            }
            last = end;
        }
        out.push_str(&html[last..]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_local, src_value_spans};

    #[test]
    fn detects_local_vs_remote() {
        assert!(is_local("assets/x.png"));
        assert!(is_local("./a.png"));
        assert!(!is_local("https://cdn/x.png"));
        assert!(!is_local("data:image/png;base64,AAAA"));
    }

    #[test]
    fn finds_quoted_src() {
        let s = r#"<img alt="a" src="assets/x.png">"#;
        let spans = src_value_spans(s);
        assert_eq!(spans.len(), 1);
        let (start, end) = spans[0];
        assert_eq!(&s[start..end], "assets/x.png");
    }

    #[test]
    fn finds_single_quoted_and_multiple() {
        let s = r#"<img src='a.png'><img src="https://cdn/b.png">"#;
        let spans = src_value_spans(s);
        assert_eq!(spans.len(), 2);
        assert_eq!(&s[spans[0].0..spans[0].1], "a.png");
        assert_eq!(&s[spans[1].0..spans[1].1], "https://cdn/b.png");
    }
}
