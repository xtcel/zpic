//! Factory: produce the right `Uploader` impl for a `UploaderSection`.
//!
//! The CLI calls `build_uploader(name, section)` and gets back a boxed
//! `Uploader` ready to handle requests.

use std::sync::Arc;

use zpic_config::UploaderSection;
use zpic_core::config::UploaderKind;
use zpic_core::error::Result;
use zpic_core::upload::Uploader;

use crate::github::GitHubUploader;
use crate::local::LocalUploader;
use crate::s3::S3Uploader;

/// Convenience trait that exposes the same `from_config` constructor for
/// every uploader, so the factory can dispatch on the section's `kind`.
pub trait UploaderFactory: Send + Sync {
    fn from_section(section: &UploaderSection) -> Result<Self>
    where
        Self: Sized;
}

impl UploaderFactory for LocalUploader {
    fn from_section(section: &UploaderSection) -> Result<Self> {
        Self::from_config(section)
    }
}

impl UploaderFactory for GitHubUploader {
    fn from_section(section: &UploaderSection) -> Result<Self> {
        Self::from_config(section)
    }
}

impl UploaderFactory for S3Uploader {
    fn from_section(section: &UploaderSection) -> Result<Self> {
        Self::from_config(section)
    }
}

/// Build a boxed `Uploader` from a config section, dispatching on `kind`.
pub fn build_uploader(_name: &str, section: &UploaderSection) -> Result<Box<dyn Uploader>> {
    let uploader: Box<dyn Uploader> = match section.kind {
        UploaderKind::Local => Box::new(LocalUploader::from_section(section)?),
        UploaderKind::Github => Box::new(GitHubUploader::from_section(section)?),
        UploaderKind::S3 => Box::new(S3Uploader::from_section(section)?),
    };
    Ok(uploader)
}

/// Return the canonical name (matches `Uploader::name`) for a kind.
pub fn name_for_kind(kind: UploaderKind) -> &'static str {
    match kind {
        UploaderKind::Local => "local",
        UploaderKind::Github => "github",
        UploaderKind::S3 => "s3",
    }
}

/// Wrap a concrete uploader in an `Arc<dyn Uploader>` for callers that
/// want to share instances across threads.
pub fn shared_uploader(_name: &str, section: &UploaderSection) -> Result<Arc<dyn Uploader>> {
    let boxed = build_uploader(_name, section)?;
    Ok(Arc::from(boxed))
}
