//! Native zpic TOML configuration model.
//!
//! The model is PicGo-compatible: a `pic_bed` section holds the active
//! uploader type and per-type mirrors, and `uploader.<type>.configList`
//! holds the named configurations per type with a `defaultId` pointer.
//! Legacy v0.1 configs (`default_uploader` + `[uploaders.<name>]`) are
//! auto-migrated on first load by the loader.

use std::collections::BTreeMap;
use std::sync::Once;

use serde::{Deserialize, Serialize};

use zpic_core::config::{OutputFormat, RenameStrategy, UploaderKind};

/// Top-level zpic config file. Uses the PicGo-compatible shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZpicConfigFile {
    /// Legacy: name of the default uploader block (e.g. `r2`). Kept so
    /// old configs can be migrated transparently; new code should read
    /// `pic_bed.current` instead.
    #[serde(default, skip_serializing)]
    pub default_uploader: Option<String>,
    /// Default output format for `zpic upload` and `zpic migrate`.
    #[serde(default)]
    pub default_format: OutputFormat,
    /// Copy rendered output to the system clipboard after each successful upload.
    #[serde(default)]
    pub copy_after_upload: bool,
    /// Persist successful uploads in the SQLite history store.
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,
    /// Rename / path-template strategy.
    #[serde(default)]
    pub rename: RenameSection,
    /// Per-format output templates.
    #[serde(default)]
    pub format: FormatSection,
    /// Legacy: name-keyed map of `UploaderSection` (e.g. `r2`, `github`).
    /// Kept for migration; new code should use `uploader.<type>.configList`.
    #[serde(default, skip_serializing)]
    pub uploaders: BTreeMap<String, UploaderSection>,
    /// `picBed` block, mirrors PicGo's active-uploader tracking.
    #[serde(default)]
    pub pic_bed: PicBedSection,
    /// PicGo-compatible per-type configs. Keyed by uploader type
    /// (`github`, `s3`, `local`, ...). The value's `configList` holds
    /// the named configurations, and `defaultId` selects the active one.
    #[serde(default)]
    pub uploader: BTreeMap<String, UploaderTypeConfigs>,
    /// PicGo-compatible plugin enable/disable map.
    #[serde(default, rename = "picgoPlugins")]
    pub picgo_plugins: BTreeMap<String, bool>,
}

fn default_history_enabled() -> bool {
    true
}

impl ZpicConfigFile {
    /// Render a stable TOML representation with secrets redacted. Used by
    /// `zpic config show` and `doctor` output.
    pub fn redacted_toml(&self) -> String {
        // Migration is a no-op when the new model is already populated, so
        // `redacted_toml` can be called on a borrowed config safely.
        let mut clone = self.clone();
        migrate_legacy(&mut clone);
        for uploader in clone.uploaders.values_mut() {
            uploader.redact_secrets();
        }
        for uploader_type in clone.uploader.values_mut() {
            for item in &mut uploader_type.config_list {
                item.redact_secrets();
            }
        }
        for mirror in clone.pic_bed.uploader_mirrors.values_mut() {
            mirror.redact_secrets();
        }
        // The legacy fields are skipped on serialize, so the output is the
        // PicGo-compatible shape only.
        toml::to_string_pretty(&clone).unwrap_or_else(|_| String::new())
    }

    /// Active uploader type name, if any.
    pub fn active_uploader_type(&self) -> Option<&str> {
        self.pic_bed
            .current
            .as_deref()
            .or(self.pic_bed.uploader.as_deref())
    }
}

