//! Terminal styling: status glyphs, semantic colors, and table chrome.
//!
//! Every `if_supports_color` decision lives here; callers pass the [`Stream`] the
//! text is bound for so a piped stdout still leaves a terminal stderr colored.

use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets};
use ghost_cms_core::domain::PostStatus;
use owo_colors::{OwoColorize, Stream};

/// Marks a completed action.
pub(crate) const SUCCESS: &str = "✓";
/// Marks a failed action.
pub(crate) const ERROR: &str = "✗";
/// Marks a warning.
pub(crate) const WARN: &str = "!";
/// Marks an informational note.
pub(crate) const INFO: &str = "›";
/// Marks a skipped / unchanged (no-op) action.
pub(crate) const SKIP: &str = "∘";

/// Apply `colorize` to `text` only when `stream` supports color.
fn paint(stream: Stream, text: &str, colorize: impl Fn(&str) -> String) -> String {
    text.if_supports_color(stream, |t| colorize(t)).to_string()
}

/// Color `text` as a success (green).
pub(crate) fn success(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.green().to_string())
}

/// Color `text` as an error (red).
pub(crate) fn error(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.red().to_string())
}

/// Color `text` as a warning (yellow).
pub(crate) fn warn(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.yellow().to_string())
}

/// Dim `text` to a secondary/auxiliary weight.
pub(crate) fn dim(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.dimmed().to_string())
}

/// Accent `text` (cyan) — used for identifiers like slugs and URLs.
pub(crate) fn accent(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.cyan().to_string())
}

/// Emphasize `text` (bold).
pub(crate) fn bold(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.bold().to_string())
}

/// Style a slug for inline display (accented, like a value the user can reuse).
pub(crate) fn slug(stream: Stream, text: &str) -> String {
    accent(stream, text)
}

/// Style a filesystem path for inline display.
pub(crate) fn path(stream: Stream, text: &str) -> String {
    paint(stream, text, |t| t.cyan().to_string())
}

/// Style a numeric count for inline display (bold).
pub(crate) fn count(stream: Stream, text: &str) -> String {
    bold(stream, text)
}

/// Color a post status string for inline (non-table) display.
pub(crate) fn status(stream: Stream, status: PostStatus) -> String {
    let text = status.to_string();
    paint(stream, &text, |t| match status {
        PostStatus::Published => t.green().to_string(),
        PostStatus::Scheduled => t.cyan().to_string(),
        _ => t.yellow().to_string(),
    })
}

/// The comfy-table color for a post status (used inside table cells, which carry
/// their own terminal-aware color handling).
pub(crate) const fn status_table_color(status: PostStatus) -> Color {
    match status {
        PostStatus::Published => Color::Green,
        PostStatus::Scheduled => Color::Cyan,
        _ => Color::Yellow,
    }
}

/// A fresh table with the house preset: light outer borders, a header rule, and
/// width that adapts to the terminal.
pub(crate) fn table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

/// Build a dimmed header row from column labels.
pub(crate) fn header_row<I>(labels: I) -> Vec<Cell>
where
    I: IntoIterator<Item = &'static str>,
{
    labels
        .into_iter()
        .map(|label| Cell::new(label).add_attribute(Attribute::Dim))
        .collect()
}
