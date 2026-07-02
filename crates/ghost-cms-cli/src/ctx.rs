//! The per-invocation context passed to every command.

use std::io::IsTerminal;
use std::time::Duration;

use ghost_cms_core::Ghost;
use ghost_cms_shared::config::Resolved;
use indicatif::ProgressBar;
use owo_colors::Stream;

use crate::error::{config_report, to_report};
use crate::output::Render;
use crate::ui;

/// Shared per-invocation state passed to every command.
pub(crate) struct Ctx {
    /// Emit machine-readable JSON instead of prose.
    pub(crate) json: bool,
    /// Suppress decorative output (errors still print).
    pub(crate) quiet: bool,
    /// Resolved configuration with provenance.
    pub(crate) settings: Resolved,
}

impl Ctx {
    /// Build a Ghost client from the resolved settings.
    pub(crate) fn client(&self) -> miette::Result<Ghost> {
        let cfg = self.settings.to_config().map_err(config_report)?;
        Ghost::new(&cfg).map_err(|e| to_report(e.into()))
    }

    /// Emit a view as JSON or human-facing prose per the `--json` flag.
    pub(crate) fn emit<R: Render>(&self, view: &R) {
        if self.json {
            crate::output::print_json(&view.to_json());
        } else {
            view.print_human(self);
        }
    }

    /// Print a green success line (suppressed when `--quiet`).
    pub(crate) fn success(&self, msg: &str) {
        if !self.quiet {
            println!("{} {msg}", ui::success(Stream::Stdout, ui::SUCCESS));
        }
    }

    /// Print a dimmed note to stderr (suppressed when `--quiet`).
    pub(crate) fn note(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{}", ui::dim(Stream::Stderr, msg));
        }
    }

    /// Print a yellow warning line to stderr (suppressed when `--quiet`).
    pub(crate) fn warn(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{} {msg}", ui::warn(Stream::Stderr, ui::WARN));
        }
    }

    /// Print an informational line to stderr (suppressed when `--quiet`).
    pub(crate) fn info(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{} {msg}", ui::accent(Stream::Stderr, ui::INFO));
        }
    }

    /// Create a spinner for a network wait. Hidden under `--quiet`/`--json` or a
    /// non-interactive stderr.
    pub(crate) fn spinner(&self, msg: &str) -> ProgressBar {
        if self.quiet || self.json || !std::io::stderr().is_terminal() {
            return ProgressBar::hidden();
        }
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(90));
        pb.set_message(msg.to_owned());
        pb
    }
}

/// Print a red failure line to stderr. Never suppressed by `--quiet`.
pub(crate) fn fail(msg: &str) {
    eprintln!("{} {msg}", ui::error(Stream::Stderr, ui::ERROR));
}
