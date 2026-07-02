//! Layered configuration resolution (flag > env > file > default) with
//! provenance. The token is only read from a flag or env, never the file.

use std::path::{Path, PathBuf};

use ghost_cms_core::Config;
use ghost_cms_core::constants::DEFAULT_ACCEPT_VERSION;
use serde::{Deserialize, Serialize};

/// Default config file name, searched in the CWD and the user config dir.
pub const CONFIG_FILE: &str = "ghost-cms.toml";

const ENV_API_URL: &str = "GHOST_ADMIN_API_URL";
const ENV_TOKEN: &str = "GHOST_STAFF_TOKEN";
const ENV_ACCEPT_VERSION: &str = "GHOST_ACCEPT_VERSION";
const ENV_BLOG_DIR: &str = "GHOST_BLOG_DIR";

/// Required configuration was missing when building a [`Config`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// No Ghost site URL was configured.
    #[error("no Ghost site URL configured")]
    MissingSiteUrl,
    /// No Staff Access Token was configured.
    #[error("no Staff Access Token configured")]
    MissingToken,
}

/// Where a resolved value came from (shown by `doctor`).
#[derive(Debug, Clone)]
pub enum Source {
    /// A command-line flag.
    Flag,
    /// An environment variable (name carried for display).
    Env(&'static str),
    /// The config file at the given path.
    File(PathBuf),
    /// A built-in default.
    Default,
    /// Not set anywhere.
    Unset,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flag => f.write_str("flag"),
            Self::Env(name) => write!(f, "env {name}"),
            Self::File(path) => write!(f, "file {}", path.display()),
            Self::Default => f.write_str("default"),
            Self::Unset => f.write_str("unset"),
        }
    }
}

/// Non-secret settings persisted in `ghost-cms.toml`.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct FileConfig {
    /// Ghost site origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    /// Admin API version header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_version: Option<String>,
    /// Directory holding `posts/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blog_dir: Option<String>,
}

/// Configuration-relevant overrides (CLI globals; empty for MCP).
#[derive(Debug, Default)]
pub struct Overrides {
    /// `--api-url`.
    pub api_url: Option<String>,
    /// `--token`.
    pub token: Option<String>,
    /// `--accept-version`.
    pub accept_version: Option<String>,
    /// `--blog-dir`.
    pub blog_dir: Option<String>,
}

/// Fully resolved configuration plus provenance for diagnostics.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Resolved site URL, if any.
    pub api_url: Option<String>,
    /// Where the site URL came from.
    pub api_url_source: Source,
    /// Resolved token, if any.
    pub token: Option<String>,
    /// Where the token came from.
    pub token_source: Source,
    /// Resolved Admin API version.
    pub accept_version: String,
    /// Where the version came from.
    pub accept_version_source: Source,
    /// Resolved blog directory.
    pub blog_dir: PathBuf,
    /// The config file that was loaded, if any.
    pub config_path: Option<PathBuf>,
}

impl Resolved {
    /// Build a core [`Config`], requiring the URL and token to be present.
    ///
    /// # Errors
    /// Returns [`ConfigError`] when the URL or token is unset.
    pub fn to_config(&self) -> Result<Config, ConfigError> {
        let api_url = self.api_url.clone().ok_or(ConfigError::MissingSiteUrl)?;
        let token = self.token.clone().ok_or(ConfigError::MissingToken)?;
        Ok(Config::new(api_url, token, self.accept_version.clone()))
    }
}

/// The user-specific config directory (`%APPDATA%/ghost-cms`, `~/.config/ghost-cms`).
#[must_use]
pub fn user_config_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "ghost-cms").map(|d| d.config_dir().to_path_buf())
}

/// Write a non-secret config file (overwrites).
///
/// # Errors
/// Returns an error if serialization or the write fails.
pub fn write_config(path: &Path, cfg: &FileConfig) -> std::io::Result<()> {
    let text = toml::to_string(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, text)
}

/// Locate an existing config file: CWD first, then the user config dir.
fn find_config_file() -> Option<PathBuf> {
    let cwd = PathBuf::from(CONFIG_FILE);
    if cwd.is_file() {
        return Some(cwd);
    }
    let user = user_config_dir()?.join(CONFIG_FILE);
    user.is_file().then_some(user)
}

/// Read and parse a config file, returning an empty config on any failure.
fn load_file(path: &Path) -> FileConfig {
    let Ok(text) = std::fs::read_to_string(path) else {
        return FileConfig::default();
    };
    toml::from_str(&text).unwrap_or_else(|e| {
        tracing::warn!("ignoring malformed {}: {e}", path.display());
        FileConfig::default()
    })
}

/// Resolve a single optional string with flag > env > file precedence.
fn pick(
    flag: Option<String>,
    env: &'static str,
    file: Option<String>,
    file_path: Option<&Path>,
) -> (Option<String>, Source) {
    if let Some(v) = flag {
        return (Some(v), Source::Flag);
    }
    if let Ok(v) = std::env::var(env)
        && !v.is_empty()
    {
        return (Some(v), Source::Env(env));
    }
    match (file, file_path) {
        (Some(v), Some(p)) => (Some(v), Source::File(p.to_path_buf())),
        _ => (None, Source::Unset),
    }
}

/// Resolve settings from overrides, environment, and the config file.
#[must_use]
pub fn resolve(ov: Overrides) -> Resolved {
    let config_path = find_config_file();
    let file = config_path.as_deref().map(load_file).unwrap_or_default();
    let fp = config_path.as_deref();

    let (api_url, api_url_source) = pick(ov.api_url, ENV_API_URL, file.api_url, fp);
    // The token never comes from a file.
    let (token, token_source) = pick(ov.token, ENV_TOKEN, None, None);

    let (accept_version, accept_version_source) = match pick(
        ov.accept_version,
        ENV_ACCEPT_VERSION,
        file.accept_version,
        fp,
    ) {
        (Some(v), src) => (v, src),
        (None, _) => (DEFAULT_ACCEPT_VERSION.to_owned(), Source::Default),
    };

    let (blog_dir, _) = pick(ov.blog_dir, ENV_BLOG_DIR, file.blog_dir, fp);
    let blog_dir = blog_dir.map_or_else(|| PathBuf::from("blog"), PathBuf::from);

    Resolved {
        api_url,
        api_url_source,
        token,
        token_source,
        accept_version,
        accept_version_source,
        blog_dir,
        config_path,
    }
}
