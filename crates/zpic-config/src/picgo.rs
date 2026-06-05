//! PicGo configuration parsing.
//!
//! PicGo stores its config in two flavors depending on the user:
//!
//! * PicGo-Core: `~/.picgo/config.json` with `{ "picBed": ..., "picgoPlugins": ... }`
//! * PicGo GUI: per-OS data file with the same shape, located in platform
//!   application support paths.
//!
//! We do not run PicGo Node plugins; we only parse the JSON and treat the
//! `picBed` block as the source of truth for the active uploader.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use zpic_core::config::UploaderKind;
use zpic_core::error::{Result, ZpicError};

/// Raw PicGo config file. Fields we don't understand are ignored on
/// purpose — we only need the `picBed` and `picgoPlugins` blocks.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PicGoConfig {
    #[serde(rename = "picBed", default)]
    pub pic_bed: Option<PicBed>,
    #[serde(default)]
    pub uploader: BTreeMap<String, PicGoUploaderTypeConfigs>,
    #[serde(rename = "picgoPlugins", default)]
    pub picgo_plugins: Option<Value>,
}

impl PicGoConfig {
    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Return the name of the active uploader. Prefers `picBed.current`,
    /// falling back to `picBed.uploader` for older PicGo versions.
    pub fn active_uploader(&self) -> Option<&str> {
        let bed = self.pic_bed.as_ref()?;
        bed.current.as_deref().or(bed.uploader.as_deref())
    }

    /// Look up a uploader block by name. Returns `None` if `picBed` is
    /// missing or the uploader isn't represented.
    pub fn block(&self, name: &str) -> Option<&PicGoUploaderBlock> {
        let bed = self.pic_bed.as_ref()?;
        bed.uploaders.get(name)
    }

    /// Translate the active PicGo uploader name into a zpic `UploaderKind`,
    /// or `None` if it isn't supported.
    pub fn active_kind(&self) -> Option<UploaderKind> {
        let name = self.active_uploader()?;
        UploaderKind::from_alias(name)
    }

    /// Return `true` when the active uploader is only available as a PicGo
    /// plugin (i.e. not in the zpic alias table).
    pub fn is_unsupported_plugin(&self) -> bool {
        match self.active_uploader() {
            None => false,
            Some(name) => UploaderKind::from_alias(name).is_none(),
        }
    }

    /// Validate the active uploader is supported; returns the active name
    /// for the error message.
    pub fn ensure_supported(&self) -> Result<&str> {
        let name = self
            .active_uploader()
            .ok_or_else(|| ZpicError::ConfigInvalid("picBed.current is missing".into()))?;
        if self.is_unsupported_plugin() {
            return Err(ZpicError::UploaderUnsupported(name.to_string()));
        }
        Ok(name)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PicBed {
    pub current: Option<String>,
    pub uploader: Option<String>,
    pub transformer: Option<String>,
    pub proxy: Option<String>,
    #[serde(flatten)]
    pub uploaders: BTreeMap<String, PicGoUploaderBlock>,
}

/// Generic per-uploader settings as they appear in PicGo JSON files.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PicGoUploaderBlock {
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl PicGoUploaderBlock {
    pub fn string(&self, key: &str) -> Option<String> {
        self.fields
            .get(key)
            .and_then(|v| v.as_str().map(String::from))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PicGoUploaderTypeConfigs {
    #[serde(rename = "configList", default)]
    pub config_list: Vec<PicGoUploaderConfigItem>,
    #[serde(rename = "defaultId", default)]
    pub default_id: String,
}

impl PicGoUploaderTypeConfigs {
    pub fn active(&self) -> Option<&PicGoUploaderConfigItem> {
        if !self.default_id.is_empty() {
            if let Some(item) = self
                .config_list
                .iter()
                .find(|item| item.id == self.default_id)
            {
                return Some(item);
            }
        }
        self.config_list.first()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PicGoUploaderConfigItem {
    #[serde(rename = "_id", default)]
    pub id: String,
    #[serde(rename = "_configName", default)]
    pub config_name: String,
    #[serde(rename = "_createdAt", default)]
    pub created_at: i64,
    #[serde(rename = "_updatedAt", default)]
    pub updated_at: i64,
    #[serde(flatten, default)]
    pub fields: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_picgo_config() {
        let json = r#"{
            "picBed": {
                "current": "github",
                "uploader": "github",
                "github": {
                    "repo": "me/picbed",
                    "branch": "main",
                    "token": "ghp_x",
                    "path": "img/",
                    "customUrl": "https://cdn.jsdelivr.net/gh/me/picbed"
                }
            },
            "picgoPlugins": {}
        }"#;
        let cfg = PicGoConfig::from_json(json).unwrap();
        assert_eq!(cfg.active_uploader(), Some("github"));
        assert_eq!(cfg.active_kind(), Some(UploaderKind::Github));
        let gh = cfg.block("github").unwrap();
        assert_eq!(gh.string("repo").as_deref(), Some("me/picbed"));
    }

    #[test]
    fn detects_unsupported_plugin() {
        let json = r#"{
            "picBed": {
                "current": "something-weird",
                "uploader": "something-weird"
            }
        }"#;
        let cfg = PicGoConfig::from_json(json).unwrap();
        assert!(cfg.is_unsupported_plugin());
        assert!(cfg.ensure_supported().is_err());
    }
}
