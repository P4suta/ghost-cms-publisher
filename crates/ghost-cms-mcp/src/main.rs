//! ghost-cms-mcp: stdio MCP server exposing Ghost publishing tools.
//!
//! IMPORTANT: stdout is the JSON-RPC channel. Never write to stdout (no
//! `println!`); all logging goes to stderr.
#![allow(
    clippy::redundant_pub_crate,
    reason = "binary crate: pub(crate) is honest here and this nursery lint conflicts with unreachable_pub"
)]

mod args;
mod error;
mod server;
mod tools;

use ghost_cms_core::Ghost;
use ghost_cms_shared::config;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

use crate::server::GhostServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("GHOST_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Resolve config like the CLI (env + ghost-cms.toml) for the same precedence.
    let resolved = config::resolve(config::Overrides::default());
    let cfg = resolved.to_config()?;
    let client = Ghost::new(&cfg)?;
    let blog_dir = resolved.blog_dir.clone();

    let server = GhostServer::new(client, blog_dir);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
