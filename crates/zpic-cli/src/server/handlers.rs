//! HTTP request handlers for the PicGo-compatible endpoints.
//!
//! Handlers stay small: they parse the request, hand the work to the
//! shared [`AppState`], and translate the result into a JSON response.
//! Anything that can fail in interesting ways is funnelled through
//! [`ServerError`] so the error type and the JSON shape stay in sync.

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use chrono::Utc;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, instrument, warn};

use zpic_media::detect_mime;

use crate::pipeline::{run_upload, PendingUpload};

use super::error::{ServerError, ServerResult};
use super::models::{
    ConfigResponse, HealthResponse, PathListRequest, UploadResponse, UploadResultItem,
};
use super::state::AppState;

/// Cap the size of a single multipart file. `25 MiB` is generous for
/// screenshots and small diagrams; larger files should still fit
/// because the multipart parser is bounded.
const MAX_PART_BYTES: usize = 25 * 1024 * 1024;

/// Extension allow-list, mirrored from the Obsidian plugin and the
/// proposal spec. Centralised here so the server and the client agree
/// on what counts as "uploadable media".
///
/// Covers images, audio, and video. `webm` appears in both the audio and
/// video categories upstream but is only listed once here — content-based
/// MIME detection picks the right `audio/webm` vs `video/webm` at the
/// uploader boundary.
const MEDIA_EXTENSIONS: &[&str] = &[
    // images
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "svg", "avif",
    // audio
    "mp3", "flac", "wav", "ogg", "oga", "m4a", "3gp",
    // video
    "mp4", "webm", "ogv",
];

// ---------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------

/// Liveness probe. Returns immediately; never touches the filesystem
/// or the network.
#[instrument(skip_all, fields(http.method = "GET", http.route = "/health"))]
pub async fn health(State(state): State<AppState>) -> Response {
    let body = HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime: state.uptime_seconds(),
    };
    json_ok(body)
}

// ---------------------------------------------------------------------
// GET /config
// ---------------------------------------------------------------------

/// Non-sensitive config dump. Used by the Obsidian plugin to confirm
/// the active uploader and to display the available uploader list.
#[instrument(skip_all, fields(http.method = "GET", http.route = "/config"))]
pub async fn config(State(state): State<AppState>) -> Response {
    let active = state
        .config
        .active_uploader_type()
        .unwrap_or("<none>")
        .to_string();
    let mut uploaders: Vec<String> = state
        .registry
        .descriptors()
        .iter()
        .map(|d| d.type_name.clone())
        .collect();
    uploaders.sort();
    uploaders.dedup();

    let body = ConfigResponse {
        current_uploader: active,
        uploaders,
        version: env!("CARGO_PKG_VERSION"),
    };
    json_ok(body)
}

// ---------------------------------------------------------------------
// POST /upload
// ---------------------------------------------------------------------

