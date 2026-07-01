//! The MCP server type and its protocol handler.

use std::path::PathBuf;
use std::sync::Arc;

use ghost_cms_core::Ghost;
use rmcp::ServerHandler;
use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::tool_handler;

/// The MCP server: a thin adapter over [`Ghost`] and `ghost_cms_core::publish`.
///
/// `new` and the tools live in `tools.rs` (the `#[tool_router]` impl block).
#[derive(Clone)]
pub(crate) struct GhostServer {
    /// The shared Ghost client.
    pub(crate) client: Arc<Ghost>,
    /// The blog directory used to resolve relative paths and the state cache.
    pub(crate) blog_dir: PathBuf,
    /// The generated tool router.
    pub(crate) tool_router: ToolRouter<Self>,
}

#[tool_handler]
impl ServerHandler for GhostServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: env!("CARGO_PKG_NAME").to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                ..Implementation::default()
            },
            instructions: Some(
                "Publish and manage posts on a Ghost CMS blog via the Admin API.".to_owned(),
            ),
            ..ServerInfo::default()
        }
    }
}
