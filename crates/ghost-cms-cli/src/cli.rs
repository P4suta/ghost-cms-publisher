//! The clap command-line interface definition.

use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};

use crate::commands;
use crate::output::ColorChoice;

/// Help coloring matched to the app's palette: green headings, cyan literals.
const HELP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

/// Worked examples appended to the top-level `--help`.
const TOP_EXAMPLES: &str = "\
Examples:
  ghost-cms init                     Guided first-time setup (URL + token)
  ghost-cms new \"My First Post\"       Scaffold a draft and open it in $EDITOR
  ghost-cms publish blog/posts/      Publish every Markdown file in a directory
  ghost-cms list                     Show recent posts as a table
  ghost-cms watch                    Auto-publish files as you save them

Run `ghost-cms <command> --help` for command-specific options and examples.";

/// Worked examples appended to `publish --help`.
const PUBLISH_EXAMPLES: &str = "\
Examples:
  ghost-cms publish post.md              Create or update one post (idempotent by slug)
  ghost-cms publish blog/posts/          Publish every *.md in a directory
  ghost-cms publish post.md --dry-run    Preview the plan without writing
  ghost-cms publish post.md --publish    Force the post's status to published";

/// Worked examples appended to `new --help`.
const NEW_EXAMPLES: &str = "\
Examples:
  ghost-cms new \"My First Post\"            Scaffold from a title, then open $EDITOR
  ghost-cms new \"My Post\" --slug my-post   Override the derived slug
  ghost-cms new \"My Post\" --no-edit        Scaffold without opening an editor";

/// Worked examples appended to `tags --help`.
const TAGS_EXAMPLES: &str = "\
Examples:
  ghost-cms tags list                       List tags with post counts
  ghost-cms tags set news --name \"News\"      Create or update a tag (upsert by slug)
  ghost-cms tags get news                   Show one tag's metadata
  ghost-cms tags delete news                Delete a tag";

/// Worked examples appended to `watch --help`.
const WATCH_EXAMPLES: &str = "\
Examples:
  ghost-cms watch                  Watch <blog_dir>/posts and auto-publish on save
  ghost-cms watch drafts/          Watch a specific directory instead";

/// Publish Markdown posts to a Ghost CMS blog via the Admin API.
///
/// Configuration is resolved with the precedence: flag > environment variable >
/// `ghost-cms.toml` > default. Relevant env vars: `GHOST_ADMIN_API_URL`,
/// `GHOST_STAFF_TOKEN`, `GHOST_ACCEPT_VERSION`, `GHOST_BLOG_DIR`. Run
/// `ghost-cms init` for guided first-time setup.
#[derive(Debug, Parser)]
#[command(
    name = "ghost-cms",
    version,
    about,
    long_about,
    styles = HELP_STYLES,
    after_help = TOP_EXAMPLES
)]
pub(crate) struct Cli {
    /// Ghost site origin, e.g. `https://example.ghost.io`.
    #[arg(long, global = true, value_hint = clap::ValueHint::Url)]
    pub(crate) api_url: Option<String>,

    /// Staff Access Token in `{id}:{secret}` form.
    #[arg(long, global = true)]
    pub(crate) token: Option<String>,

    /// Admin API version header (default `v5.0`).
    #[arg(long, global = true)]
    pub(crate) accept_version: Option<String>,

    /// Directory containing `posts/` (default `blog`).
    #[arg(long, global = true, value_hint = clap::ValueHint::DirPath)]
    pub(crate) blog_dir: Option<String>,

    /// When to colorize output.
    #[arg(long, global = true, value_enum, default_value_t = ColorArg::Auto)]
    pub(crate) color: ColorArg,

    /// Emit machine-readable JSON where supported.
    #[arg(long, global = true)]
    pub(crate) json: bool,

    /// Suppress decorative output (errors still print).
    #[arg(long, short, global = true)]
    pub(crate) quiet: bool,

    /// Increase logging verbosity (-v info, -vv debug, -vvv trace).
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub(crate) verbose: u8,

    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Color preference flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ColorArg {
    /// Detect from the terminal and `NO_COLOR`.
    Auto,
    /// Always colorize.
    Always,
    /// Never colorize.
    Never,
}

impl From<ColorArg> for ColorChoice {
    fn from(c: ColorArg) -> Self {
        match c {
            ColorArg::Auto => Self::Auto,
            ColorArg::Always => Self::Always,
            ColorArg::Never => Self::Never,
        }
    }
}

/// The set of subcommands.
#[derive(Debug, Subcommand)]
#[allow(
    clippy::large_enum_variant,
    reason = "clap subcommand variants are sized inline; boxing arg fields breaks clap derive"
)]
pub(crate) enum Command {
    /// Guided first-time setup (URL + token), validated and saved.
    Init(commands::init::InitArgs),
    /// Set or update the Staff Access Token.
    Login(commands::init::LoginArgs),
    /// Diagnose configuration and connectivity.
    Doctor(commands::doctor::DoctorArgs),
    /// Validate the token and print the site it points at.
    Whoami(commands::whoami::WhoamiArgs),
    /// Scaffold a new post file under `<blog_dir>/posts/`.
    #[command(after_help = NEW_EXAMPLES)]
    New(commands::new::NewArgs),
    /// Create or update one or more posts from Markdown files (idempotent by slug).
    #[command(after_help = PUBLISH_EXAMPLES)]
    Publish(commands::publish::PublishArgs),
    /// List recent posts.
    List(commands::list::ListArgs),
    /// Fetch one post by slug.
    Get(commands::get::GetArgs),
    /// Open a post's URL in the browser.
    Open(commands::open::OpenArgs),
    /// Open the local file for a slug in `$EDITOR`.
    Edit(commands::edit::EditArgs),
    /// Delete a post by slug.
    Delete(commands::delete::DeleteArgs),
    /// Upload an image file and print its CDN URL.
    #[command(name = "upload-image")]
    UploadImage(commands::upload::UploadImageArgs),
    /// Upload an image, media (audio/video), or arbitrary file.
    Upload(commands::upload::UploadArgs),
    /// Manage tags (list, get, set metadata, delete).
    #[command(after_help = TAGS_EXAMPLES)]
    Tags(commands::tags::TagsArgs),
    /// Watch the blog directory and auto-publish saved Markdown files.
    #[command(after_help = WATCH_EXAMPLES)]
    Watch(commands::watch::WatchArgs),
    /// Print a shell completion script.
    Completions(commands::meta::CompletionsArgs),
    /// Generate man pages into a directory.
    Man(commands::meta::ManArgs),
}
