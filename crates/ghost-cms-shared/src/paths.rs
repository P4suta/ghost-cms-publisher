//! Path conventions shared by the frontends.
//!
//! There is one state-cache convention (`<blog_dir>/.ghost-cms/state.json`) and
//! one relative-path rule (resolve against the blog directory), used by both the
//! CLI and the MCP server.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Directory under the blog root that holds tool state.
pub const STATE_DIR: &str = ".ghost-cms";

/// Path to the publish-state cache for a blog directory.
#[must_use]
pub fn state_path(blog_dir: &Path) -> PathBuf {
    blog_dir.join(STATE_DIR).join("state.json")
}

/// Resolve a possibly-relative path against `base_dir`.
#[must_use]
pub fn resolve(base_dir: &Path, p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

/// Whether `path` looks like a Markdown file.
fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Expand publish inputs: directories become their top-level `*.md` files
/// (sorted); plain files pass through unchanged.
///
/// # Errors
/// Returns [`Error::ReadDir`] if a directory cannot be read.
pub fn expand_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for input in inputs {
        if input.is_dir() {
            let entries = std::fs::read_dir(input).map_err(|source| Error::ReadDir {
                path: input.display().to_string(),
                source,
            })?;
            let mut md: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| is_markdown(p))
                .collect();
            md.sort();
            out.extend(md);
        } else {
            out.push(input.clone());
        }
    }
    Ok(out)
}

/// List the front-matter slugs of local post files under `<blog_dir>/posts`,
/// sorted. Files that cannot be read or parsed are skipped.
///
/// # Errors
/// Returns [`Error::ReadDir`] if the posts directory is unreadable.
pub fn local_post_slugs(blog_dir: &Path) -> Result<Vec<String>> {
    let posts = blog_dir.join("posts");
    let entries = std::fs::read_dir(&posts).map_err(|source| Error::ReadDir {
        path: posts.display().to_string(),
        source,
    })?;
    let mut slugs: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_markdown(p))
        .filter_map(|p| std::fs::read_to_string(&p).ok())
        .filter_map(|text| ghost_cms_core::markdown::parse(&text).ok())
        .map(|parsed| parsed.front.slug)
        .collect();
    slugs.sort();
    Ok(slugs)
}

/// Find the local post file whose front matter slug matches `slug`.
///
/// # Errors
/// Returns [`Error::ReadDir`] if the posts directory is unreadable, or
/// [`Error::NotFound`] if no file matches.
pub fn find_post_file(blog_dir: &Path, slug: &str) -> Result<PathBuf> {
    let posts = blog_dir.join("posts");
    let entries = std::fs::read_dir(&posts).map_err(|source| Error::ReadDir {
        path: posts.display().to_string(),
        source,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_markdown(&path) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path)
            && matches!(ghost_cms_core::markdown::parse(&text), Ok(p) if p.front.slug == slug)
        {
            return Ok(path);
        }
    }
    Err(Error::NotFound {
        resource: "post",
        slug: slug.to_owned(),
        suggestion: None,
    })
}
