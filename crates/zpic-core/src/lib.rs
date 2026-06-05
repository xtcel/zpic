//! Core data models, error types, and uploader trait shared by every zpic surface.

pub mod config;
pub mod error;
pub mod upload;
pub mod format;

pub use error::{Result, ZpicError};
pub use config::{OutputFormat, RenameStrategy, UploaderKind, ZpicConfig};
pub use upload::{
    UploadContext, UploadInput, UploadItem, UploadOutput, UploadReport, UploadRequest, Uploader,
};
pub use format::{render_format, render_format_for_kind};