/// Dispatch to the right body parser based on the `Content-Type` header.
/// Both modes return the same wire shape so the client doesn't need to
/// branch on the way back.
///
/// When the `Content-Type` header is missing or empty, the handler
/// peeks at the first few bytes of the body to detect the likely
/// envelope (multipart starts with `--`, JSON starts with `{` or `[`)
/// and dispatches accordingly. This makes the server robust against
/// Obsidian's `requestUrl`, which occasionally drops the
/// `Content-Type` header when given a `FormData` body.
#[instrument(skip_all, fields(http.method = "POST", http.route = "/upload"))]
pub async fn upload(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let start = Instant::now();

    // Preserve the raw Content-Type so we can log it verbatim (the
    // boundary may be mixed-case and the casing carries diagnostic
    // value when clients misbehave).
    let raw_content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_type_lower = raw_content_type.to_ascii_lowercase();

    tracing::debug!(
        body_bytes = body.len(),
        content_type = %raw_content_type,
        "upload received"
    );

    let result = if content_type_lower.starts_with("application/json") {
        handle_json(&state, &body).await
    } else if content_type_lower.starts_with("multipart/form-data") {
        // Resolve the boundary once here so the handler doesn't have
        // to re-parse the Content-Type. `multer::parse_boundary` is
        // the same parser the `axum::extract::Multipart` extractor
        // would use internally. We pass the *original* header (not
        // the lowercased one) so the boundary keeps its case.
        match multer::parse_boundary(&raw_content_type) {
            Ok(boundary) => handle_multipart(&state, &body, boundary).await,
            Err(e) => Err(ServerError::BadRequest(format!(
                "invalid multipart boundary: {e}"
            ))),
        }
    } else if raw_content_type.is_empty() {
        // No Content-Type header. This is the case Obsidian's
        // `requestUrl` hits when it forwards a `FormData` body
        // without auto-setting the multipart envelope. Sniff the
        // body to figure out what we got and dispatch accordingly.
        handle_missing_content_type(&state, &body).await
    } else {
        // Unknown Content-Type. Dump every request header and a
        // short body preview into the log so we can diagnose the
        // client (Obsidian, curl, browser DevTools, custom scripts,
        // ...).
        log_unrecognized_request(&headers, &raw_content_type, &body);
        Err(ServerError::UnsupportedContentType(raw_content_type))
    };

    let elapsed_ms = start.elapsed().as_millis();
    match &result {
        Ok(resp) if resp.success => {
            info!(
                elapsed_ms,
                count = resp.result.as_ref().map(|r| r.len()).unwrap_or(0),
                "upload ok"
            );
        }
        Ok(resp) => {
            warn!(
                elapsed_ms,
                msg = resp.msg.as_deref().unwrap_or(""),
                "upload failed"
            );
        }
        Err(_) => {
            // `ServerError::into_response` already logged.
        }
    }
    result
        .map(json_ok_response)
        .unwrap_or_else(|e| e.into_response())
}

/// Wrap an `UploadResponse` in a `200 OK` JSON body.
fn json_ok_response(value: super::models::UploadResponse) -> Response {
    json_ok(value)
}

// ---------------------------------------------------------------------
// Missing / empty Content-Type sniffing
// ---------------------------------------------------------------------

/// Fallback when the request has no `Content-Type` header. We look at
/// the first few bytes of the body to figure out the right parser:
///
/// * `{"list": ...}` -> JSON
/// * `--BOUNDARY...` -> multipart (we have to find the boundary in
///   the body itself because the header isn't there)
///
/// If neither marker is present we return a structured error so the
/// client can render an actionable message.
async fn handle_missing_content_type(
    state: &AppState,
    body: &Bytes,
) -> ServerResult<UploadResponse> {
    let preview = body_preview(body, 64);

    // First non-whitespace byte.
    let first = body.iter().find(|b| !b.is_ascii_whitespace()).copied();

    match first {
        Some(b'{') | Some(b'[') => {
            tracing::info!(
                body_bytes = body.len(),
                "no Content-Type header; body looks like JSON, dispatching as such"
            );
            handle_json(state, body).await
        }
        Some(b'-') if body.len() >= 2 && body[0] == b'-' && body[1] == b'-' => {
            // Multipart body without an explicit Content-Type. We
            // can still parse it because multer locates the
            // boundary in the body itself; we just have to extract
            // the boundary from the leading `--…` line.
            match extract_boundary_from_body(body) {
                Some(boundary) => {
                    tracing::info!(
                        body_bytes = body.len(),
                        boundary = %boundary,
                        "no Content-Type header; body looks like multipart, dispatching as such"
                    );
                    handle_multipart(state, body, boundary).await
                }
                None => {
                    log_unrecognized_request_with_reason(
                        "no Content-Type header and body has no recognizable multipart boundary",
                        &preview,
                    );
                    Err(ServerError::UnsupportedContentType(String::new()))
                }
            }
        }
        _ => {
            log_unrecognized_request_with_reason(
                "no Content-Type header and body does not look like JSON or multipart",
                &preview,
            );
            Err(ServerError::UnsupportedContentType(String::new()))
        }
    }
}

/// Pull the multipart boundary out of the body's leading
/// `--<boundary>` line. Returns `None` if the body doesn't start with
/// a well-formed boundary marker.
fn extract_boundary_from_body(body: &Bytes) -> Option<String> {
    // The boundary sits between the leading `--` and the first CRLF
    // (or LF, for lenient clients). multer itself will validate the
    // envelope later.
    let end = body
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(body.len());
    let line = &body[..end];
    if line.len() < 2 || line[0] != b'-' || line[1] != b'-' {
        return None;
    }
    let boundary = &line[2..];
    if boundary.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(boundary).into_owned())
}

