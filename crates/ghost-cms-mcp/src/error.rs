//! Map shared/core errors and status strings into MCP protocol errors.

use ghost_cms_core::CoreError;
use ghost_cms_core::domain::PostStatus;
use ghost_cms_shared::error::{Error, diagnose};
use rmcp::ErrorData;

/// Convert a shared [`Error`] into an MCP [`ErrorData`].
///
/// Caller-attributable failures (bad input/config) become `invalid_params`;
/// everything else becomes `internal_error`. The summary and classification are
/// the same ones the CLI uses.
fn to_error_data(error: &Error) -> ErrorData {
    let diag = diagnose(error);
    if diag.user_error {
        ErrorData::invalid_params(diag.summary, None)
    } else {
        ErrorData::internal_error(diag.summary, None)
    }
}

/// Turn `Result<T, E>` into an MCP `Result<T, ErrorData>`.
pub(crate) trait IntoMcp<T> {
    /// Map the error into an [`ErrorData`].
    fn mcp(self) -> Result<T, ErrorData>;
}

impl<T> IntoMcp<T> for Result<T, Error> {
    fn mcp(self) -> Result<T, ErrorData> {
        self.map_err(|e| to_error_data(&e))
    }
}

impl<T> IntoMcp<T> for Result<T, CoreError> {
    fn mcp(self) -> Result<T, ErrorData> {
        self.map_err(|e| to_error_data(&Error::Core(e)))
    }
}

/// Parse an optional status string into a [`PostStatus`].
///
/// # Errors
/// Returns `invalid_params` if the string is not a known status.
pub(crate) fn parse_status(s: Option<&str>) -> Result<Option<PostStatus>, ErrorData> {
    s.map_or(Ok(None), |v| {
        v.parse::<PostStatus>()
            .map(Some)
            .map_err(|()| ErrorData::invalid_params(format!("unknown status: {v}"), None))
    })
}
