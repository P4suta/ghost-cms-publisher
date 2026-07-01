//! Launching the user's `$EDITOR` — the one genuinely CLI-only file helper.

use std::path::Path;

/// Platform default editor when `$VISUAL`/`$EDITOR` are unset.
const fn default_editor() -> &'static str {
    if cfg!(windows) { "notepad" } else { "vi" }
}

/// Open a file in the user's editor (`$VISUAL`, then `$EDITOR`, then default).
///
/// # Errors
/// Returns a diagnostic if the editor cannot be launched or exits non-zero.
pub(crate) fn open_in_editor(path: &Path) -> miette::Result<()> {
    let configured = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .ok();
    let mut parts: Vec<String> = configured
        .filter(|e| !e.trim().is_empty())
        .map(|e| e.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    let program = if parts.is_empty() {
        default_editor().to_owned()
    } else {
        parts.remove(0)
    };

    let status = std::process::Command::new(&program)
        .args(&parts)
        .arg(path)
        .status()
        .map_err(|e| {
            miette::miette!(
                help = "Set $EDITOR to your editor.",
                "failed to launch `{program}`: {e}"
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(miette::miette!("editor `{program}` exited with {status}"))
    }
}
