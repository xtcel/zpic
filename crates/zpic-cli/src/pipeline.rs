//! Upload orchestration: read a file, compute metadata, render the target
//! key, and dispatch to the active uploader.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;

use zpic_config::loader::LoadedConfig;
use zpic_core::config::ZpicConfig as ZpicConfigTrait;
use zpic_core::error::{Result, ZpicError};
use zpic_core::format::{render_format_for_kind, FormatVars};
use zpic_core::upload::{
    UploadContext, UploadInput, UploadItem, UploadOutput, UploadRequest, Uploader,
};
use zpic_media::{
    content_hash_hex, detect_mime, read_dimensions, render_template, TemplateContext,
};

/// A clipboard image as captured by `arboard`.
pub struct ClipboardImage {
    pub bytes: Bytes,
    /// A reasonable default name to use when persisting or naming the upload.
    pub file_name: String,
    pub mime: String,
}

/// In-memory representation of one upload target.
pub struct PendingUpload {
    pub source_path: PathBuf,
    pub file_name: String,
    pub mime: String,
    pub bytes: Bytes,
    pub explicit_name: Option<String>,
    pub explicit_alt: Option<String>,
}

impl PendingUpload {
    /// Build a `PendingUpload` from a path on disk.
    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(ZpicError::FileNotFound(path.to_path_buf()));
        }
        let bytes = Bytes::from(std::fs::read(path)?);
        let mime = detect_mime(&bytes, Some(path));
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        Ok(Self {
            source_path: path.to_path_buf(),
            file_name,
            mime,
            bytes,
            explicit_name: None,
            explicit_alt: None,
        })
    }

    /// Build a `PendingUpload` from a clipboard image.
    pub fn from_clipboard(image: ClipboardImage) -> Self {
        Self {
            source_path: PathBuf::from("<clipboard>"),
            file_name: image.file_name,
            mime: image.mime,
            bytes: image.bytes,
            explicit_name: None,
            explicit_alt: None,
        }
    }
}

/// Load a file, render the target key, and run the upload.
pub async fn run_upload(
    config: &zpic_config::loader::LoadedConfig,
    uploader: &dyn Uploader,
    pending: PendingUpload,
    dry_run: bool,
) -> Result<UploadOutput> {
    let template = config.zpic.rename.effective_template();
    let hash_hex = content_hash_hex(&pending.bytes);
    let (file_name_for_template, ext) = split_name_ext(&pending.file_name);
    let ctx_template = TemplateContext::new(file_name_for_template, ext, &hash_hex);
    let mut key = render_template(&template, &ctx_template);

    if let Some(name_override) = &pending.explicit_name {
        // Replace the basename of the rendered key.
        let parent = std::path::Path::new(&key)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let key_ext = std::path::Path::new(&key)
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_else(|| ext.to_string());
        key = if parent.is_empty() {
            format!("{}.{}", name_override, key_ext)
        } else {
            format!(
                "{}/{}.{}",
                parent.trim_end_matches('/'),
                name_override,
                key_ext
            )
        };
    }

    let dims = read_dimensions(&pending.bytes).ok().flatten();
    let input = UploadInput::new(
        pending.source_path.clone(),
        pending.file_name.clone(),
        pending.mime.clone(),
        pending.bytes,
    )
    .with_alt(pending.explicit_alt.clone());

    let context = UploadContext {
        target_key: key,
        config: Arc::new(ConfigAdapter {
            inner: config.clone(),
        }) as Arc<dyn ZpicConfigTrait>,
        dry_run,
    };

    let req = UploadRequest { context, input };
    let mut output = uploader.upload(req).await?;
    if output.width.is_none() && output.height.is_none() && dims.is_some() {
        if let Some((w, h)) = dims {
            output.width = Some(w);
            output.height = Some(h);
        }
    }
    Ok(output)
}

/// Split a file name into `(name, ext)`. Returns `("name", "")` if there's
/// no extension.
fn split_name_ext(file_name: &str) -> (&str, &str) {
    match file_name.rfind('.') {
        Some(i) if i > 0 => (&file_name[..i], &file_name[i + 1..]),
        _ => (file_name, ""),
    }
}

/// Adapter that exposes a `LoadedConfig` as a `ZpicConfig` trait object
/// so uploader contexts can carry a reference to the active config.
#[derive(Debug)]
struct ConfigAdapter {
    inner: LoadedConfig,
}

impl ZpicConfigTrait for ConfigAdapter {
    fn source(&self) -> &str {
        self.inner.source.label()
    }
}

/// Convert an `UploadOutput` into a CLI-friendly `UploadItem`.
pub fn to_item(out: UploadOutput) -> UploadItem {
    UploadItem::success(out)
}

/// Render a single upload through the user-selected format.
pub fn render_output(
    out: &UploadOutput,
    format: zpic_core::config::OutputFormat,
    custom_template: Option<&str>,
) -> String {
    render_format_for_kind(format, custom_template, out)
}

/// Suppress unused warnings for `FormatVars` re-export tests.
#[allow(dead_code)]
fn _vars_used(_v: FormatVars<'_>) {}
