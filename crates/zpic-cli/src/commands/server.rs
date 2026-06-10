//! `zpic server` — start the HTTP API that the Obsidian plugin and
//! other PicGo-compatible clients talk to.
//!
//! Only the `start` action is implemented in this change; `stop` /
//! `status` / `install` / `uninstall` are tracked in follow-up
//! changes for the launchd / systemd service integration.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::server::{self, ServerOptions};
use zpic_core::error::Result;

#[derive(Debug, Args)]
pub struct ServerArgs {
    /// Path to a config file (overrides all other sources).
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub action: ServerAction,
}

#[derive(Debug, Subcommand)]
pub enum ServerAction {
    /// Start the HTTP server in the foreground.
    Start {
        /// Bind host. Defaults to `127.0.0.1` (loopback only).
        #[arg(long, value_name = "HOST")]
        host: Option<String>,

        /// Bind port. Defaults to `36677` (PicGo-compatible).
        #[arg(long, value_name = "PORT")]
        port: Option<u16>,
    },
}

/// Dispatch for the `zpic server` subcommand.
pub async fn run(args: ServerArgs) -> Result<i32> {
    match args.action {
        ServerAction::Start { host, port } => {
            let options = ServerOptions::from_parts(host, port)
                .map_err(|e| zpic_core::error::ZpicError::InvalidArgument(e))?;
            match server::start(options, args.config, server::shutdown_signal()).await {
                Ok(()) => Ok(0),
                Err(msg) => {
                    eprintln!("error: {msg}");
                    Ok(1)
                }
            }
        }
    }
}