/// Render a short, printable preview of the body for the log line.
fn body_preview(body: &Bytes, limit: usize) -> String {
    let take = body.len().min(limit);
    let mut out = String::with_capacity(take);
    for &b in &body[..take] {
        if b.is_ascii_graphic() || b == b'\r' || b == b'\n' || b == b' ' {
            out.push(b as char);
        } else {
            out.push('.');
        }
    }
    out
}

/// Dump every request header plus a body preview when the
/// Content-Type is something we don't recognise.
fn log_unrecognized_request(headers: &HeaderMap, content_type: &str, body: &Bytes) {
    let pairs: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    tracing::warn!(
        body_bytes = body.len(),
        content_type = %content_type,
        body_preview = %body_preview(body, 120),
        headers = ?pairs,
        "unsupported Content-Type; client should send multipart/form-data or application/json"
    );
}

fn log_unrecognized_request_with_reason(reason: &str, preview: &str) {
    tracing::warn!(reason = %reason, body_preview = %preview, "unrecognized upload request");
}

// ---------------------------------------------------------------------
// JSON path-list mode
// ---------------------------------------------------------------------

async fn handle_json(state: &AppState, body: &Bytes) -> ServerResult<UploadResponse> {
    let req: PathListRequest = serde_json::from_slice(body)
        .map_err(|e| ServerError::BadRequest(format!("invalid JSON body: {e}")))?;

    if req.list.is_empty() {
        return Err(ServerError::EmptyUpload("`list` is empty"));
    }

    let mut urls = Vec::with_capacity(req.list.len());
    let mut full_results = Vec::with_capacity(req.list.len());

    for raw in &req.list {
        let path = PathBuf::from(raw);
        if !path.exists() {
            return Err(ServerError::FileNotFound(raw.clone()));
        }
        let ext_ok = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| {
                let lower = s.to_ascii_lowercase();
                MEDIA_EXTENSIONS.contains(&lower.as_str())
            })
            .unwrap_or(false);
        if !ext_ok {
            return Err(ServerError::InvalidFileType(raw.clone()));
        }

        let pending = PendingUpload::from_path(&path).map_err(ServerError::from)?;
        let output = run_upload(
            state.config.as_ref(),
            state.uploader.as_ref(),
            pending,
            false,
        )
        .await
        .map_err(ServerError::from)?;
        urls.push(output.url.clone());
        full_results.push(UploadResultItem {
            img_url: output.url,
            delete: None,
        });
    }

    info!(count = urls.len(), "json upload complete");
    Ok(UploadResponse::ok(urls, Some(full_results)))
}

// ---------------------------------------------------------------------
// Multipart mode
// ---------------------------------------------------------------------

