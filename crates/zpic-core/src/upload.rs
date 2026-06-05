//! Uploader trait and request/response payloads.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::config::ZpicConfig;
use crate::error::Result;

/// Inputs passed to an uploader.
///
/// `bytes` and `size` are pre-loaded by the CLI; uploaders do not need to
/// re-read the file. `alt` is the optional `alt` text used when the markdown
/// output is rendered.
#[derive(Debug, Clone)]
pub struct UploadInput {
    /// Original path of the file on disk, retained for diagnostics and history.
    pub source_path: PathBuf,
    /// Sanitized file name (used in default `target_key` templates).
    pub file_name: String,
    /// Detected MIME type, e.g. `image/png`.
    pub mime: String,
    /// File contents, already read.
    pub bytes: Bytes,
    /// Size in bytes.
    pub size: u64,
    /// Optional alt text carried from CLI flags or markdown reference.
    pub alt: Option<String>,
}

impl UploadInput {
    /// Construct a new `UploadInput` from raw bytes.
    pub fn new(
        source_path: PathBuf,
        file_name: impl Into<String>,
        mime: impl Into<String>,
        bytes: Bytes,
    ) -> Self {
        let size = bytes.len() as u64;
        Self {
            source_path,
            file_name: file_name.into(),
            mime: mime.into(),
            bytes,
            size,
            alt: None,
        }
    }

    /// Builder-style setter for `alt`.
    pub fn with_alt(mut self, alt: Option<String>) -> Self {
        self.alt = alt;
        self
    }
}

/// Per-call context passed to an uploader.
#[derive(Debug, Clone)]
pub struct UploadContext {
    /// The fully-resolved object key the uploader should write to.
    pub target_key: String,
    /// Shared reference to the resolved zpic config.
    pub config: Arc<dyn ZpicConfig>,
    /// `true` when the caller wants a "what would happen" run without writes.
    pub dry_run: bool,
}

impl UploadContext {
    /// Convenience constructor used by the CLI.
    pub fn new(target_key: String, config: Arc<dyn ZpicConfig>) -> Self {
        Self {
            target_key,
            config,
            dry_run: false,
        }
    }
}

/// Per-file upload result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadOutput {
    /// Absolute or as-supplied path to the file that was uploaded.
    pub source: String,
    /// Final public URL exposed to the user.
    pub url: String,
    /// Resolved object key (path inside the bucket or repository).
    pub key: String,
    /// Rendered markdown referencing the URL, e.g. `![alt](url)`.
    pub markdown: String,
    /// Detected MIME type.
    pub mime: String,
    /// File size in bytes.
    pub size: u64,
    /// Image width in pixels, if known.
    pub width: Option<u32>,
    /// Image height in pixels, if known.
    pub height: Option<u32>,
    /// Identifier of the uploader used (matches `Uploader::name`).
    pub uploader: String,
}

/// Aggregated result returned from a multi-file `upload` invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadReport {
    /// `true` if every requested item was uploaded successfully.
    pub success: bool,
    /// Per-file results. Failed files are still present with placeholder
    /// fields so the JSON shape stays stable for integrations.
    pub items: Vec<UploadItem>,
}

impl UploadReport {
    /// Build a successful single-item report.
    pub fn single(item: UploadItem) -> Self {
        Self {
            success: item.error.is_none(),
            items: vec![item],
        }
    }

    /// Build a report from many items; success means none had an error.
    pub fn from_items(items: Vec<UploadItem>) -> Self {
        let success = items.iter().all(|i| i.error.is_none());
        Self { success, items }
    }
}

/// Single item in an `UploadReport`. When an upload failed, `result` will
/// be `None` and `error` will describe what went wrong.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadItem {
    pub source: String,
    /// The original requested source path or `<clipboard>` placeholder.
    pub url: Option<String>,
    pub key: Option<String>,
    pub markdown: Option<String>,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub uploader: Option<String>,
    /// Error message; `None` means success.
    pub error: Option<String>,
}

impl UploadItem {
    /// Build a successful item from an `UploadOutput`.
    pub fn success(out: UploadOutput) -> Self {
        Self {
            source: out.source,
            url: Some(out.url),
            key: Some(out.key),
            markdown: Some(out.markdown),
            mime: Some(out.mime),
            size: Some(out.size),
            width: out.width,
            height: out.height,
            uploader: Some(out.uploader),
            error: None,
        }
    }

    /// Build a failed item with a diagnostic message.
    pub fn failure(source: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            url: None,
            key: None,
            markdown: None,
            mime: None,
            size: None,
            width: None,
            height: None,
            uploader: None,
            error: Some(error.into()),
        }
    }
}

/// Per-file request the CLI hands to the uploader registry. Bundles the
/// input bytes together with the resolved context so uploaders only have to
/// implement one method.
#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub context: UploadContext,
    pub input: UploadInput,
}

/// Every uploader implements this trait. Implementations live in the
/// `zpic-uploaders` crate; the `zpic-cli` orchestrator picks one based on
/// config / CLI flags.
#[async_trait]
pub trait Uploader: Send + Sync {
    /// Stable identifier used in CLI flags and config files.
    fn name(&self) -> &str;

    /// Run the upload. Returning `Err` causes the CLI to mark the file as
    /// failed and continue with the rest of the batch.
    async fn upload(&self, request: UploadRequest) -> Result<UploadOutput>;
}
