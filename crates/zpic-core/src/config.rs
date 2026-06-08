//! Native zpic configuration model and enums shared with the config crate.
//!
//! The full `ZpicConfig` struct lives in the `zpic-config` crate, but the
//! value types referenced everywhere (uploader kinds, output formats,
//! rename strategies) are defined here so the rest of the workspace can use
//! them without pulling in filesystem-dependent code.

use serde::{Deserialize, Serialize};

/// What an `Uploader` advertises itself as. Used by config validation, the
/// CLI `--uploader` flag, and the PicGo compatibility layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UploaderKind {
    Local,
    Github,
    S3,
    AliyunOss,
}

impl UploaderKind {
    /// All uploader kinds in declaration order. Used by the PicGo
    /// compatibility layer and the CLI help text.
    pub fn all() -> [UploaderKind; 4] {
        [
            UploaderKind::Local,
            UploaderKind::Github,
            UploaderKind::S3,
            UploaderKind::AliyunOss,
        ]
    }

    /// Stable, lowercase name used in CLI flags and config files.
    pub fn as_str(&self) -> &'static str {
        match self {
            UploaderKind::Local => "local",
            UploaderKind::Github => "github",
            UploaderKind::S3 => "s3",
            UploaderKind::AliyunOss => "aliyun-oss",
        }
    }

    /// Return the PicGo `picBed.uploader` token that maps to this kind.
    pub fn picgo_aliases(&self) -> &'static [&'static str] {
        match self {
            UploaderKind::Local => &["local"],
            UploaderKind::Github => &["github"],
            UploaderKind::S3 => &["s3", "aws-s3", "r2"],
            UploaderKind::AliyunOss => &["aliyun-oss", "oss", "aliyun"],
        }
    }

    /// Resolve a CLI/config token into a built-in uploader kind.
    pub fn from_alias(value: &str) -> Option<Self> {
        let value = value.trim();
        UploaderKind::all().into_iter().find(|kind| {
            kind.picgo_aliases()
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(value))
        })
    }
}

/// Output rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Markdown,
    Url,
    Html,
    Jsx,
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Markdown
    }
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Markdown => "markdown",
            OutputFormat::Url => "url",
            OutputFormat::Html => "html",
            OutputFormat::Jsx => "jsx",
            OutputFormat::Json => "json",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "markdown" | "md" => Some(OutputFormat::Markdown),
            "url" => Some(OutputFormat::Url),
            "html" => Some(OutputFormat::Html),
            "jsx" => Some(OutputFormat::Jsx),
            "json" => Some(OutputFormat::Json),
            _ => None,
        }
    }
}

/// Built-in rename strategies. The first three are concrete templates; the
/// `Custom` variant carries the user-provided template string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenameStrategy {
    /// `images/{yyyy}/{mm}/{dd}/{hash8}.{ext}` (the default).
    DateHash,
    /// `images/{yyyy}/{mm}/{dd}/{name}.{ext}` keeping the source filename.
    DateName,
    /// `images/{yyyy}/{mm}/{uuid}.{ext}` (uuid v4).
    DateUuid,
    /// Caller-supplied template.
    Custom { template: String },
}

impl Default for RenameStrategy {
    fn default() -> Self {
        RenameStrategy::DateHash
    }
}

impl RenameStrategy {
    /// Return the template string for this strategy.
    pub fn template(&self) -> &str {
        match self {
            RenameStrategy::DateHash => "images/{yyyy}/{mm}/{dd}/{hash8}.{ext}",
            RenameStrategy::DateName => "images/{yyyy}/{mm}/{dd}/{name}.{ext}",
            RenameStrategy::DateUuid => "images/{yyyy}/{mm}/{uuid}.{ext}",
            RenameStrategy::Custom { template } => template.as_str(),
        }
    }
}

/// Marker trait for anything that should be persisted as a zpic config.
pub trait ZpicConfig: Send + Sync + std::fmt::Debug {
    /// A short identifier for the config source (e.g. "zpic-toml", "picgo-json").
    fn source(&self) -> &str;
}
