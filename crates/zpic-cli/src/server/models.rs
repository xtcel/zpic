//! Wire models for the PicGo-compatible HTTP API.
//!
//! The shapes here intentionally mirror what the [Obsidian plugin][1] and
//! other PicGo clients expect. Renaming a field is a breaking change to
//! the public API and should be done deliberately.
//!
//! [1]: ../../../../openspec/changes/add-http-server-for-obsidian/specs/api-specification.md

use serde::{Deserialize, Serialize};

/// JSON body of `POST /upload` in the "path list" mode.
///
/// The `list` field carries absolute paths to files that already exist on
/// the server's filesystem. Desktop Obsidian uses this mode because the
/// file picker hands it real paths.
#[derive(Debug, Clone, Deserialize)]
pub struct PathListRequest {
    /// Absolute paths to upload, in order.
    pub list: Vec<String>,
}

/// Successful `/upload` response. Always wrapped in an outer
/// `success: true` discriminator so clients can branch without reading
/// a separate HTTP status code.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    /// `true` when every requested file was uploaded.
    pub success: bool,
    /// Public URLs of the uploaded files, in the same order as the
    /// request. `None` only when the entire request failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Vec<String>>,
    /// Per-file extended metadata. Used by uploaders that expose a
    /// delete token; otherwise it is omitted to keep responses small.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_result: Option<Vec<UploadResultItem>>,
    /// Human-readable error message when `success` is `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    /// Machine-readable error code; see [`UploadErrorCode`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<UploadErrorCode>,
}

impl UploadResponse {
    /// Build a successful response from a list of URLs and optional
    /// per-file metadata.
    pub fn ok(result: Vec<String>, full_result: Option<Vec<UploadResultItem>>) -> Self {
        Self {
            success: true,
            result: Some(result),
            full_result,
            msg: None,
            code: None,
        }
    }

    /// Build an error response. `success` is always `false` here.
    pub fn err(msg: impl Into<String>, code: UploadErrorCode) -> Self {
        Self {
            success: false,
            result: None,
            full_result: None,
            msg: Some(msg.into()),
            code: Some(code),
        }
    }

    /// Human-readable message for a `ServerError`. Some variants
    /// (notably `UnsupportedContentType`) need a hand-tuned message
    /// because the auto-derived `Display` is too terse for the
    /// Obsidian plugin's `Notice` UI.
    pub fn err_msg(err: &super::error::ServerError) -> String {
        use super::error::ServerError;
        match err {
            ServerError::UnsupportedContentType(actual) => {
                let actual_display = if actual.is_empty() {
                    "<empty>".to_string()
                } else {
                    actual.clone()
                };
                format!(
                    "zpic server did not recognise the request Content-Type (`{actual_display}`). \
                     Set `Content-Type: multipart/form-data; boundary=...` for file uploads, \
                     or `application/json` for the path-list mode. See the server log for details."
                )
            }
            other => other.to_string(),
        }
    }

    /// Build an error response from a partial batch — e.g. some files
    /// uploaded successfully while one or more failed. The HTTP status
    /// is still 200 (PicGo semantics) but `success` is `false` and the
    /// `result` is the partial list with the failed slots dropped.
    pub fn partial(result: Vec<String>, msg: impl Into<String>, code: UploadErrorCode) -> Self {
        Self {
            success: false,
            result: Some(result),
            full_result: None,
            msg: Some(msg.into()),
            code: Some(code),
        }
    }
}

/// One entry in the optional `fullResult` array. Matches the PicGo
/// `delete` token convention so clients can offer undo support.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResultItem {
    /// Public URL of the uploaded image.
    pub img_url: String,
    /// Optional delete URL the uploader exposes for the asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<String>,
}

/// Machine-readable error codes that mirror the spec. Clients switch on
/// these to render targeted UI without parsing free-form messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UploadErrorCode {
    /// File extension is not in the allow-list (covers image, audio, video).
    InvalidFileType,
    /// A path in the JSON list does not exist on disk.
    FileNotFound,
    /// Uploader returned a non-success result.
    UploadFailed,
    /// Config file is missing or the active uploader is not configured.
    ConfigError,
    /// Anything else (panic, internal invariant violation, ...).
    ServerError,
}

impl UploadErrorCode {
    /// String form used in JSON output. Equivalent to `serde_json::to_string`
    /// but avoids an allocation on the hot path.
    pub fn as_str(self) -> &'static str {
        match self {
            UploadErrorCode::InvalidFileType => "INVALID_FILE_TYPE",
            UploadErrorCode::FileNotFound => "FILE_NOT_FOUND",
            UploadErrorCode::UploadFailed => "UPLOAD_FAILED",
            UploadErrorCode::ConfigError => "CONFIG_ERROR",
            UploadErrorCode::ServerError => "SERVER_ERROR",
        }
    }
}

/// `GET /health` response. `status` is always `"ok"` while the server
/// is able to accept requests.
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    /// zpic version reported via `CARGO_PKG_VERSION`.
    pub version: &'static str,
    /// Seconds since the server started accepting connections.
    pub uptime: u64,
}

/// `GET /config` response. Intentionally omits any field whose value
/// would leak a credential.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// Active uploader type identifier (e.g. `github`, `s3`, `local`).
    pub current_uploader: String,
    /// Sorted list of uploader types currently registered.
    pub uploaders: Vec<String>,
    /// zpic version reported via `CARGO_PKG_VERSION`.
    pub version: &'static str,
}
