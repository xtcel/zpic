//! Resolves zpic and PicGo configuration files according to the documented
//! precedence rules.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use toml::Value as TomlValue;

use zpic_core::config::ZpicConfig as ZpicConfigTrait;
use zpic_core::config::{OutputFormat, RenameStrategy, UploaderKind};
use zpic_core::error::{Result, ZpicError};

use crate::paths::{candidate_picgo_paths, candidate_zpic_paths};
use crate::picgo::{PicGoConfig, PicGoUploaderConfigItem, PicGoUploaderTypeConfigs};
use crate::zpic::{
    migrate_legacy, new_id, now_ms, warn_legacy_migration_once, PicBedSection,
    PicBedUploaderMirror, UploaderConfigItem, UploaderTypeConfigs, ZpicConfigFile,
};

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
/// so the CLI can ask "which file did this come from?" and resolve the
/// active uploader consistently.
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
    pub fn active_uploader_type(&self) -> Option<&str> {
        self.zpic.active_uploader_type()
    }

    pub fn active_uploader_config_name(&self) -> Option<&str> {
        let uploader_type = self.active_uploader_type()?;
        self.zpic
            .uploader
            .get(uploader_type)?
            .active()
            .map(|item| item.config_name.as_str())
    }

    /// Return the active built-in uploader section, if any.
    pub fn active_uploader(&self) -> Option<(String, crate::zpic::UploaderSection)> {
        let uploader_type = self.active_uploader_type()?.to_string();
        let section = self
            .zpic
            .uploader
            .get(&uploader_type)?
            .active()?
            .to_uploader_section_for_type(&uploader_type)
            .ok()?;
        Some((uploader_type, section))
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
        let mut parsed: ZpicConfigFile = toml::from_str(&raw).ok()?;
        if migrate_legacy(&mut parsed) {
            warn_legacy_migration_once();
        }
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
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized =
            toml::to_string_pretty(&loaded.zpic).map_err(|e| ZpicError::Internal(e.to_string()))?;
        std::fs::write(dest, serialized)?;
        Ok(loaded.zpic)
    }
}

/// Convert a PicGo config into a `ZpicConfigFile`. Prefers PicGo's
/// multi-config `uploader.<type>` source of truth and falls back to the
/// active `picBed.<type>` mirror for older configs.
pub fn convert_picgo_to_zpic(picgo: &PicGoConfig) -> ZpicConfigFile {
    let mut out = ZpicConfigFile::default();
    out.default_format = OutputFormat::Markdown;
    out.rename.strategy = RenameStrategy::DateHash;
    if let Some(plugins) = picgo
        .picgo_plugins
        .as_ref()
        .and_then(|value| value.as_object())
    {
        for (name, enabled) in plugins {
            if let Some(enabled) = enabled.as_bool() {
                out.picgo_plugins.insert(name.clone(), enabled);
            }
        }
    }

    if let Some(pic_bed) = &picgo.pic_bed {
        out.pic_bed = PicBedSection {
            current: pic_bed.current.clone(),
            uploader: pic_bed.uploader.clone(),
            transformer: pic_bed.transformer.clone(),
            proxy: pic_bed.proxy.clone(),
            uploader_mirrors: BTreeMap::new(),
        };
    }

    for (uploader_type, configs) in &picgo.uploader {
        let converted = convert_picgo_type_configs(configs);
        if !converted.config_list.is_empty() {
            out.uploader.insert(uploader_type.clone(), converted);
        }
    }

    if out.uploader.is_empty() {
        let active = match picgo.active_uploader() {
            Some(name) => name,
            None => return out,
        };
        let block = match picgo.block(active) {
            Some(block) => block,
            None => return out,
        };
        let fields = normalize_picgo_fields(json_object_to_toml_map(&block.fields));
        let now = now_ms();
        let item = UploaderConfigItem {
            id: new_id(),
            config_name: default_import_name(active, picgo.active_kind()),
            created_at: now,
            updated_at: now,
            fields,
        };
        out.uploader.insert(
            active.to_string(),
            UploaderTypeConfigs {
                default_id: item.id.clone(),
                config_list: vec![item],
            },
        );
    }

    for (uploader_type, configs) in &out.uploader {
        if let Some(active) = configs.active() {
            out.pic_bed.uploader_mirrors.insert(
                uploader_type.clone(),
                PicBedUploaderMirror::from_item(active),
            );
        }
    }

    if let Some(active_type) = out
        .pic_bed
        .current
        .clone()
        .or_else(|| out.pic_bed.uploader.clone())
        .or_else(|| out.uploader.keys().next().cloned())
    {
        out.pic_bed.current = Some(active_type.clone());
        out.pic_bed.uploader = Some(active_type);
    }
    out
}

