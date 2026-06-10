//! Shared state for the HTTP server.
//!
//! Built once in `start` and wrapped in an `Arc` so the axum router can
//! hand a cheap reference to every request handler. Reads stay cheap
//! (no locks); the uploader itself is constructed per server start,
//! not per request, so the pipeline is reused across calls.

use std::sync::Arc;

use zpic_config::loader::LoadedConfig;
use zpic_core::upload::Uploader;
use zpic_plugins::UploaderRegistry;

use crate::util::ResolvedUploaderTarget;

/// Bundle of everything a request handler might need. Cheap to clone
/// (every field is already an `Arc` or a reference-counted handle).
#[derive(Clone)]
pub struct AppState {
    /// Resolved zpic config (with the active uploader pinned).
    pub config: Arc<LoadedConfig>,
    /// The instantiated active uploader, ready to accept `UploadRequest`s.
    pub uploader: Arc<dyn Uploader>,
    /// Registry of every uploader type the binary knows about. Used to
    /// render `/config` and to expose type names without re-instantiating.
    pub registry: Arc<UploaderRegistry>,
    /// The configured display name of the active uploader (e.g. `MyBlog`).
    pub active_config_name: Arc<String>,
    /// `Instant::now()` value at server startup; used to compute uptime.
    pub started_at: std::time::Instant,
}

impl AppState {
    /// Build a fresh `AppState` from a resolved uploader target.
    pub fn new(
        config: LoadedConfig,
        registry: UploaderRegistry,
        target: ResolvedUploaderTarget,
    ) -> Result<Self, zpic_core::error::ZpicError> {
        // Convert the `Box<dyn Uploader>` returned by the factory into
        // an `Arc<dyn Uploader>` so the state stays cheap to clone and
        // every request handler can borrow the uploader without owning
        // it.
        let uploader: Arc<dyn Uploader> = Arc::from(target.instantiate()?);
        Ok(Self {
            config: Arc::new(config),
            uploader,
            registry: Arc::new(registry),
            active_config_name: Arc::new(target.config_name),
            started_at: std::time::Instant::now(),
        })
    }

    /// Uptime in seconds, used by the `/health` endpoint.
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
