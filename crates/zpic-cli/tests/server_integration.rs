//! End-to-end tests for the PicGo-compatible HTTP server.
//!
//! Each test boots the server on a random loopback port, points it at a
//! temp `local` uploader, exercises one wire scenario, and shuts the
//! server back down. Network access is never required: every test
//! uses the in-tree `local` uploader so the round-trip is just
//! `request → file on disk → URL`.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

/// Smallest valid PNG (1×1 transparent). 67 bytes.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Write a zpic config that points its `local` uploader at the given
/// temp dir, then return the path to the config file.
fn write_local_config(dir: &TempDir) -> std::path::PathBuf {
    let target = dir.path().join("public");
    let cfg = format!(
        r#"
default_uploader = "local"
default_format = "markdown"
copy_after_upload = false
history_enabled = false

[rename]
strategy = "date-hash"
path = "images/{{yyyy}}/{{mm}}/{{dd}}/{{hash8}}.{{ext}}"

[format]
markdown = "![{{alt}}]({{url}})"

[uploaders.local]
type = "local"
target_dir = "{}"
public_base_url = "http://127.0.0.1:0/images"
"#,
        target.display()
    );
    let path = dir.path().join("config.toml");
    std::fs::write(&path, cfg).unwrap();
    path
}

struct TestServer {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestServer {
    async fn start(config: &Path) -> Self {
        // Bind a random port and immediately drop the listener so the
        // server can re-bind to the same address. (`SO_REUSEADDR` would
        // be more elegant but `TcpListener::bind` + drop is portable.)
        let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);

        let config = config.to_path_buf();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            // The server's start() never returns until it errors or
            // gets a shutdown signal. We use the same code path the
            // CLI uses so the test exercises the real wiring.
            let options = zpic_cli::server::ServerOptions { bind: addr };
            let _ = zpic_cli::server::start(options, Some(config), async {
                let _ = shutdown_rx.await;
            })
            .await;
        });

        // Wait for the server to start listening.
        let start = std::time::Instant::now();
        loop {
            if TcpListener::bind(addr).await.is_err() {
                // Port is taken → server is up.
                break;
            }
            if start.elapsed() > Duration::from_secs(5) {
                panic!("server did not start listening on {addr}");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        Self {
            addr,
            handle,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Give the server a moment to release the socket.
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.handle.abort();
    }
}

fn rt() -> &'static Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("tokio runtime"))
}

fn run<F: std::future::Future>(future: F) -> F::Output {
    rt().block_on(future)
}

