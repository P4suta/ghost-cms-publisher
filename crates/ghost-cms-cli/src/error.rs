//! Render shared/core errors as friendly miette diagnostics.
//!
//! The classification lives in `ghost-cms-shared`; this module only maps a
//! [`Diagnosis`](ghost_cms_shared::error::Diagnosis) into a miette report
//! (summary + remediation hint), plus the
//! CLI-only source-span highlighting for front-matter syntax errors.

use std::path::Path;

use ghost_cms_core::CoreError;
use ghost_cms_shared::config::ConfigError;
use ghost_cms_shared::error::{Error, diagnose};
use miette::{NamedSource, SourceSpan};

/// Convert a shared [`Error`] into a friendly miette report.
#[allow(
    clippy::needless_pass_by_value,
    reason = "callers hand off an owned error via map_err; by-value is the ergonomic signature"
)]
pub(crate) fn to_report(error: Error) -> miette::Report {
    let diag = diagnose(&error);
    diag.remediation.hint().map_or_else(
        || miette::miette!("{}", diag.summary),
        |hint| miette::miette!(help = hint, "{}", diag.summary),
    )
}

/// Convert a [`ConfigError`] into a friendly report.
pub(crate) fn config_report(error: ConfigError) -> miette::Report {
    to_report(Error::Config(error))
}

/// Turn `Result<T, E>` into a friendly `miette::Result<T>`.
pub(crate) trait Friendly<T> {
    /// Map the error into a friendly [`miette::Report`].
    fn friendly(self) -> miette::Result<T>;
}

impl<T> Friendly<T> for Result<T, Error> {
    fn friendly(self) -> miette::Result<T> {
        self.map_err(to_report)
    }
}

impl<T> Friendly<T> for Result<T, CoreError> {
    fn friendly(self) -> miette::Result<T> {
        self.map_err(|e| to_report(Error::Core(e)))
    }
}

/// A rich frontmatter diagnostic that underlines the offending byte in the file.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid frontmatter")]
struct FrontmatterDiag {
    #[source_code]
    src: NamedSource<String>,
    #[label("{label}")]
    span: SourceSpan,
    label: String,
    #[help]
    help: String,
}

/// Build a source-highlighted report for a frontmatter syntax error in `file`.
pub(crate) fn frontmatter_report(file: &Path, message: &str, offset: usize) -> miette::Report {
    let content = std::fs::read_to_string(file).unwrap_or_default();
    let off = offset.min(content.len().saturating_sub(1));
    let span: SourceSpan = (off, 1usize).into();
    miette::Report::new(FrontmatterDiag {
        src: NamedSource::new(file.display().to_string(), content),
        span,
        label: message.to_owned(),
        help: "Fix the YAML in the `---` block and try again.".to_owned(),
    })
}
