//! The MCP server type and its protocol handler.

use std::path::PathBuf;
use std::sync::Arc;

use ghost_cms_core::Ghost;
use rmcp::ServerHandler;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;

/// The MCP server: a thin adapter over [`Ghost`] and `ghost_cms_core::publish`.
///
/// `new` and the tools live in `tools.rs` (the `#[tool_router]` impl block).
/// The tool router is regenerated on demand by `#[tool_handler]` via
/// `Self::tool_router()`, so it is not stored as a field.
#[derive(Clone)]
pub(crate) struct GhostServer {
    /// The shared Ghost client.
    pub(crate) client: Arc<Ghost>,
    /// The blog directory used to resolve relative paths and the state cache.
    pub(crate) blog_dir: PathBuf,
}

#[tool_handler]
impl ServerHandler for GhostServer {
    fn get_info(&self) -> ServerInfo {
        // Set server_info explicitly: `ServerInfo::new` defaults it to
        // `Implementation::from_build_env()`, which resolves to rmcp's own crate
        // name/version, not ours.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Publish and manage posts on a Ghost CMS blog via the Admin API.")
    }
}