async fn handle_multipart(
    state: &AppState,
    body: &Bytes,
    boundary: String,
) -> ServerResult<UploadResponse> {
    // multer 3 takes a `Stream<Item = Result<O, E>>`. We yield the
    // buffered body as a single chunk; multer streams internally and
    // does not care about chunk boundaries. Box the stream so its
    // type is unambiguous to the compiler.
    tracing::debug!(body_bytes = body.len(), boundary = %boundary, "multipart parse begin");
    let body_owned = body.clone();
    let stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<Bytes, std::io::Error>> + Send>,
    > = Box::pin(futures::stream::once(async move {
        Ok::<Bytes, std::io::Error>(body_owned)
    }));
    let mut multipart = multer::Multipart::new(stream, boundary);

    let mut urls = Vec::new();
    let mut full_results = Vec::new();
    let mut saved_paths: Vec<PathBuf> = Vec::new();

    loop {
        let field: multer::Field<'_> = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                cleanup(&saved_paths).await;
                return Err(ServerError::Multipart(e.to_string()));
            }
        };

        let name = field.name().unwrap_or("").to_string();
        if name != "list" {
            debug!(field = %name, "skipping unknown multipart field");
            // Drain the field so we don't corrupt the stream for the
            // next iteration.
            let _ = field.bytes().await;
            continue;
        }

        let file_name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("upload-{}.bin", Utc::now().timestamp_millis()));

        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                cleanup(&saved_paths).await;
                return Err(ServerError::Multipart(e.to_string()));
            }
        };

        if bytes.len() > MAX_PART_BYTES {
            cleanup(&saved_paths).await;
            return Err(ServerError::BadRequest(format!(
                "file `{file_name}` exceeds {} bytes",
                MAX_PART_BYTES
            )));
        }

        let safe_name = sanitize_file_name(&file_name);
        let temp_path = match write_temp_file(&safe_name, &bytes).await {
            Ok(p) => p,
            Err(e) => {
                cleanup(&saved_paths).await;
                return Err(e);
            }
        };
        saved_paths.push(temp_path.clone());

        match run_one(state, &temp_path, &safe_name, &bytes).await {
            Ok(output) => {
                urls.push(output.url.clone());
                full_results.push(UploadResultItem {
                    img_url: output.url,
                    delete: None,
                });
            }
            Err(e) => {
                cleanup(&saved_paths).await;
                return Err(e);
            }
        }
    }

    cleanup(&saved_paths).await;

    if urls.is_empty() {
        return Err(ServerError::EmptyUpload(
            "no `list` field in multipart body",
        ));
    }

    info!(count = urls.len(), "multipart upload complete");
    Ok(UploadResponse::ok(urls, Some(full_results)))
}

/// Run a single upload using the in-memory bytes and the temp file
/// path. We always keep the temp file around for the duration of the
/// upload so the existing pipeline can stream from disk if the
/// uploader wants to.
async fn run_one(
    state: &AppState,
    path: &Path,
    safe_name: &str,
    bytes: &Bytes,
) -> ServerResult<zpic_core::upload::UploadOutput> {
    let mime = detect_mime(bytes, Some(path));
    let pending = PendingUpload {
        source_path: path.to_path_buf(),
        file_name: safe_name.to_string(),
        mime,
        bytes: bytes.clone(),
        explicit_name: None,
        explicit_alt: None,
    };
    run_upload(
        state.config.as_ref(),
        state.uploader.as_ref(),
        pending,
        false,
    )
    .await
    .map_err(ServerError::from)
}

// ---------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------

/// Replace anything that isn't `[A-Za-z0-9._-]` in a filename with `_`
/// to keep temp files on the safe side of every filesystem.
fn sanitize_file_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return format!("upload-{}.bin", Utc::now().timestamp_millis());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out == "." || out == ".." {
        out = format!("_{out}");
    }
    out
}

/// Write the bytes into a uniquely-named temp file under the system
/// temp dir. Returns the absolute path on success.
async fn write_temp_file(safe_name: &str, bytes: &[u8]) -> ServerResult<PathBuf> {
    let dir = std::env::temp_dir().join("zpic-uploads");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ServerError::Internal(format!("create temp dir: {e}")))?;
    let stamp = Utc::now().timestamp_millis();
    let file_name = format!("zpic-{stamp}-{safe_name}");
    let path = dir.join(file_name);
    let mut f = tokio::fs::File::create(&path)
        .await
        .map_err(|e| ServerError::Internal(format!("create temp file: {e}")))?;
    f.write_all(bytes)
        .await
        .map_err(|e| ServerError::Internal(format!("write temp file: {e}")))?;
    f.flush()
        .await
        .map_err(|e| ServerError::Internal(format!("flush temp file: {e}")))?;
    Ok(path)
}

/// Best-effort cleanup of any temp files we created. Errors are
/// swallowed; the OS will eventually sweep the temp directory on
/// reboot.
async fn cleanup(paths: &[PathBuf]) {
    for p in paths {
        if let Err(e) = tokio::fs::remove_file(p).await {
            // ENOENT is fine — another handler may have raced us.
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(path = %p.display(), error = %e, "could not remove temp file");
            }
        }
    }
}

/// JSON response with a stable `Content-Type: application/json` header.
/// Centralising the header tweak avoids accidental `text/plain` from
/// `axum::Json` when the body is `null`.
fn json_ok<T: serde::Serialize>(value: T) -> Response {
    let mut resp = (StatusCode::OK, Json(value)).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp
}
