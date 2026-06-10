//! Shared error type for the zpic workspace.

use std::path::PathBuf;

/// Result alias used across the zpic workspace.
pub type Result<T> = std::result::Result<T, ZpicError>;

/// All zpic operations return `ZpicError`. Variants are deliberately coarse so
/// that integration consumers can map them to actionable messages.
#[derive(Debug, thiserror::Error)]
pub enum ZpicError {
    #[error("config file not found in any of the standard locations")]
    ConfigNotFound,

    #[error("config is invalid: {0}")]
    ConfigInvalid(String),

    #[error("uploader '{0}' is not configured")]
    UploaderNotFound(String),

    #[error("uploader '{0}' is not supported by zpic")]
    UploaderUnsupported(String),

    #[error("missing credential: {0}")]
    AuthMissing(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("upload failed: {0}")]
    UploadFailed(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("toml parse error: {0}")]
    Toml(String),

    #[error("clipboard error: {0}")]
    Clipboard(String),

    #[error("history store error: {0}")]
    History(String),

    #[error("migration error: {0}")]
    Migration(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ZpicError {
    /// Return a short, user-actionable remediation hint for the most common
    /// failure modes. Returns `None` if no hint is known.
    pub fn remediation(&self) -> Option<&'static str> {
        match self {
            ZpicError::ConfigNotFound => Some(
                "Run `zpic config init` to create a starter config, or pass `--config <path>`.",
            ),
            ZpicError::ConfigInvalid(_) => Some(
                "Validate the config file syntax; see the field mentioned in the error message.",
            ),
            ZpicError::UploaderNotFound(_) => Some(
                "Create it with `zpic set uploader <type> <name>` or switch to an existing one with `zpic use uploader <type> <name>`.",
            ),
            ZpicError::UploaderUnsupported(_) => Some(
                "Switch to a built-in uploader (local, github, s3, aliyun-oss) or import a supported PicGo config.",
            ),
            ZpicError::AuthMissing(_) => Some(
                "Provide the credential as an environment variable or in the config file.",
            ),
            ZpicError::AuthFailed(_) => Some(
                "Verify the credential is still valid; regenerate the token if needed.",
            ),
            ZpicError::FileNotFound(_) => Some("Check that the file path exists and is readable."),
            ZpicError::UnsupportedFileType(_) => Some(
                "zpic uploads images (png, jpg, jpeg, gif, webp, bmp, svg, tiff, avif), \
                 audio (mp3, flac, wav, ogg, m4a, 3gp), and video (mp4, webm, ogv).",
            ),
            ZpicError::Clipboard(_) => Some(
                "Copy an image to the clipboard before running `zpic upload --clipboard`.",
            ),
            ZpicError::History(_) => Some(
                "Check filesystem permissions on the history store path; see `zpic doctor`.",
            ),
            _ => None,
        }
    }
}
