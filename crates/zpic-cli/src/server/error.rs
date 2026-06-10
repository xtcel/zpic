//! Server-local error type. Wraps [`ZpicError`] in places where we want
//! to attach a [`UploadErrorCode`] for the JSON response, and surfaces
//! request-shaped errors (bad JSON, missing fields, ...) directly.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use thiserror::Error;

use super::models::{UploadErrorCode, UploadResponse};

/// Error type produced by the HTTP layer. Each variant carries a status
/// code, an `UploadErrorCode` for the JSON payload, and a public
/// message (safe to send to the client).
#[derive(Debug, Error)]
pub enum ServerError {
    /// Config file is missing, unreadable, or the active uploader is
    /// not configured.
    #[error("zpic config error: {0}")]
    Config(String),

    /// A file path in the JSON list does not exist on disk.
    #[error("file not found: {0}")]
    FileNotFound(String),

    /// The Content-Type could not be handled (not JSON, not multipart).
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),

    /// The request body could not be deserialized.
    #[error("invalid request body: {0}")]
    BadRequest(String),

    /// The request did not include any files / paths to upload.
    #[error("empty upload: {0}")]
    EmptyUpload(&'static str),

    /// The multipart envelope could not be parsed.
    #[error("multipart error: {0}")]
    Multipart(String),

    /// A field in the multipart envelope was invalid (missing filename,
    /// unknown field name, ...).
    #[error("invalid multipart field: {0}")]
    InvalidField(String),

    /// The uploaded file's extension is not in the allow-list
    /// (see `MEDIA_EXTENSIONS` in `handlers`).
    #[error("invalid file type: {0}")]
    InvalidFileType(String),

    /// Anything that bubbles out of the upload pipeline.
    #[error("upload failed: {0}")]
    Upload(String),

    /// Catch-all for unexpected internal failures.
    #[error("internal server error: {0}")]
    Internal(String),
}

impl ServerError {
    /// Status code returned alongside the JSON payload. PicGo clients
    /// expect 200 for *every* well-formed request, even ones that report
    /// a logical failure in the body. Genuine protocol-level errors
    /// (bad JSON, broken multipart) still surface as 4xx.
    pub fn status(&self) -> StatusCode {
        match self {
            ServerError::Config(_)
            | ServerError::FileNotFound(_)
            | ServerError::InvalidFileType(_)
            | ServerError::EmptyUpload(_)
            | ServerError::UnsupportedContentType(_)
            | ServerError::InvalidField(_)
            | ServerError::Upload(_) => StatusCode::OK,
            ServerError::BadRequest(_) | ServerError::Multipart(_) => StatusCode::BAD_REQUEST,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Wire code used in the JSON payload.
    pub fn code(&self) -> UploadErrorCode {
        match self {
            ServerError::Config(_) => UploadErrorCode::ConfigError,
            ServerError::FileNotFound(_) => UploadErrorCode::FileNotFound,
            ServerError::UnsupportedContentType(_)
            | ServerError::BadRequest(_)
            | ServerError::EmptyUpload(_)
            | ServerError::Multipart(_)
            | ServerError::InvalidField(_) => UploadErrorCode::ServerError,
            ServerError::InvalidFileType(_) => UploadErrorCode::InvalidFileType,
            ServerError::Upload(_) => UploadErrorCode::UploadFailed,
            ServerError::Internal(_) => UploadErrorCode::ServerError,
        }
    }
}

impl From<zpic_core::error::ZpicError> for ServerError {
    fn from(err: zpic_core::error::ZpicError) -> Self {
        use zpic_core::error::ZpicError;
        match err {
            ZpicError::ConfigNotFound | ZpicError::ConfigInvalid(_) => {
                ServerError::Config(err.to_string())
            }
            ZpicError::UploaderNotFound(_)
            | ZpicError::UploaderUnsupported(_)
            | ZpicError::AuthMissing(_)
            | ZpicError::AuthFailed(_) => ServerError::Config(err.to_string()),
            ZpicError::FileNotFound(path) => ServerError::FileNotFound(path.display().to_string()),
            ZpicError::UnsupportedFileType(what) => ServerError::InvalidFileType(what),
            ZpicError::UploadFailed(what) | ZpicError::Network(what) => ServerError::Upload(what),
            other => ServerError::Internal(other.to_string()),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        // Internal errors are logged at error level; everything else
        // stays at warn so the user can see why a request was rejected
        // without flooding the log on every misconfigured client.
        match &self {
            ServerError::Internal(_) | ServerError::Upload(_) => {
                tracing::error!(error = %self, "server error");
            }
            _ => {
                tracing::warn!(error = %self, "client error");
            }
        }

        let payload = UploadResponse::err(UploadResponse::err_msg(&self), self.code());
        let status = self.status();
        let mut response = (status, Json(payload)).into_response();
        // Always advertise a JSON body so clients can `response.json()`
        // unconditionally.
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        response
    }
}

/// Convenience result alias used throughout the server module.
pub type ServerResult<T> = std::result::Result<T, ServerError>;