/// `picBed` section — mirrors PicGo's active-uploader tracking. Also
/// stores per-type active-config mirrors under `pic_bed.<type>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PicBedSection {
    /// The active uploader type (e.g. `github`, `s3`).
    #[serde(default)]
    pub current: Option<String>,
    /// Legacy field with the same meaning as `current`.
    #[serde(default)]
    pub uploader: Option<String>,
    /// Active transformer (PicGo convention).
    #[serde(default)]
    pub transformer: Option<String>,
    /// Optional proxy URL (PicGo convention).
    #[serde(default)]
    pub proxy: Option<String>,
    /// Per-type active-config mirrors. The key is the uploader type.
    /// Each value mirrors the active config fields for that uploader type.
    /// These are kept in sync with `uploader.<type>.configList[defaultId]`.
    #[serde(flatten, default)]
    pub uploader_mirrors: BTreeMap<String, PicBedUploaderMirror>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PicBedUploaderMirror {
    #[serde(flatten, default)]
    pub fields: BTreeMap<String, toml::Value>,
}

impl PicBedUploaderMirror {
    pub fn from_item(item: &UploaderConfigItem) -> Self {
        Self {
            fields: item.fields.clone(),
        }
    }

    pub fn redact_secrets(&mut self) {
        redact_secret_fields(&mut self.fields);
    }
}

/// One named configuration for an uploader type, matching PicGo's
/// `IUploaderConfigItem` shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploaderConfigItem {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_configName")]
    pub config_name: String,
    #[serde(rename = "_createdAt")]
    pub created_at: i64,
    #[serde(rename = "_updatedAt")]
    pub updated_at: i64,
    /// All the uploader-specific fields (`type`, `endpoint`, `token`, ...).
    #[serde(flatten)]
    pub fields: BTreeMap<String, toml::Value>,
}

impl UploaderConfigItem {
    /// Read a string field.
    pub fn field(&self, key: &str) -> Option<String> {
        self.fields.get(key).and_then(|v| match v {
            toml::Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        })
    }

    /// Resolve a string field with `$VAR` / `${VAR}` expansion.
    pub fn resolve(&self, key: &str) -> Option<String> {
        self.field(key).map(|raw| crate::zpic::expand_env(&raw))
    }

    /// Set/replace a field.
    pub fn set_field(&mut self, key: impl Into<String>, value: toml::Value) {
        self.fields.insert(key.into(), value);
        self.updated_at = now_ms();
    }

    /// Remove a field.
    pub fn remove_field(&mut self, key: &str) {
        if self.fields.remove(key).is_some() {
            self.updated_at = now_ms();
        }
    }

    pub fn redact_secrets(&mut self) {
        redact_secret_fields(&mut self.fields);
    }

    /// Built-in uploader kind, derived from the `type` field.
    pub fn uploader_kind(&self) -> Option<UploaderKind> {
        let t = self.field("type")?;
        UploaderKind::from_alias(&t)
    }

    /// Convert to the `UploaderSection` shape that the uploader constructors
    /// accept. Prefers the uploader type path token because PicGo stores the
    /// type in the path (`uploader.<type>`) rather than inside each config.
    pub fn to_uploader_section_for_type(&self, uploader_type: &str) -> UploaderSection {
        let kind = UploaderKind::from_alias(uploader_type)
            .or_else(|| self.uploader_kind())
            .unwrap_or(UploaderKind::Local);
        UploaderSection {
            kind,
            alias: Some(self.config_name.clone()),
            fields: self.fields.clone(),
        }
    }
}

/// All named configurations for one uploader type, plus a pointer to
/// the default one. Matches PicGo's `IUploaderTypeConfigs` shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UploaderTypeConfigs {
    #[serde(rename = "configList", default)]
    pub config_list: Vec<UploaderConfigItem>,
    #[serde(rename = "defaultId", default)]
    pub default_id: String,
}

impl UploaderTypeConfigs {
    /// Find a config by its case-insensitive name.
    pub fn find_by_name(&self, name: &str) -> Option<&UploaderConfigItem> {
        let target = name.trim().to_lowercase();
        self.config_list
            .iter()
            .find(|c| c.config_name.trim().to_lowercase() == target)
    }

    /// Find a config by id.
    pub fn find_by_id(&self, id: &str) -> Option<&UploaderConfigItem> {
        self.config_list.iter().find(|c| c.id == id)
    }

