//! Compatibility checks for the MCP stdio boundary.

use std::collections::BTreeSet;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const IPC_TIMEOUT: Duration = Duration::from_secs(30);
const PROTOCOL_VERSION: &str = "2025-06-18";
const FIXTURE_TOKEN: &str =
    "fixture:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Deserialize)]
struct RpcResponse {
    jsonrpc: String,
    id: u64,
    result: Value,
}

async fn send(input: &mut ChildStdin, message: Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(&message)?;
    bytes.push(b'\n');
    input.write_all(&bytes).await?;
    input.flush().await?;
    Ok(())
}

async fn receive(output: &mut Lines<BufReader<ChildStdout>>, id: u64) -> Result<RpcResponse> {
    let line = timeout(IPC_TIMEOUT, output.next_line())
        .await
        .context("MCP response timed out")??
        .context("MCP stdout closed before its response")?;
    let response: RpcResponse = serde_json::from_str(&line)?;
    assert_eq!(response.jsonrpc, "2.0");
    assert_eq!(response.id, id);
    Ok(response)
}

#[tokio::test]
async fn stdio_negotiates_the_protocol_and_exposes_the_tool_schemas() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghost-cms-mcp"))
        .current_dir(directory.path())
        .env("GHOST_ADMIN_API_URL", "http://127.0.0.1:9")
        .env("GHOST_STAFF_TOKEN", FIXTURE_TOKEN)
        .env("GHOST_LOG", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().context("MCP stdin is unavailable")?;
    let stdout = child.stdout.take().context("MCP stdout is unavailable")?;
    let mut output = BufReader::new(stdout).lines();

    send(
        &mut input,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "compatibility-test", "version": "1"}
            }
        }),
    )
    .await?;
    let initialization = receive(&mut output, 1).await?;
    assert_eq!(initialization.result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(initialization.result["serverInfo"]["name"], "ghost-cms-mcp");
    assert!(initialization.result["capabilities"]["tools"].is_object());

    send(
        &mut input,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await?;
    send(
        &mut input,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await?;
    let listing = receive(&mut output, 2).await?;
    let tools = listing.result["tools"]
        .as_array()
        .context("MCP tools are missing")?;
    let expected = BTreeSet::from([
        "ghost_get_post",
        "ghost_list_posts",
        "ghost_list_tags",
        "ghost_publish_inline",
        "ghost_publish_markdown",
        "ghost_set_tag",
        "ghost_upload",
        "ghost_upload_image",
        "ghost_whoami",
    ]);
    let names: BTreeSet<_> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names, expected);
    assert_eq!(tools.len(), expected.len());
    for tool in tools {
        assert!(tool["inputSchema"].is_object());
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let markdown = tools
        .iter()
        .find(|tool| tool["name"] == "ghost_publish_markdown")
        .context("Markdown publishing tool is missing")?;
    assert_eq!(
        markdown["inputSchema"]["properties"]["file"]["type"],
        "string"
    );
    let required = markdown["inputSchema"]["required"]
        .as_array()
        .context("Markdown publishing fields are missing")?;
    assert!(required.iter().any(|name| name == "file"));
    child.kill().await?;
    Ok(())
}
