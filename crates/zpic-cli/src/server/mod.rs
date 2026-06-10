//! HTTP server: PicGo-compatible `/upload` plus `/health` and `/config`.
//!
//! The server is intentionally thin — it reuses the existing CLI
//! upload pipeline (`crate::pipeline::run_upload`) and only adds the
//! HTTP-shaped plumbing around it. The CLI subcommand `zpic server
//! start` lives next to the other subcommands in
//! `crate::commands::server`; this module exposes the lower-level
//! `start` helper that both the subcommand and the integration tests
//! drive.

pub mod error;
pub mod handlers;
pub mod models;
pub mod routes;
pub mod state;

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info, warn};

use crate::util::{load_config, load_uploader_registry, resolve_uploader};
use state::AppState;

/// Default host, matching PicGo's `127.0.0.1` choice. Bounded to
/// loopback so a stray `zpic server start` on a laptop doesn't open
/// the upload pipeline to the whole network.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// Default port, matching PicGo's `:36677`. The Obsidian plugin and
/// other PicGo clients default to this port too, which makes the
/// server interchangeable.
pub const DEFAULT_PORT: u16 = 36677;

/// Parameters for `start`.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Bind address. Defaults to [`DEFAULT_HOST`]:[DEFAULT_PORT].
    pub bind: SocketAddr,
}

impl ServerOptions {
    /// Construct from individual host/port values, applying the
    /// proposal defaults if either is `None`.
    pub fn from_parts(
        host: Option<String>,
        port: Option<u16>,
    ) -> std::result::Result<Self, String> {
        let host = host.unwrap_or_else(|| DEFAULT_HOST.to_string());
        let port = port.unwrap_or(DEFAULT_PORT);
        let bind: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e| format!("invalid bind address `{host}:{port}`: {e}"))?;
        Ok(Self { bind })
    }
}

/// Run the server until `shutdown` resolves. Returns `Ok(())` after a
/// graceful shutdown, or `Err(msg)` when the bind / startup phase
/// fails. Logging is already initialised by the CLI; this function
/// assumes `tracing` is configured.
pub async fn start(
    options: ServerOptions,
    config_path: Option<std::path::PathBuf>,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::result::Result<(), String> {
    // Resolve config + uploader exactly the way the other commands do
    // so the server uses the same active uploader as `zpic upload`.
    let config = load_config(config_path.as_deref())
        .map_err(|e| format!("could not load zpic config: {e}"))?;
    let registry =
        load_uploader_registry().map_err(|e| format!("could not load uploader registry: {e}"))?;
    let resolved = resolve_uploader(&config, &registry.registry, None)
        .map_err(|e| format!("could not resolve active uploader: {e}"))?;

    let state = AppState::new(config, registry.registry, resolved)
        .map_err(|e| format!("could not instantiate uploader: {e}"))?;

    let router = routes::router(state.clone());

    let listener = TcpListener::bind(options.bind)
        .await
        .map_err(|e| format!("could not bind to {}: {e}", options.bind))?;
    let bound = listener
        .local_addr()
        .map_err(|e| format!("could not read bound address: {e}"))?;

    info!(
        host = %bound.ip(),
        port = bound.port(),
        uploader = %state.config.active_uploader_type().unwrap_or("<none>"),
        "zpic server listening"
    );
    println!(
        "✓ zpic server listening on http://{bound}\n  uploader: {} ({})\n  Press Ctrl+C to stop.",
        state.config.active_uploader_type().unwrap_or("<none>"),
        state.active_config_name
    );

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| {
            error!(error = %e, "server error");
            format!("server stopped with error: {e}")
        })?;

    info!("server shut down cleanly");
    println!("\nzpic server stopped.");
    Ok(())
}

/// Wait for SIGINT or SIGTERM. Imported as a single helper so the
/// tests can share the same shutdown semantics.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            warn!(error = %e, "could not install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                warn!(error = %e, "could not install SIGTERM handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