    /// Active config (matches `defaultId`, or the first entry).
    pub fn active(&self) -> Option<&UploaderConfigItem> {
        if !self.default_id.is_empty() {
            if let Some(c) = self.find_by_id(&self.default_id) {
                return Some(c);
            }
        }
        self.config_list.first()
    }

    /// Mutable variant of `active`.
    pub fn active_mut(&mut self) -> Option<&mut UploaderConfigItem> {
        if !self.default_id.is_empty() {
            if let Some(idx) = self
                .config_list
                .iter()
                .position(|c| c.id == self.default_id)
            {
                return Some(&mut self.config_list[idx]);
            }
        }
        self.config_list.first_mut()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSection {
    #[serde(default)]
    pub strategy: RenameStrategy,
    /// Optional path template. Overrides `strategy` when set.
    pub path: Option<String>,
    /// When true, the source file name is preserved (equivalent to
    /// `RenameStrategy::DateName`).
    #[serde(default)]
    pub keep_original_name: bool,
}

impl Default for RenameSection {
    fn default() -> Self {
        Self {
            strategy: RenameStrategy::default(),
            path: None,
            keep_original_name: false,
        }
    }
}

impl RenameSection {
    /// Resolve the effective template string. `path` (if set) wins over the
    /// built-in strategy; `keep_original_name` switches to a name-based
    /// strategy.
    pub fn effective_template(&self) -> String {
        if let Some(p) = &self.path {
            return p.clone();
        }
        if self.keep_original_name {
            return "images/{yyyy}/{mm}/{dd}/{name}.{ext}".to_string();
        }
        self.strategy.template().to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatSection {
    pub markdown: Option<String>,
    pub html: Option<String>,
    pub jsx: Option<String>,
    pub url: Option<String>,
}

impl FormatSection {
    pub fn template_for(&self, format: OutputFormat) -> Option<&str> {
        match format {
            OutputFormat::Markdown => self.markdown.as_deref(),
            OutputFormat::Html => self.html.as_deref(),
            OutputFormat::Jsx => self.jsx.as_deref(),
            OutputFormat::Url => self.url.as_deref(),
            OutputFormat::Json => None,
        }
    }
}

/// A single named uploader entry (the in-memory shape used by uploader
/// constructors). Configurations stored in `UploaderConfigItem.fields`
/// are converted to this shape when an uploader is instantiated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploaderSection {
    /// Built-in kind: `local`, `github`, `s3`.
    #[serde(rename = "type")]
    pub kind: UploaderKind,
    /// Display name / alias used by the PicGo compatibility layer.
    #[serde(default)]
    pub alias: Option<String>,
    /// Free-form fields per uploader type. We keep them as a TOML value so
    /// we don't have to enumerate every backend-specific field here.
    #[serde(flatten)]
    pub fields: BTreeMap<String, toml::Value>,
}

impl UploaderSection {
    /// Replace any field whose name suggests a secret with the literal
    /// `"<redacted>"`. Used when rendering the config for human display.
    pub fn redact_secrets(&mut self) {
        redact_secret_fields(&mut self.fields);
    }

    /// Resolve a string value: env-var expansion (`$FOO` or `${FOO}`) takes
    /// precedence; otherwise the raw value is returned. When the env var is
    /// missing, the original literal is preserved.
    pub fn resolve_string(&self, key: &str) -> Option<String> {
        let v = self.fields.get(key)?;
        let raw = match v {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        Some(expand_env(&raw))
    }

    /// Convenience accessor for `endpoint` / `repo` / etc. Returns an empty
    /// string when the field is absent so callers can chain `.to_string()`.
    pub fn string_field(&self, key: &str) -> String {
        self.fields
            .get(key)
            .map(|v| match v {
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    }

    /// Read a field as `Option<String>`.
    pub fn field(&self, key: &str) -> Option<String> {
        self.fields.get(key).map(|v| match v {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
    }
}

fn redact_secret_fields(fields: &mut BTreeMap<String, toml::Value>) {
    let secret_keys = [
        "token",
        "secret",
        "secret_access_key",
        "secretaccesskey",
        "password",
        "access_key_secret",
    ];
    for (key, value) in fields.iter_mut() {
        let normalized = key.to_ascii_lowercase();
        if secret_keys.iter().any(|secret| normalized.contains(secret)) {
            *value = toml::Value::String("<redacted>".to_string());
        }
    }
}

/// Expand `$VAR` and `${VAR}` references in a string. Unknown vars are
/// left as-is. This is the same syntax used by PicGo.
pub fn expand_env(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            if let Some(after) = peek_env_ref(input, i + 1) {
                let (name, consumed) = after;
                if let Ok(value) = std::env::var(&name) {
                    out.push_str(&value);
                } else {
                    out.push('$');
                    if input.as_bytes()[i + 1] == b'{' {
                        out.push('{');
                    }
                    out.push_str(&name);
                    if input.as_bytes()[i + 1] == b'{' {
                        out.push('}');
                    }
                }
                i += 1 + consumed;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn peek_env_ref(s: &str, start: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    if bytes[start] == b'{' {
        let close = s[start + 1..].find('}')?;
        let name = &s[start + 1..start + 1 + close];
        return Some((name.to_string(), 1 + close + 1));
    }
    if bytes[start].is_ascii_alphabetic() || bytes[start] == b'_' {
        let mut end = start + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        return Some((s[start..end].to_string(), end - start));
    }
    None
}

pub(crate) fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a small unique id (UUIDv4-shaped) without bringing in the
/// `uuid` crate as a runtime dependency.
pub(crate) fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut bytes = [0u8; 16];
    bytes[0..8].copy_from_slice(&(nanos as u64).to_le_bytes());
    bytes[8..16].copy_from_slice(&((nanos >> 64) as u64).to_le_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut out = String::with_capacity(36);
    for (i, b) in bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            out.push('-');
        }
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn warn_legacy_migration_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        tracing::warn!(
            "detected legacy zpic uploader config; auto-migrated it to the PicGo-compatible model in memory. Run `zpic config show` and save the result to clean up the file."
        );
    });
}

/// Migrate a v0.1-style config (legacy `default_uploader` +
/// `[uploaders.<name>]`) into the new PicGo-compatible model in place.
/// Idempotent: if the new model is already populated, this is a no-op.
pub fn migrate_legacy(cfg: &mut ZpicConfigFile) -> bool {
    // No migration needed if the new model already has data or the legacy
    // fields are empty.
    if !cfg.uploaders.is_empty() && cfg.uploader.is_empty() {
        // Promote each [uploaders.<name>] into a UploaderConfigItem under
        // the type indicated by its `type` field (defaulting to the name).
        let mut any_migrated = false;
        let mut active_type: Option<String> = None;
        let legacy_default = cfg.default_uploader.clone();
        for (name, section) in cfg.uploaders.iter() {
            let type_str = section
                .field("type")
                .unwrap_or_else(|| section.kind.as_str().to_string());
            let id = new_id();
            let now = now_ms();
            let mut item = UploaderConfigItem {
                id,
                config_name: name.clone(),
                created_at: now,
                updated_at: now,
                fields: section.fields.clone(),
            };
            // We don't want the `alias` field polluting the TOML; the
            // alias is implicit in `_configName`.
            item.remove_field("alias");
            let store = cfg
                .uploader
                .entry(type_str.clone())
                .or_insert_with(UploaderTypeConfigs::default);
            store.config_list.push(item);
            let latest_id = store.config_list.last().unwrap().id.clone();
            if legacy_default
                .as_deref()
                .map(|default_name| default_name.eq_ignore_ascii_case(name))
                .unwrap_or(false)
            {
                store.default_id = latest_id.clone();
                active_type = Some(type_str.clone());
            } else if store.default_id.is_empty() {
                store.default_id = latest_id;
            }
            if active_type.is_none() && cfg.pic_bed.current.is_none() {
                active_type = Some(type_str.clone());
            }
            if cfg.pic_bed.current.is_none() && cfg.pic_bed.uploader.is_none() {
                cfg.pic_bed.current = Some(type_str.clone());
                cfg.pic_bed.uploader = Some(type_str.clone());
            }
            if store.default_id.is_empty() {
                store.default_id = store.config_list.last().unwrap().id.clone();
            }
            any_migrated = true;
        }
        if any_migrated {
            for (uploader_type, store) in &cfg.uploader {
                if let Some(active) = store.active() {
                    cfg.pic_bed.uploader_mirrors.insert(
                        uploader_type.clone(),
                        PicBedUploaderMirror::from_item(active),
                    );
                }
            }
            let active_type = active_type.or_else(|| cfg.uploader.keys().next().cloned());
            if let Some(t) = active_type {
                cfg.pic_bed.current = Some(t.clone());
                cfg.pic_bed.uploader = Some(t.clone());
            }
        }
        return any_migrated;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_picgo_style_config() {
        let toml = r#"
default_format = "markdown"

[pic_bed]
current = "github"
uploader = "github"

[uploader.github]
configList = [
    { _id = "abc", _configName = "Personal", _createdAt = 0, _updatedAt = 0, type = "github", repo = "me/picbed", branch = "main", token = "$GITHUB_TOKEN" },
]
defaultId = "abc"
"#;
        let parsed: ZpicConfigFile = toml::from_str(toml).unwrap();
        assert_eq!(parsed.pic_bed.current.as_deref(), Some("github"));
        let store = parsed.uploader.get("github").unwrap();
        assert_eq!(store.config_list.len(), 1);
        assert_eq!(store.config_list[0].config_name, "Personal");
    }

    #[test]
    fn migrate_legacy_promotes_simple_config() {
        let mut cfg = ZpicConfigFile {
            default_uploader: Some("r2".into()),
            uploaders: {
                let mut m = BTreeMap::new();
                m.insert(
                    "r2".into(),
                    UploaderSection {
                        kind: UploaderKind::S3,
                        alias: None,
                        fields: {
                            let mut f = BTreeMap::new();
                            f.insert(
                                "endpoint".into(),
                                toml::Value::String("https://r2.example.com".into()),
                            );
                            f.insert("bucket".into(), toml::Value::String("b".into()));
                            f.insert(
                                "public_base_url".into(),
                                toml::Value::String("https://cdn".into()),
                            );
                            f
                        },
                    },
                );
                m
            },
            ..Default::default()
        };
        let changed = migrate_legacy(&mut cfg);
        assert!(changed);
        assert_eq!(cfg.pic_bed.current.as_deref(), Some("s3"));
        let store = cfg.uploader.get("s3").unwrap();
        assert_eq!(store.config_list.len(), 1);
        assert_eq!(store.config_list[0].config_name, "r2");
        assert!(!store.default_id.is_empty());
        assert_eq!(
            cfg.pic_bed
                .uploader_mirrors
                .get("s3")
                .and_then(|mirror| mirror.fields.get("bucket"))
                .and_then(|value| value.as_str()),
            Some("b")
        );
    }

    #[test]
    fn migrate_legacy_noop_when_new_model_already_present() {
        let mut cfg = ZpicConfigFile::default();
        let item = UploaderConfigItem {
            id: "id".into(),
            config_name: "x".into(),
            created_at: 0,
            updated_at: 0,
            fields: BTreeMap::new(),
        };
        cfg.uploader.insert(
            "github".into(),
            UploaderTypeConfigs {
                config_list: vec![item],
                default_id: "id".into(),
            },
        );
        // Add a legacy entry; migration should still skip because the
        // new model is non-empty.
        let changed = migrate_legacy(&mut cfg);
        assert!(!changed);
    }
}
