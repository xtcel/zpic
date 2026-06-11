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

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
    println!("✓ zpic server listening on http://{bound}");
    for line in lan_banner_lines(bound) {
        println!("{line}");
    }
    println!(
        "  uploader: {} ({})\n  Press Ctrl+C to stop.",
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

/// True for IPv4 addresses that a phone or laptop on the same LAN
/// is unlikely to reach: loopback, link-local, CGNAT, multicast,
/// benchmarking, reserved-by-IANA, documentation, unspecified, and
/// broadcast. RFC 1918 (10/8, 172.16/12, 192.168/16) and public IPs
/// are kept — those are what the user wants to see in the banner.
fn is_uninteresting_ipv4(ip: Ipv4Addr) -> bool {
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_link_local()
        || ip.is_documentation()
    {
        return true;
    }
    // The remaining "uninteresting" predicates are still nightly-only
    // on stable Rust (`is_shared` / `is_benchmarking` / `is_reserved`
    // live behind `#![feature(ip)]`). The project MSRV is 1.75, so we
    // match the ranges by hand.
    let [a, b, _, _] = ip.octets();
    // CGNAT (100.64.0.0/10)
    a == 100 && (64..=127).contains(&b)
    // Benchmarking (198.18.0.0/15)
    || (a == 198 && (18..=19).contains(&b))
    // IANA reserved for future use (240.0.0.0/4)
    || a >= 240
}

/// Enumerate every local IPv4 address that is plausibly reachable
/// from another device on the same network. Uses `getifaddrs` on
/// Unix and `GetAdaptersAddresses` on Windows via the `if-addrs`
/// crate — the same enumeration `ip addr` / `ipconfig` would show.
///
/// We don't try to pick "the best" one: a host with WiFi + ethernet
/// legitimately has two LAN addresses, and the user (or their phone)
/// is the one who knows which network they're on. The banner just
/// shows all of them.
fn lan_ipv4_addrs() -> Vec<Ipv4Addr> {
    let addrs = match if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => {
            // `getifaddrs` is essentially infallible on every
            // platform we support, but treat any error as "no LAN
            // info" rather than panicking the server.
            warn!(error = %e, "could not enumerate network interfaces");
            return Vec::new();
        }
    };
    let mut out: Vec<Ipv4Addr> = addrs
        .into_iter()
        .filter_map(|iface| match iface.ip() {
            IpAddr::V4(v4) if !is_uninteresting_ipv4(v4) => Some(v4),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Extra banner lines to print after the "listening on" line. Only
/// emitted when the bound address is the IPv4 wildcard (`0.0.0.0`),
/// i.e. the user explicitly asked for a public bind — the same case
/// where a phone on the same WiFi might want the URL. Otherwise the
/// banner stays compact.
fn lan_banner_lines(bound: SocketAddr) -> Vec<String> {
    if !bound.ip().is_unspecified() {
        return Vec::new();
    }
    let mut lines = vec![format!("  ➜  Local:   http://127.0.0.1:{}/", bound.port())];
    for ip in lan_ipv4_addrs() {
        lines.push(format!("  ➜  Network: http://{ip}:{}/", bound.port()));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn is_uninteresting_ipv4_catches_special_ranges() {
        // Loopback
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(127, 255, 255, 254)));
        // Unspecified / broadcast
        assert!(is_uninteresting_ipv4(Ipv4Addr::UNSPECIFIED));
        assert!(is_uninteresting_ipv4(Ipv4Addr::BROADCAST));
        // Link-local
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(169, 254, 1, 1)));
        // CGNAT (100.64/10)
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(100, 127, 255, 254)));
        // Multicast
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(224, 0, 0, 1)));
        // Benchmarking (198.18/15)
        assert!(is_uninteresting_ipv4(Ipv4Addr::new(198, 18, 0, 1)));
    }

    #[test]
    fn is_uninteresting_ipv4_keeps_real_lan_ips() {
        // RFC 1918
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(192, 168, 1, 1)));
        // CGNAT just-outside (100.63 / 100.128) must pass
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(100, 63, 255, 254)));
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(100, 128, 0, 1)));
        // Public-ish
        assert!(!is_uninteresting_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
    }

    #[test]
    fn lan_banner_is_empty_for_specific_bind() {
        // Loopback and specific non-loopback binds stay quiet so the
        // default `zpic server start` UX doesn't get noisier.
        let loopback: SocketAddr = "127.0.0.1:36677".parse().unwrap();
        assert!(lan_banner_lines(loopback).is_empty());
        let specific: SocketAddr = "192.0.2.10:36677".parse().unwrap();
        assert!(lan_banner_lines(specific).is_empty());
    }

    #[test]
    fn lan_banner_names_local_and_network_for_wildcard() {
        // We can't assert the exact LAN IPs (they depend on the
        // host), but the shape should be: first line is `Local:
        // http://127.0.0.1:<port>/`, every following line starts
        // with `Network: http://` and ends in `:<port>/`.
        let wildcard: SocketAddr = "0.0.0.0:36677".parse().unwrap();
        let lines = lan_banner_lines(wildcard);
        // A fully sandboxed runner may legitimately have no LAN
        // interface at all.
        if lines.is_empty() {
            return;
        }
        assert!(
            lines[0].contains("127.0.0.1:36677"),
            "local line: {}",
            lines[0]
        );
        for line in &lines[1..] {
            assert!(line.contains(":36677"), "network line: {line}");
            assert!(
                line.starts_with("  \u{279c}  Network: http://"),
                "network line: {line}"
            );
        }
    }
}
