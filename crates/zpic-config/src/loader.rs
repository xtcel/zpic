//! Resolves zpic and PicGo configuration files according to the documented
//! precedence rules.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use toml::Value as TomlValue;

use zpic_core::config::ZpicConfig as ZpicConfigTrait;
use zpic_core::config::{OutputFormat, RenameStrategy, UploaderKind};
use zpic_core::error::{Result, ZpicError};

use crate::paths::{candidate_picgo_paths, candidate_zpic_paths};
use crate::picgo::PicGoConfig;
use crate::zpic::ZpicConfigFile;

/// Where the active config came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// Explicit `--config` path supplied on the command line.
    Explicit(PathBuf),
    /// `ZPIC_CONFIG` environment variable.
    EnvVar(PathBuf),
    /// Project-local `.zpic/config.toml` discovered in the current directory.
    Project(PathBuf),
    /// User-global zpic config file.
    User(PathBuf),
    /// PicGo core config (no native zpic config found).
    PicgoCore(PathBuf),
    /// PicGo GUI data file (no native zpic or core PicGo config found).
    PicgoGui(PathBuf),
}

impl ConfigSource {
    pub fn path(&self) -> &Path {
        match self {
            ConfigSource::Explicit(p)
            | ConfigSource::EnvVar(p)
            | ConfigSource::Project(p)
            | ConfigSource::User(p)
            | ConfigSource::PicgoCore(p)
            | ConfigSource::PicgoGui(p) => p,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ConfigSource::Explicit(_) => "explicit",
            ConfigSource::EnvVar(_) => "env-var",
            ConfigSource::Project(_) => "project",
            ConfigSource::User(_) => "user",
            ConfigSource::PicgoCore(_) => "picgo-core",
            ConfigSource::PicgoGui(_) => "picgo-gui",
        }
    }
}

/// In-memory representation of a resolved config. Wraps the parsed source
/// so the CLI can ask "which file did this come from?" and "what's the
/// effective default uploader name?".
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub source: ConfigSource,
    /// Native zpic configuration (always populated; falls back to defaults).
    pub zpic: ZpicConfigFile,
    /// Original PicGo config when we loaded from a PicGo file. The
    /// `zpic` field is the conversion result of this when present.
    pub picgo: Option<PicGoConfig>,
}

impl LoadedConfig {
    /// Return the configured default uploader name, if any.
    pub fn default_uploader_name(&self) -> Option<&str> {
        self.zpic.default_uploader.as_deref()
    }

    /// Return the active uploader section, if any.
    pub fn active_uploader(&self) -> Option<(&str, &crate::zpic::UploaderSection)> {
        let name = self.default_uploader_name()?;
        let section = self.zpic.uploaders.get(name)?;
        Some((name, section))
    }
}

impl ZpicConfigTrait for LoadedConfig {
    fn source(&self) -> &str {
        self.source.label()
    }
}

/// Stateless loader entry point. All methods are free functions so callers
/// can compose them in tests.
pub struct ConfigLoader;

impl ConfigLoader {
    /// Resolve a config from the given explicit path (if any) plus the
    /// environment. Returns an error when nothing usable is found.
    pub fn load(explicit: Option<PathBuf>) -> Result<LoadedConfig> {
        // 1. Explicit --config overrides everything.
        if let Some(p) = explicit {
            return Self::load_explicit(&p).ok_or_else(|| {
                ZpicError::ConfigInvalid(format!("config file not found at {}", p.display()))
            });
        }
        // 2. ZPIC_CONFIG env var.
        if let Some(p) = std::env::var_os("ZPIC_CONFIG").map(PathBuf::from) {
            if p.exists() {
                return Self::load_explicit(&p).ok_or_else(|| {
                    ZpicError::ConfigInvalid(format!("invalid config at {}", p.display()))
                });
            }
        }
        // 3. Project-local .zpic/config.toml.
        for candidate in candidate_zpic_paths() {
            if candidate.exists() {
                if let Some(loaded) = Self::load_explicit(&candidate) {
                    let label = if candidate
                        .parent()
                        .map(|p| p.ends_with(".zpic"))
                        .unwrap_or(false)
                    {
                        ConfigSource::Project(candidate.clone())
                    } else {
                        ConfigSource::User(candidate.clone())
                    };
                    return Ok(LoadedConfig {
                        source: label,
                        zpic: loaded.zpic,
                        picgo: loaded.picgo,
                    });
                }
            }
        }
        // 4. PicGo fallback chain.
        for candidate in candidate_picgo_paths() {
            if !candidate.exists() {
                continue;
            }
            if let Some(loaded) = Self::load_picgo(&candidate) {
                let src = if candidate
                    .file_name()
                    .map(|n| n == "config.json")
                    .unwrap_or(false)
                {
                    ConfigSource::PicgoCore(candidate.clone())
                } else {
                    ConfigSource::PicgoGui(candidate.clone())
                };
                return Ok(LoadedConfig {
                    source: src,
                    ..loaded
                });
            }
        }
        Err(ZpicError::ConfigNotFound)
    }

    /// Read a config from an explicit path. Returns `None` if the file is
    /// missing or empty (so the loader can move on to the next candidate).
    pub fn load_explicit(path: &Path) -> Option<LoadedConfig> {
        if !path.exists() {
            return None;
        }
        // Detect PicGo JSON.
        if path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
        {
            return Self::load_picgo(path);
        }
        let raw = std::fs::read_to_string(path).ok()?;
        if raw.trim().is_empty() {
            return None;
        }
        let parsed: ZpicConfigFile = toml::from_str(&raw).ok()?;
        Some(LoadedConfig {
            source: ConfigSource::Explicit(path.to_path_buf()),
            zpic: parsed,
            picgo: None,
        })
    }

