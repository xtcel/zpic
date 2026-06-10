//! axum router composition.
//!
//! Keeps the wiring for `POST /upload`, `GET /health`, and `GET /config`
//! in one place. Middleware (CORS, tracing, body limit) is layered on
//! top of the per-route handlers so a future change to the route
//! shape only touches this file.

use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use super::handlers::{config, health, upload};
use super::state::AppState;

/// Build the application router. Caller is responsible for binding
/// it to a listener — see [`crate::server::start`].
pub fn router(state: AppState) -> Router {
    // Permissive CORS: the server binds to `127.0.0.1` by default and
    // only accepts local traffic. Letting *every* origin through keeps
    // mobile Obsidian clients happy without adding attack surface.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/config", get(config))
        .route("/upload", post(upload))
        .with_state(state)
        // Permit 50 MiB bodies by default. Multipart handlers enforce
        // a tighter per-part limit (`MAX_PART_BYTES`) on top of this.
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        // Cap request handling at 60 seconds so a stuck uploader
        // can't tie up a worker forever. The `tower_http` variant
        // produces a status-code response on timeout instead of an
        // `Err`, which fits axum's `Infallible` constraint.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
