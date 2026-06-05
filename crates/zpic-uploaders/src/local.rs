//! Local filesystem uploader. Copies the source file into a target
//! directory and returns a public URL derived from a configured prefix.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use zpic_config::UploaderSection;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{UploadOutput, UploadRequest, Uploader};

/// Copy bytes to a local directory. Suitable for static-site generators.
#[derive(Debug)]
pub struct LocalUploader {
    target_dir: PathBuf,
    public_base_url: String,
}

impl LocalUploader {
    /// Build a local uploader from a `[uploaders.<name>]` config section.
    pub fn from_config(section: &UploaderSection) -> Result<Self> {
        let target_dir = section.string_field("target_dir").trim().to_string();
        if target_dir.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "local uploader requires `target_dir`".into(),
            ));
        }
        let public_base_url = section
            .string_field("public_base_url")
            .trim()
            .trim_end_matches('/')
            .to_string();
        if public_base_url.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "local uploader requires `public_base_url`".into(),
            ));
        }
        Ok(Self {
            target_dir: PathBuf::from(target_dir),
            public_base_url,
        })
    }

    /// Resolve the absolute destination path for a given key.
    pub fn resolve_dest(&self, key: &str) -> PathBuf {
        self.target_dir.join(key)
    }
}

#[async_trait]
impl Uploader for LocalUploader {
    fn name(&self) -> &str {
        "local"
    }

    async fn upload(&self, req: UploadRequest) -> Result<UploadOutput> {
        let dest = self.resolve_dest(&req.context.target_key);
        if !req.context.dry_run {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&dest, &req.input.bytes).await?;
        }
        let url = build_url(&self.public_base_url, &req.context.target_key);
        let markdown = format!("![{}]({})", req.input.file_name, url);
        Ok(UploadOutput {
            source: req.input.source_path.to_string_lossy().into_owned(),
            url,
            key: req.context.target_key,
            markdown,
            mime: req.input.mime,
            size: req.input.size,
            width: None,
            height: None,
            uploader: self.name().to_string(),
        })
    }
}

fn build_url(base: &str, key: &str) -> String {
    let key = key.trim_start_matches('/');
    if base.is_empty() {
        return format!("/{}", key);
    }
    if base.ends_with('/') {
        format!("{}{}", base, key)
    } else {
        format!("{}/{}", base, key)
    }
}

#[allow(dead_code)]
pub(crate) fn parent_dir(path: &Path) -> Option<&Path> {
    path.parent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::sync::Arc;
    use tempfile::tempdir;
    use zpic_core::config::ZpicConfig;
    use zpic_core::upload::{UploadContext, UploadInput};

    #[derive(Debug)]
    struct StubConfig;
    impl ZpicConfig for StubConfig {
        fn source(&self) -> &str {
            "test"
        }
    }

    #[tokio::test]
    async fn writes_file_to_target_dir() {
        let dir = tempdir().unwrap();
        let uploader = LocalUploader {
            target_dir: dir.path().to_path_buf(),
            public_base_url: "/images".into(),
        };
        let input = UploadInput::new(
            PathBuf::from("cover.png"),
            "cover",
            "image/png",
            Bytes::from_static(b"hello"),
        );
        let ctx = UploadContext::new("2026/06/04/cover.png".into(), Arc::new(StubConfig));
        let out = uploader
            .upload(UploadRequest {
                context: ctx,
                input,
            })
            .await
            .unwrap();
        assert_eq!(out.url, "/images/2026/06/04/cover.png");
        assert_eq!(out.uploader, "local");
        let written = fs::read(dir.path().join("2026/06/04/cover.png"))
            .await
            .unwrap();
        assert_eq!(written, b"hello");
    }

    #[tokio::test]
    async fn dry_run_does_not_write() {
        let dir = tempdir().unwrap();
        let uploader = LocalUploader {
            target_dir: dir.path().to_path_buf(),
            public_base_url: "/images".into(),
        };
        let input = UploadInput::new(
            PathBuf::from("cover.png"),
            "cover",
            "image/png",
            Bytes::from_static(b"hello"),
        );
        let mut ctx = UploadContext::new("cover.png".into(), Arc::new(StubConfig));
        ctx.dry_run = true;
        uploader
            .upload(UploadRequest {
                context: ctx,
                input,
            })
            .await
            .unwrap();
        assert!(!dir.path().join("cover.png").exists());
    }
}