#[test]
fn health_endpoint_reports_ok() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/health");
    let body: Value = run(async {
        let text = reqwest_get(&url).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
    assert!(body["uptime"].is_number());

    run(server.shutdown());
}

#[test]
fn config_endpoint_lists_uploader_metadata() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/config");
    let body: Value = run(async {
        let text = reqwest_get(&url).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["currentUploader"], "local");
    assert!(
        body["uploaders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "local"),
        "uploaders array should include `local`, got: {}",
        body["uploaders"]
    );

    run(server.shutdown());
}

#[test]
fn json_path_upload_writes_file_to_disk() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let png_path = dir.path().join("cover.png");
    std::fs::write(&png_path, TINY_PNG).unwrap();

    let server = run(async { TestServer::start(&cfg).await });

    let body: Value = run(async {
        let url = server.url("/upload");
        let payload = serde_json::json!({ "list": [png_path.display().to_string()] });
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], true);
    let urls = body["result"].as_array().unwrap();
    assert_eq!(urls.len(), 1);
    let full = body["fullResult"].as_array().unwrap();
    assert_eq!(full.len(), 1);
    assert_eq!(full[0]["imgUrl"], urls[0]);

    // The local uploader must have actually written the file to disk.
    let stored: Vec<_> = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();
    assert_eq!(stored.len(), 1, "exactly one file should be stored");

    run(server.shutdown());
}

#[test]
fn multipart_upload_writes_file_to_disk() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    // Build a multipart body by hand so the test is independent of
    // any client library. The boundary is arbitrary; the server parses
    // it out of the Content-Type.
    let boundary = "----zpic-test-boundary-mxq4c7";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"list\"; filename=\"clipboard.png\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    body.extend_from_slice(TINY_PNG);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let url = server.url("/upload");
    let content_type = format!("multipart/form-data; boundary={boundary}");

    let body_json: Value = run(async {
        let text = reqwest_post_multipart(&url, &content_type, body).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body_json["success"], true);
    let urls = body_json["result"].as_array().unwrap();
    assert_eq!(urls.len(), 1);
    let full = body_json["fullResult"].as_array().unwrap();
    assert_eq!(full[0]["imgUrl"], urls[0]);

    // Same disk-side check as the JSON case.
    let stored: Vec<_> = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();
    assert_eq!(stored.len(), 1, "exactly one file should be stored");

    run(server.shutdown());
}

#[test]
fn json_path_rejects_missing_file() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/upload");
    let payload = serde_json::json!({ "list": ["/definitely/not/here.png"] });
    let body: Value = run(async {
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "FILE_NOT_FOUND");

    run(server.shutdown());
}

#[test]
fn json_path_rejects_disallowed_extension() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let txt = dir.path().join("note.txt");
    std::fs::write(&txt, "not a media file").unwrap();
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/upload");
    let payload = serde_json::json!({ "list": [txt.display().to_string()] });
    let body: Value = run(async {
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "INVALID_FILE_TYPE");

    run(server.shutdown());
}

#[test]
fn json_path_accepts_audio_upload() {
    // An MP3 with a valid ID3 header should pass the allow-list, get
    // detected as `audio/mpeg`, and be persisted under the same local
    // uploader path as an image.
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let mp3_path = dir.path().join("track.mp3");
    // Minimal ID3v2 header: "ID3" + version + flags + size. The exact
    // body is irrelevant — we only need the magic for MIME detection.
    let mp3_bytes: &[u8] = b"ID3\x04\x00\x00\x00\x00\x00\x00zpic-test-audio";
    std::fs::write(&mp3_path, mp3_bytes).unwrap();

    let server = run(async { TestServer::start(&cfg).await });

    let body: Value = run(async {
        let url = server.url("/upload");
        let payload = serde_json::json!({ "list": [mp3_path.display().to_string()] });
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], true, "expected mp3 upload to succeed; got {body}");
    let urls = body["result"].as_array().unwrap();
    assert_eq!(urls.len(), 1);

    // The local uploader should have written the file with the .mp3
    // extension preserved by the rename template.
    let stored: Vec<_> = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();
    assert_eq!(stored.len(), 1, "exactly one file should be stored");
    let stored_name = stored[0].file_name().to_string_lossy().into_owned();
    assert!(
        stored_name.ends_with(".mp3"),
        "stored file should keep the .mp3 extension; got {stored_name}"
    );

    run(server.shutdown());
}

#[test]
fn json_path_accepts_video_upload() {
    // A 12-byte MP4 ftyp box is enough for `infer` to recognise the file
    // as `video/mp4`. The rest of the bytes are filler.
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let mp4_path = dir.path().join("clip.mp4");
    let mp4_bytes: [u8; 12] = [
        0x00, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm',
    ];
    std::fs::write(&mp4_path, mp4_bytes).unwrap();

    let server = run(async { TestServer::start(&cfg).await });

    let body: Value = run(async {
        let url = server.url("/upload");
        let payload = serde_json::json!({ "list": [mp4_path.display().to_string()] });
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], true, "expected mp4 upload to succeed; got {body}");

    let stored: Vec<_> = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();
    assert_eq!(stored.len(), 1, "exactly one file should be stored");
    let stored_name = stored[0].file_name().to_string_lossy().into_owned();
    assert!(
        stored_name.ends_with(".mp4"),
        "stored file should keep the .mp4 extension; got {stored_name}"
    );

    run(server.shutdown());
}

#[test]
fn unsupported_content_type_reports_server_error() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/upload");
    let body: Value = run(async {
        let text = reqwest_post_text(&url, "text/plain", "hello").await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "SERVER_ERROR");
    let msg = body["msg"].as_str().unwrap();
    assert!(
        msg.contains("text/plain"),
        "message should mention the rejected content type: {msg}"
    );
    assert!(
        msg.contains("multipart/form-data"),
        "message should hint at the correct content type: {msg}"
    );

    run(server.shutdown());
}

#[test]
fn missing_content_type_falls_back_to_multipart() {
    // Obsidian's `requestUrl` is known to occasionally drop the
    // Content-Type header when handed a FormData body. The server
    // should sniff the body and dispatch as multipart in that case.
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let boundary = "----zpic-missing-content-type-test";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"list\"; filename=\"clipboard.png\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    body.extend_from_slice(TINY_PNG);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let url = server.url("/upload");
    let body_json: Value = run(async {
        // Intentionally send no Content-Type header. The server has
        // to sniff and dispatch.
        let text = reqwest_post_bytes(&url, None, body).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(
        body_json["success"], true,
        "expected success without Content-Type; got {body_json}"
    );
    assert_eq!(body_json["result"].as_array().unwrap().len(), 1);

    let stored: Vec<_> = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .collect();
    assert_eq!(stored.len(), 1, "exactly one file should be stored");

    run(server.shutdown());
}

#[test]
fn missing_content_type_falls_back_to_json() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let png = dir.path().join("a.png");
    std::fs::write(&png, TINY_PNG).unwrap();
    let server = run(async { TestServer::start(&cfg).await });

    let payload = serde_json::json!({ "list": [png.display().to_string()] });
    let body: Value = run(async {
        let url = server.url("/upload");
        // No Content-Type: server should still recognise the JSON body.
        let text = reqwest_post_bytes(&url, None, payload.to_string().into_bytes()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], true);
    assert_eq!(body["result"].as_array().unwrap().len(), 1);

    run(server.shutdown());
}

#[test]
fn empty_body_with_no_content_type_reports_server_error() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/upload");
    let body: Value = run(async {
        // No Content-Type, no body. Should still produce a structured
        // error rather than crashing.
        let text = reqwest_post_bytes(&url, None, Vec::new()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], false);
    assert_eq!(body["code"], "SERVER_ERROR");
    let msg = body["msg"].as_str().unwrap();
    assert!(
        msg.contains("Content-Type"),
        "error message should reference Content-Type: {msg}"
    );

    run(server.shutdown());
}

#[test]
fn concurrent_uploads_do_not_collide() {
    let dir = TempDir::new().unwrap();
    let cfg = write_local_config(&dir);
    let server = run(async { TestServer::start(&cfg).await });

    let url = server.url("/upload");
    // Use three distinct payloads (a single byte flipped per file) so
    // the local uploader doesn't content-hash-collapse them into one
    // stored file.
    let payload = serde_json::json!({
        "list": (0..3)
            .map(|i| {
                let mut bytes = TINY_PNG.to_vec();
                let last = bytes.len() - 2;
                bytes[last] = bytes[last].wrapping_add(i as u8);
                let p = dir.path().join(format!("img-{i}.png"));
                std::fs::write(&p, &bytes).unwrap();
                p.display().to_string()
            })
            .collect::<Vec<_>>()
    });

    let body: Value = run(async {
        let text = reqwest_post_json(&url, &payload.to_string()).await;
        serde_json::from_str(&text).unwrap()
    });

    assert_eq!(body["success"], true);
    let urls = body["result"].as_array().unwrap();
    assert_eq!(urls.len(), 3);
    let stored = walkdir::WalkDir::new(dir.path().join("public"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .count();
    assert_eq!(stored, 3);

    run(server.shutdown());
}

// ---- minimal HTTP client helpers --------------------------------------
//
// We don't want to take a hard dependency on `reqwest` from the test
// binary (the runtime does, but the tests should be portable across
// feature flags), so the helpers below build a request with the
// in-tree `reqwest` crate only via the existing transitive dep.

async fn reqwest_get(url: &str) -> String {
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .expect("GET succeeds");
    let status = response.status();
    let text = response.text().await.expect("read body");
    assert!(status.is_success(), "GET {url} returned {status}: {text}");
    text
}

async fn reqwest_post_json(url: &str, body: &str) -> String {
    let response = reqwest::Client::new()
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .expect("POST succeeds");
    let status = response.status();
    let text = response.text().await.expect("read body");
    assert!(
        status.is_success() || status.as_u16() == 400,
        "POST {url} returned {status}: {text}"
    );
    text
}

async fn reqwest_post_multipart(url: &str, content_type: &str, body: Vec<u8>) -> String {
    let response = reqwest::Client::new()
        .post(url)
        .header("Content-Type", content_type)
        .body(body)
        .send()
        .await
        .expect("POST succeeds");
    let status = response.status();
    let text = response.text().await.expect("read body");
    assert!(
        status.is_success() || status.as_u16() == 400,
        "POST {url} returned {status}: {text}"
    );
    text
}

async fn reqwest_post_text(url: &str, content_type: &str, body: &str) -> String {
    let response = reqwest::Client::new()
        .post(url)
        .header("Content-Type", content_type)
        .body(body.to_string())
        .send()
        .await
        .expect("POST succeeds");
    response.text().await.expect("read body")
}

/// POST `body` with an *optional* Content-Type. Passing `None` sends
/// the request with no Content-Type header at all, which is the
/// scenario the Obsidian fallback path is built for.
async fn reqwest_post_bytes(url: &str, content_type: Option<&str>, body: Vec<u8>) -> String {
    let mut request = reqwest::Client::new().post(url).body(body);
    if let Some(ct) = content_type {
        request = request.header("Content-Type", ct);
    }
    let response = request.send().await.expect("POST succeeds");
    let status = response.status();
    let text = response.text().await.expect("read body");
    assert!(
        status.is_success() || status.as_u16() == 400,
        "POST {url} returned {status}: {text}"
    );
    text
}