fn convert_picgo_type_configs(configs: &PicGoUploaderTypeConfigs) -> UploaderTypeConfigs {
    let mut out = UploaderTypeConfigs {
        default_id: configs.default_id.clone(),
        config_list: configs
            .config_list
            .iter()
            .map(convert_picgo_config_item)
            .collect(),
    };
    if out.default_id.is_empty() {
        if let Some(first) = out.config_list.first() {
            out.default_id = first.id.clone();
        }
    }
    out
}

fn convert_picgo_config_item(item: &PicGoUploaderConfigItem) -> UploaderConfigItem {
    let now = now_ms();
    UploaderConfigItem {
        id: if item.id.is_empty() {
            new_id()
        } else {
            item.id.clone()
        },
        config_name: if item.config_name.trim().is_empty() {
            "Default".to_string()
        } else {
            item.config_name.clone()
        },
        created_at: if item.created_at == 0 {
            now
        } else {
            item.created_at
        },
        updated_at: if item.updated_at == 0 {
            now
        } else {
            item.updated_at
        },
        fields: normalize_picgo_fields(json_object_to_toml_map(&item.fields)),
    }
}

fn default_import_name(active: &str, kind: Option<UploaderKind>) -> String {
    match kind {
        Some(UploaderKind::Local | UploaderKind::Github) => "Default".to_string(),
        Some(UploaderKind::S3) => {
            if active.eq_ignore_ascii_case("s3") {
                "Default".to_string()
            } else {
                active.to_string()
            }
        }
        None => "Default".to_string(),
    }
}

fn json_object_to_toml_map(
    values: &BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, TomlValue> {
    let mut out = BTreeMap::new();
    for (key, value) in values {
        if key == "current" || key == "uploader" || key == "transformer" || key == "proxy" {
            continue;
        }
        if let Some(value) = json_to_toml(value) {
            out.insert(key.clone(), value);
        }
    }
    out
}

fn normalize_picgo_fields(mut fields: BTreeMap<String, TomlValue>) -> BTreeMap<String, TomlValue> {
    if let Some(custom) = fields.remove("customUrl") {
        fields.insert("public_base_url".to_string(), custom);
    }
    if let Some(path) = fields.remove("path") {
        fields.insert("path_prefix".to_string(), path);
    }
    fields.remove("_id");
    fields.remove("_configName");
    fields.remove("_createdAt");
    fields.remove("_updatedAt");
    fields
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
        assert_eq!(loaded.active_uploader_type(), Some("local"));
        assert_eq!(loaded.active_uploader_config_name(), Some("l"));
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
        let gh = zpic.uploader.get("github").unwrap().active().unwrap();
        assert_eq!(gh.field("repo").as_deref(), Some("me/picbed"));
        assert_eq!(
            gh.field("public_base_url").as_deref(),
            Some("https://cdn.jsdelivr.net/gh/me/picbed")
        );
        assert_eq!(gh.field("path_prefix").as_deref(), Some("img/"));
        assert_eq!(zpic.active_uploader_type(), Some("github"));
        assert!(zpic.pic_bed.uploader_mirrors.contains_key("github"));
    }

    #[test]
    fn picgo_multi_config_is_preserved() {
        let json = r#"{
            "picBed": {
                "current": "github",
                "uploader": "github",
                "github": {
                    "repo": "fallback/repo"
                }
            },
            "uploader": {
                "github": {
                    "defaultId": "id-work",
                    "configList": [
                        {
                            "_id": "id-personal",
                            "_configName": "Personal",
                            "_createdAt": 1700000000000,
                            "_updatedAt": 1700000000000,
                            "repo": "me/personal",
                            "token": "ghp_a"
                        },
                        {
                            "_id": "id-work",
                            "_configName": "Work",
                            "_createdAt": 1700000001000,
                            "_updatedAt": 1700000002000,
                            "repo": "me/work",
                            "token": "ghp_b"
                        }
                    ]
                }
            }
        }"#;
        let picgo = PicGoConfig::from_json(json).unwrap();
        let zpic = convert_picgo_to_zpic(&picgo);
        let gh = zpic.uploader.get("github").unwrap();
        assert_eq!(gh.config_list.len(), 2);
        assert_eq!(gh.default_id, "id-work");
        assert_eq!(gh.active().unwrap().config_name, "Work");
        assert_eq!(
            zpic.pic_bed
                .uploader_mirrors
                .get("github")
                .and_then(|mirror| mirror.fields.get("repo"))
                .and_then(|value| value.as_str()),
            Some("me/work")
        );
    }
}
