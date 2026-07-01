//! Terminal presentation: color, tables, and the [`Render`] view abstraction.
//!
//! The neutral data (JSON shapes, rows, detail fields, one-line summaries) comes
//! from `ghost-cms-shared`; this module only adds color and table chrome. Every
//! command emits a view through [`Ctx::emit`], so the `if json { … } else { … }`
//! fork lives here once instead of in each command.

use std::path::Path;

use comfy_table::{Cell, CellAlignment};
use ghost_cms_core::PublishOutcome;
use ghost_cms_core::domain::{Post, PostStatus, SiteInfo, Tag};
use ghost_cms_shared::render;
use owo_colors::Stream;
use serde_json::Value;

use crate::ctx::Ctx;
use crate::ui;

/// How to colorize output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorChoice {
    /// Auto-detect from the terminal / `NO_COLOR`.
    Auto,
    /// Always colorize.
    Always,
    /// Never colorize.
    Never,
}

/// Apply the color choice process-wide.
pub(crate) fn apply_color(choice: ColorChoice) {
    match choice {
        ColorChoice::Auto => owo_colors::unset_override(),
        ColorChoice::Always => owo_colors::set_override(true),
        ColorChoice::Never => owo_colors::set_override(false),
    }
}

/// Pretty-print a JSON value to stdout (best-effort).
pub(crate) fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
    );
}

/// Print a one-line, colored tally of a multi-file publish run (human-only;
/// suppressed under `--quiet`). Zero-count buckets are omitted.
pub(crate) fn print_publish_summary(ctx: &Ctx, summary: render::PublishSummary) {
    if ctx.quiet {
        return;
    }
    let entries = [
        (summary.created, "created"),
        (summary.updated, "updated"),
        (summary.skipped, "unchanged"),
        (summary.dry_run, "dry-run"),
    ];
    let parts: Vec<String> = entries
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, label)| {
            format!(
                "{} {}",
                ui::count(Stream::Stdout, &n.to_string()),
                ui::dim(Stream::Stdout, label)
            )
        })
        .collect();
    if parts.is_empty() {
        return;
    }
    let sep = ui::dim(Stream::Stdout, " · ");
    println!(
        "{} {}",
        ui::accent(Stream::Stdout, ui::INFO),
        parts.join(&sep)
    );
}

/// A view that can render itself as JSON or as human-facing prose.
pub(crate) trait Render {
    /// The machine-readable JSON projection.
    fn to_json(&self) -> Value;
    /// Render to the terminal (stdout/stderr), using `ctx` for quiet policy.
    fn print_human(&self, ctx: &Ctx);
}

/// Print key/value detail rows, dimming labels and coloring select values.
fn print_detail(fields: &[(&'static str, String)]) {
    for (key, value) in fields {
        let label = ui::dim(Stream::Stdout, &format!("{key:<11}"));
        let rendered = match *key {
            "status" => value
                .parse::<PostStatus>()
                .map_or_else(|()| value.clone(), |s| ui::status(Stream::Stdout, s)),
            "slug" => ui::slug(Stream::Stdout, value),
            "url" if value != "-" => ui::accent(Stream::Stdout, value),
            _ => value.clone(),
        };
        println!("{label} {rendered}");
    }
}

/// A list of posts.
pub(crate) struct PostList<'a>(pub(crate) &'a [Post]);

impl Render for PostList<'_> {
    fn to_json(&self) -> Value {
        Value::Array(self.0.iter().map(render::post_value).collect())
    }

    fn print_human(&self, ctx: &Ctx) {
        if self.0.is_empty() {
            ctx.note("(no posts) — run `ghost-cms new <slug>` to create one.");
            return;
        }
        let mut table = ui::table();
        table.set_header(ui::header_row(["status", "slug", "updated", "title"]));
        for row in render::post_rows(self.0) {
            table.add_row(vec![
                Cell::new(row.status.to_string()).fg(ui::status_table_color(row.status)),
                Cell::new(row.slug),
                Cell::new(row.updated),
                Cell::new(row.title),
            ]);
        }
        println!("{table}");
    }
}

/// A single post's detail.
pub(crate) struct PostView<'a>(pub(crate) &'a Post);

impl Render for PostView<'_> {
    fn to_json(&self) -> Value {
        render::post_value(self.0)
    }

    fn print_human(&self, _ctx: &Ctx) {
        print_detail(&render::post_detail_fields(self.0));
    }
}

/// A list of tags.
pub(crate) struct TagList<'a>(pub(crate) &'a [Tag]);

impl Render for TagList<'_> {
    fn to_json(&self) -> Value {
        Value::Array(self.0.iter().map(render::tag_value).collect())
    }

    fn print_human(&self, ctx: &Ctx) {
        if self.0.is_empty() {
            ctx.note("(no tags) — run `ghost-cms tags set <slug>` to create one.");
            return;
        }
        let mut table = ui::table();
        table.set_header(ui::header_row(["slug", "name", "posts", "visibility"]));
        for row in render::tag_rows(self.0) {
            table.add_row(vec![
                Cell::new(row.slug),
                Cell::new(row.name),
                Cell::new(row.posts).set_alignment(CellAlignment::Right),
                Cell::new(row.visibility),
            ]);
        }
        println!("{table}");
    }
}

/// A single tag's detail.
pub(crate) struct TagView<'a>(pub(crate) &'a Tag);

impl Render for TagView<'_> {
    fn to_json(&self) -> Value {
        render::tag_value(self.0)
    }

    fn print_human(&self, _ctx: &Ctx) {
        print_detail(&render::tag_detail_fields(self.0));
    }
}

/// Site metadata.
pub(crate) struct SiteView<'a>(pub(crate) &'a SiteInfo);

impl Render for SiteView<'_> {
    fn to_json(&self) -> Value {
        render::site_value(self.0)
    }

    fn print_human(&self, ctx: &Ctx) {
        ctx.success(&format!("{} ({})", self.0.title, self.0.url));
        if let Some(v) = &self.0.version {
            ctx.note(&format!("Ghost {v}"));
        }
    }
}

/// A publish outcome, optionally tagged with the source file.
pub(crate) struct OutcomeView<'a> {
    /// The file this outcome is for, if any.
    pub(crate) file: Option<&'a Path>,
    /// The outcome.
    pub(crate) outcome: &'a PublishOutcome,
}

impl Render for OutcomeView<'_> {
    fn to_json(&self) -> Value {
        render::outcome_value(self.file, self.outcome)
    }

    fn print_human(&self, ctx: &Ctx) {
        let line = render::outcome_line(self.outcome);
        match self.outcome {
            PublishOutcome::Created { .. } | PublishOutcome::Updated { .. } => ctx.success(&line),
            PublishOutcome::SkippedUnchanged { .. } | PublishOutcome::DryRun { .. } => {
                ctx.note(&format!("{} {line}", ui::SKIP));
            },
        }
    }
}