    /// Load a PicGo JSON file and convert the active uploader into a
    /// zpic config. Returns `None` if the file cannot be read.
    pub fn load_picgo(path: &Path) -> Option<LoadedConfig> {
        let raw = std::fs::read_to_string(path).ok()?;
        let picgo = PicGoConfig::from_json(&raw).ok()?;
        let zpic = convert_picgo_to_zpic(&picgo);
        let source = if path
            .file_name()
            .map(|n| n == "config.json")
            .unwrap_or(false)
        {
            ConfigSource::PicgoCore(path.to_path_buf())
        } else {
            ConfigSource::PicgoGui(path.to_path_buf())
        };
        Some(LoadedConfig {
            source,
            zpic,
            picgo: Some(picgo),
        })
    }

    /// Import a PicGo config into a new native zpic TOML file. The source
    /// PicGo file is not modified.
    pub fn import_picgo(source: &Path, dest: &Path) -> Result<ZpicConfigFile> {
        let loaded = Self::load_picgo(source)
            .ok_or_else(|| ZpicError::ConfigInvalid(format!("cannot read {}", source.display())))?;
        if let Err(e) = loaded.picgo.as_ref().unwrap().ensure_supported() {
            return Err(e);
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized =
            toml::to_string_pretty(&loaded.zpic).map_err(|e| ZpicError::Internal(e.to_string()))?;
        std::fs::write(dest, serialized)?;
        Ok(loaded.zpic)
    }
}

/// Convert a PicGo config into a `ZpicConfigFile`. Only the active uploader
/// is converted; the result is suitable for `import-picgo` and is also
/// what the loader uses when reading directly from a PicGo file.
pub fn convert_picgo_to_zpic(picgo: &PicGoConfig) -> ZpicConfigFile {
    let mut out = ZpicConfigFile::default();
    let active = match picgo.active_uploader() {
        Some(name) => name,
        None => return out,
    };
    let kind = match picgo.active_kind() {
        Some(k) => k,
        None => return out,
    };
    let block = match picgo.block(active) {
        Some(b) => b,
        None => return out,
    };
    let mut fields: BTreeMap<String, TomlValue> = BTreeMap::new();
    for (k, v) in &block.fields {
        // Skip the active uploader name to avoid `[uploaders.github].github`.
        if k == "current" || k == "uploader" {
            continue;
        }
        if let Some(toml_v) = json_to_toml(v) {
            fields.insert(k.clone(), toml_v);
        }
    }
    // Normalize common PicGo fields.
    if let Some(custom) = fields.remove("customUrl") {
        fields.insert("public_base_url".to_string(), custom);
    }
    if let Some(path) = fields.remove("path") {
        // PicGo's `path` is a prefix; treat it as path_prefix.
        fields.insert("path_prefix".to_string(), path);
    }
    // For S3-style backends PicGo uses `area` etc.; preserve them.
    if kind == UploaderKind::Github {
        if let Some(repo) = fields.get("repo").cloned() {
            // The customUrl for jsdelivr needs the branch. We don't know it
            // here, so the user can set public_base_url later.
            tracing::debug!(?repo, "imported PicGo github repo");
        }
    }
    if kind == UploaderKind::S3 {
        // PicGo `s3` uploader has `bucket`, `endpoint`, `region`, etc.
    }
    out.default_uploader = Some(active.to_string());
    out.uploaders.insert(
        active.to_string(),
        crate::zpic::UploaderSection {
            kind,
            alias: Some(active.to_string()),
            fields,
        },
    );
    out.default_format = OutputFormat::Markdown;
    out.rename.strategy = RenameStrategy::DateHash;
    out
}

/// Best-effort JSON -> TOML conversion used by the PicGo importer.
fn json_to_toml(v: &serde_json::Value) -> Option<TomlValue> {
    match v {
        serde_json::Value::String(s) => Some(TomlValue::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(TomlValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Some(TomlValue::Float(f))
            } else {
                None
            }
        }
        serde_json::Value::Bool(b) => Some(TomlValue::Boolean(*b)),
        serde_json::Value::Null => None,
        serde_json::Value::Array(arr) => {
            let mut items = Vec::new();
            for item in arr {
                items.push(json_to_toml(item)?);
            }
            Some(TomlValue::Array(items))
        }
        serde_json::Value::Object(obj) => {
            let mut table = toml::map::Map::new();
            for (k, v) in obj {
                table.insert(k.clone(), json_to_toml(v)?);
            }
            Some(TomlValue::Table(table))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn explicit_overrides_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("custom.toml");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(
            f,
            r#"
            default_uploader = "l"
            [uploaders.l]
            type = "local"
            target_dir = "./x"
            public_base_url = "/x"
            "#
        )
        .unwrap();
        let loaded = ConfigLoader::load_explicit(&p).unwrap();
        assert_eq!(loaded.default_uploader_name(), Some("l"));
    }

    #[test]
    fn picgo_to_zpic_conversion_preserves_fields() {
        let json = r#"{
            "picBed": {
                "current": "github",
                "github": {
                    "repo": "me/picbed",
                    "branch": "main",
                    "token": "ghp_x",
                    "path": "img/",
                    "customUrl": "https://cdn.jsdelivr.net/gh/me/picbed"
                }
            }
        }"#;
        let picgo = PicGoConfig::from_json(json).unwrap();
        let zpic = convert_picgo_to_zpic(&picgo);
        let gh = zpic.uploaders.get("github").unwrap();
        assert_eq!(gh.string_field("repo"), "me/picbed");
        assert_eq!(
            gh.string_field("public_base_url"),
            "https://cdn.jsdelivr.net/gh/me/picbed"
        );
        assert_eq!(gh.string_field("path_prefix"), "img/");
    }
}
