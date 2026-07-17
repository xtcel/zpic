//! `zpic-mcp` — an MCP server that lets AI agents (Claude Code, Codex, and
//! other MCP-aware tools) call zpic's upload/migrate/history/doctor
//! commands directly, without shelling out on their own.
//!
//! Communicates over stdio, so all logging goes to stderr: anything
//! written to stdout other than JSON-RPC frames would corrupt the
//! protocol stream.

mod config;
mod server;

use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use config::McpConfig;
use server::ZpicMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = McpConfig::load()?;
    tracing::info!(workspace_roots = ?config.workspace_roots, zpic_bin = %config.zpic_bin, "starting zpic-mcp");

    let service = ZpicMcpServer::new(config).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
