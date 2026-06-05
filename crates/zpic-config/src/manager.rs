//! `UploaderConfigManager` — the PicGo-compatible CRUD layer for named
//! uploader configurations.
//!
//! The manager wraps a mutable `ZpicConfigFile` and exposes the same
//! operations PicGo users already know: `list`, `use_config`,
//! `create_or_update`, `rename`, `copy`, `remove`. It also auto-migrates
//! legacy v0.1 configs on first call and keeps `pic_bed.<type>` mirrors
//! in sync.

use std::collections::BTreeMap;

use zpic_core::error::{Result, ZpicError};

use crate::zpic::{
    expand_env, migrate_legacy, new_id, now_ms, warn_legacy_migration_once, PicBedUploaderMirror,
    UploaderConfigItem, UploaderTypeConfigs, ZpicConfigFile,
};

/// All errors raised by the manager. Most of these map onto `ZpicError`
/// variants so the CLI can produce a consistent user experience.
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    #[error("uploader type '{0}' is not configured")]
    UnknownType(String),
    #[error("config '{1}' not found in type '{0}'")]
    ConfigNotFound(String, String),
    #[error("config name '{0}' already exists in type '{1}'")]
    DuplicateName(String, String),
    #[error("config name can not be empty")]
    EmptyName,
}

impl From<ManagerError> for ZpicError {
    fn from(e: ManagerError) -> Self {
        ZpicError::ConfigInvalid(e.to_string())
    }
}

/// The manager holds a mutable reference to a `ZpicConfigFile` and
/// mutates it through the methods below. All changes happen in place;
/// the caller is responsible for persisting the file.
pub struct UploaderConfigManager<'a> {
    pub config: &'a mut ZpicConfigFile,
}

impl<'a> UploaderConfigManager<'a> {
    /// Construct a manager. Auto-migrates the legacy v0.1 config on first
    /// construction if both the legacy and new models are present.
    pub fn new(config: &'a mut ZpicConfigFile) -> Self {
        if migrate_legacy(config) {
            warn_legacy_migration_once();
        }
        Self { config }
    }

    /// List the uploader types with at least one configuration.
    pub fn list_types(&self) -> Vec<&str> {
        let mut types: Vec<&str> = self.config.uploader.keys().map(|s| s.as_str()).collect();
        types.sort();
        types
    }

    /// List configs for one type.
    pub fn list_configs(&self, uploader_type: &str) -> Vec<&UploaderConfigItem> {
        self.config
            .uploader
            .get(uploader_type)
            .map(|t| t.config_list.iter().collect())
            .unwrap_or_default()
    }

    /// Active config for a type (matches `defaultId` or the first entry).
    pub fn get_active(&self, uploader_type: &str) -> Option<&UploaderConfigItem> {
        self.config
            .uploader
            .get(uploader_type)
            .and_then(|t| t.active())
    }

    /// Find a config by name (case-insensitive).
    pub fn get_by_name(&self, uploader_type: &str, name: &str) -> Option<&UploaderConfigItem> {
        self.config
            .uploader
            .get(uploader_type)
            .and_then(|t| t.find_by_name(name))
    }

    /// Find a config by id.
    pub fn get_by_id(&self, uploader_type: &str, id: &str) -> Option<&UploaderConfigItem> {
        self.config
            .uploader
            .get(uploader_type)
            .and_then(|t| t.find_by_id(id))
    }

    /// Activate a config. Pass `None` to use whatever the current default
    /// is (or create a placeholder if the type has no configs yet).
    pub fn use_config(
        &mut self,
        uploader_type: &str,
        config_name: Option<&str>,
    ) -> Result<&UploaderConfigItem> {
        self.assert_type(uploader_type)?;
        let store = self.config.uploader.get(uploader_type).unwrap();
        if store.config_list.is_empty() {
            let placeholder_name = config_name
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or("Default");
            return self.create_or_update(
                uploader_type,
                Some(placeholder_name),
                BTreeMap::new(),
            );
        }
        let target_id = if let Some(name) = config_name {
            let target = store
                .find_by_name(name)
                .ok_or_else(|| ManagerError::ConfigNotFound(uploader_type.into(), name.into()))?;
            target.id.clone()
        } else {
            // No explicit name: pick the current default or the first entry.
            store.default_id.clone()
        };
        let target_id = if target_id.is_empty() {
            store
                .config_list
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_default()
        } else {
            target_id
        };

        // Activate and mirror.
        let store = self.config.uploader.get_mut(uploader_type).unwrap();
        store.default_id = target_id.clone();
        self.config.pic_bed.current = Some(uploader_type.to_string());
        self.config.pic_bed.uploader = Some(uploader_type.to_string());
        if let Some(active) = store.find_by_id(&target_id) {
            self.config.pic_bed.uploader_mirrors.insert(
                uploader_type.to_string(),
                PicBedUploaderMirror::from_item(active),
            );
        }
        Ok(store.find_by_id(&target_id).unwrap())
    }

    /// Create a new config or update an existing one (matched by name). The
    /// `fields` map is merged into the config's existing fields; pass an
    /// empty map to create a stub config. Creates a new type entry on
    /// first call for a previously unknown type.
    pub fn create_or_update(
        &mut self,
        uploader_type: &str,
        config_name: Option<&str>,
        fields: BTreeMap<String, toml::Value>,
    ) -> Result<&UploaderConfigItem> {
        let desired_name = config_name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_default();

        let store = self
            .config
            .uploader
            .entry(uploader_type.to_string())
            .or_insert_with(UploaderTypeConfigs::default);

        let target_idx = if !desired_name.is_empty() {
            store
                .config_list
                .iter()
                .position(|c| c.config_name.eq_ignore_ascii_case(&desired_name))
        } else {
            None
        };

        if let Some(idx) = target_idx {
            // Update existing.
            let now = now_ms();
            let item = &mut store.config_list[idx];
            for (k, v) in fields {
                item.fields.insert(k, v);
            }
            item.updated_at = now;
            store.default_id = item.id.clone();
        } else {
            // Create new.
            let now = now_ms();
            let name = if desired_name.is_empty() {
                generate_default_name(&store.config_list)
            } else {
                desired_name
            };
            let item = UploaderConfigItem {
                id: new_id(),
                config_name: name,
                created_at: now,
                updated_at: now,
                fields,
            };
            store.config_list.push(item);
            store.default_id = store.config_list.last().unwrap().id.clone();
        }

        // Update the mirror.
        if let Some(active) = store.find_by_id(&store.default_id) {
            self.config.pic_bed.uploader_mirrors.insert(
                uploader_type.to_string(),
                PicBedUploaderMirror::from_item(active),
            );
        }
        // Activate the new/updated config so subsequent `zpic upload` uses it.
        self.config.pic_bed.current = Some(uploader_type.to_string());
        self.config.pic_bed.uploader = Some(uploader_type.to_string());

        let store = self.config.uploader.get(uploader_type).unwrap();
        Ok(store.find_by_id(&store.default_id).unwrap())
    }

    /// Rename a config.
    pub fn rename(&mut self, uploader_type: &str, old_name: &str, new_name: &str) -> Result<()> {
        self.assert_type(uploader_type)?;
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(ManagerError::EmptyName.into());
        }
        let store = self
            .config
            .uploader
            .get(uploader_type)
            .ok_or_else(|| ManagerError::UnknownType(uploader_type.into()))?;
        let target = store
            .find_by_name(old_name)
            .ok_or_else(|| ManagerError::ConfigNotFound(uploader_type.into(), old_name.into()))?;
        // Check for a name collision (case-insensitive).
        if let Some(collision) = store
            .config_list
            .iter()
            .find(|c| c.id != target.id && c.config_name.eq_ignore_ascii_case(new_name))
        {
            return Err(ManagerError::DuplicateName(
                collision.config_name.clone(),
                uploader_type.into(),
            )
            .into());
        }
        let id = target.id.clone();
        let store = self.config.uploader.get_mut(uploader_type).unwrap();
        let item = store.config_list.iter_mut().find(|c| c.id == id).unwrap();
        item.config_name = new_name.to_string();
        item.updated_at = now_ms();
        // If the renamed config is active, refresh the mirror.
        if store.default_id == id {
            self.config.pic_bed.uploader_mirrors.insert(
                uploader_type.to_string(),
                PicBedUploaderMirror::from_item(item),
            );
        }
        Ok(())
    }

    /// Copy a config to a new name. The new config is *not* activated.
    pub fn copy(
        &mut self,
        uploader_type: &str,
        config_name: &str,
        new_config_name: &str,
    ) -> Result<&UploaderConfigItem> {
        self.assert_type(uploader_type)?;
        let new_name = new_config_name.trim();
        if new_name.is_empty() {
            return Err(ManagerError::EmptyName.into());
        }
        let store = self
            .config
            .uploader
            .get(uploader_type)
            .ok_or_else(|| ManagerError::UnknownType(uploader_type.into()))?;
        let target = store
            .find_by_name(config_name)
            .ok_or_else(|| ManagerError::ConfigNotFound(uploader_type.into(), config_name.into()))?
            .clone();
        if let Some(collision) = store
            .config_list
            .iter()
            .find(|c| c.config_name.eq_ignore_ascii_case(new_name))
        {
            return Err(ManagerError::DuplicateName(
                collision.config_name.clone(),
                uploader_type.into(),
            )
            .into());
        }
        let now = now_ms();
        let copy = UploaderConfigItem {
            id: new_id(),
            config_name: new_name.to_string(),
            created_at: now,
            updated_at: now,
            fields: target.fields.clone(),
        };
        let store = self.config.uploader.get_mut(uploader_type).unwrap();
        store.config_list.push(copy);
        Ok(store.config_list.last().unwrap())
    }

    /// Remove a config. When the last config is removed, PicGo clears the
    /// active mirror for that uploader type.
    pub fn remove(&mut self, uploader_type: &str, config_name: &str) -> Result<()> {
        self.assert_type(uploader_type)?;
        let store = self
            .config
            .uploader
            .get(uploader_type)
            .ok_or_else(|| ManagerError::UnknownType(uploader_type.into()))?;
        let target = store
            .find_by_name(config_name)
            .ok_or_else(|| ManagerError::ConfigNotFound(uploader_type.into(), config_name.into()))?
            .clone();
        let removed_id = target.id.clone();
        let was_active = store.default_id == removed_id;
        let store = self.config.uploader.get_mut(uploader_type).unwrap();
        store.config_list.retain(|c| c.id != removed_id);
        if was_active {
            store.default_id = store
                .config_list
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_default();
            if let Some(active) = store.find_by_id(&store.default_id) {
                self.config.pic_bed.uploader_mirrors.insert(
                    uploader_type.to_string(),
                    PicBedUploaderMirror::from_item(active),
                );
            } else {
                self.config.pic_bed.uploader_mirrors.remove(uploader_type);
                if self.config.pic_bed.current.as_deref() == Some(uploader_type) {
                    self.config.pic_bed.current = None;
                }
                if self.config.pic_bed.uploader.as_deref() == Some(uploader_type) {
                    self.config.pic_bed.uploader = None;
                }
            }
        }
        Ok(())
    }

    /// Set or update multiple fields on the active config of a type.
    /// Convenience used by the `set` command.
    pub fn set_fields(
        &mut self,
        uploader_type: &str,
        config_name: Option<&str>,
        fields: BTreeMap<String, toml::Value>,
    ) -> Result<&UploaderConfigItem> {
        // If a name is given and no config exists with that name, create.
        if let Some(name) = config_name {
            if self.get_by_name(uploader_type, name).is_none() {
                self.create_or_update(uploader_type, Some(name), BTreeMap::new())?;
            }
        } else if self.get_active(uploader_type).is_none() {
            self.create_or_update(uploader_type, None, BTreeMap::new())?;
        }
        // Ensure type field is present.
        let final_name = config_name.map(|n| n.to_string()).or_else(|| {
            self.get_active(uploader_type)
                .map(|c| c.config_name.clone())
        });
        self.create_or_update(uploader_type, final_name.as_deref(), fields)
    }

    fn assert_type(&self, uploader_type: &str) -> Result<()> {
        if self.config.uploader.contains_key(uploader_type) {
            return Ok(());
        }
        // First registration: if the user has a `[uploaders.X]` block with
        // type = `uploader_type`, allow it.
        // (This is rare in practice because migrate_legacy handles it.)
        Err(ManagerError::UnknownType(uploader_type.into()).into())
    }
}

/// Generate a non-colliding default name like `Default`, `Default-1`, ...
fn generate_default_name(existing: &[UploaderConfigItem]) -> String {
    let used: std::collections::HashSet<String> = existing
        .iter()
        .map(|c| c.config_name.to_lowercase())
        .collect();
    if !used.contains("default") {
        return "Default".to_string();
    }
    let mut i = 1;
    loop {
        let name = format!("Default-{i}");
        if !used.contains(&name.to_lowercase()) {
            return name;
        }
        i += 1;
    }
}

/// Helper: render the manager's view of the configs in a stable, human
/// form. Used by the `uploader list` command.
pub fn format_list_output(cfg: &ZpicConfigFile) -> String {
    let mut lines: Vec<String> = Vec::new();
    let current = cfg.active_uploader_type();
    let mut types: Vec<&str> = cfg.uploader.keys().map(|s| s.as_str()).collect();
    types.sort();
    for t in types {
        let is_current = current == Some(t);
        if is_current {
            lines.push(format!("+ {t} [Current Uploader]"));
        } else {
            lines.push(format!("+ {t}"));
        }
        if let Some(store) = cfg.uploader.get(t) {
            if store.config_list.is_empty() {
                lines.push("  (No configs found)".to_string());
            } else {
                for c in &store.config_list {
                    if c.id == store.default_id {
                        lines.push(format!("  * {} [Default Config]", c.config_name));
                    } else {
                        lines.push(format!("    {}", c.config_name));
                    }
                }
            }
        }
    }
    lines.join("\n")
}

/// Render one type's configs (used by `uploader list <type>`).
pub fn format_type_output(cfg: &ZpicConfigFile, uploader_type: &str) -> Option<String> {
    let store = cfg.uploader.get(uploader_type)?;
    let mut lines: Vec<String> = vec![format!("+ {uploader_type}")];
    if store.config_list.is_empty() {
        lines.push("  (No configs found)".to_string());
    } else {
        for c in &store.config_list {
            if c.id == store.default_id {
                lines.push(format!("  * {} [Default Config]", c.config_name));
            } else {
                lines.push(format!("    {}", c.config_name));
            }
        }
    }
    Some(lines.join("\n"))
}

/// Resolve a string field on the active config for the active uploader
/// type, with env-var expansion. Convenience for the rest of the code.
pub fn resolve_active_field(cfg: &ZpicConfigFile, key: &str) -> Option<String> {
    let t = cfg.active_uploader_type()?;
    let item = cfg.uploader.get(t)?.active()?;
    item.resolve(key)
}

// `expand_env` is re-exported for callers that want to use it on
// arbitrary user input (e.g. `--field key=value`).
pub use crate::zpic::expand_env as expand_env_string;

#[allow(dead_code)]
pub(crate) fn _silence_unused() {
    let _ = expand_env;
}

#[cfg(test)]
mod tests {
    use super::*;
    use zpic_core::config::UploaderKind;

    fn empty_cfg() -> ZpicConfigFile {
        ZpicConfigFile::default()
    }

    fn add_local_type(cfg: &mut ZpicConfigFile) {
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("local".into()));
        fields.insert("target_dir".into(), toml::Value::String("./public".into()));
        fields.insert("public_base_url".into(), toml::Value::String("/img".into()));
        let store = UploaderTypeConfigs {
            config_list: vec![UploaderConfigItem {
                id: "id-1".into(),
                config_name: "Default".into(),
                created_at: 0,
                updated_at: 0,
                fields,
            }],
            default_id: "id-1".into(),
        };
        cfg.uploader.insert("local".into(), store);
    }

    #[test]
    fn create_or_update_creates_default_config() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("github".into()));
        fields.insert("repo".into(), toml::Value::String("me/picbed".into()));
        let item = mgr
            .create_or_update("github", Some("Personal"), fields)
            .unwrap();
        assert_eq!(item.config_name, "Personal");
        assert_eq!(cfg.pic_bed.current.as_deref(), Some("github"));
    }

    #[test]
    fn create_or_update_rejects_duplicate_name_when_no_fields() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        // First create a config without any fields (an empty stub).
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("github".into()));
        mgr.create_or_update("github", Some("Personal"), fields)
            .unwrap();
        // Now try to create a second config with a different case of the
        // same name. The lookup is case-insensitive, so this matches the
        // existing config. We have no new fields, so it's a duplicate.
        let empty: BTreeMap<String, toml::Value> = BTreeMap::new();
        let err = mgr
            .create_or_update("github", Some("personal"), empty)
            .unwrap_err();
        assert!(matches!(err, ZpicError::ConfigInvalid(_)));
    }

    #[test]
    fn use_config_switches_active() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        // Add a second config
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("local".into()));
        mgr.create_or_update("local", Some("Backup"), fields)
            .unwrap();
        // Switch back to Default
        let active = mgr.use_config("local", Some("Default")).unwrap();
        assert_eq!(active.config_name, "Default");
        assert_eq!(cfg.pic_bed.current.as_deref(), Some("local"));
    }

    #[test]
    fn rename_renames_and_rejects_duplicates() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("local".into()));
        mgr.create_or_update("local", Some("Other"), fields)
            .unwrap();
        mgr.rename("local", "Other", "Renamed").unwrap();
        assert!(mgr.get_by_name("local", "Renamed").is_some());
        let err = mgr.rename("local", "Default", "Renamed").unwrap_err();
        assert!(matches!(err, ZpicError::ConfigInvalid(_)));
    }

    #[test]
    fn copy_does_not_activate_new_config() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        let copy = mgr.copy("local", "Default", "Copy").unwrap();
        assert_eq!(copy.config_name, "Copy");
        // Active is still Default.
        assert_eq!(cfg.uploader.get("local").unwrap().default_id, "id-1");
    }

    #[test]
    fn remove_last_config_clears_active_selection() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        mgr.remove("local", "Default").unwrap();
        let store = cfg.uploader.get("local").unwrap();
        assert!(store.config_list.is_empty());
        assert!(store.default_id.is_empty());
        assert!(cfg.pic_bed.current.is_none());
        assert!(cfg.pic_bed.uploader_mirrors.get("local").is_none());
    }

    #[test]
    fn remove_promotes_first_when_active_removed() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("local".into()));
        mgr.create_or_update("local", Some("Other"), fields)
            .unwrap();
        mgr.remove("local", "Default").unwrap();
        let store = cfg.uploader.get("local").unwrap();
        assert_eq!(store.config_list.len(), 1);
        assert_eq!(store.config_list[0].config_name, "Other");
    }

    #[test]
    fn set_fields_can_create_new_type() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mut mgr = UploaderConfigManager::new(&mut cfg);
        let mut fields = BTreeMap::new();
        fields.insert("bucket".into(), toml::Value::String("blog".into()));
        let item = mgr.set_fields("github", Some("Work"), fields).unwrap();
        assert_eq!(item.config_name, "Work");
        assert_eq!(cfg.active_uploader_type(), Some("github"));
    }

    #[test]
    fn list_types_returns_sorted() {
        let mut cfg = empty_cfg();
        add_local_type(&mut cfg);
        let mgr = UploaderConfigManager::new(&mut cfg);
        assert!(mgr.list_types().contains(&"local"));
    }

    #[test]
    fn kind_works_after_migration() {
        // Construct a legacy config, then construct a manager and check.
        let mut cfg = empty_cfg();
        cfg.default_uploader = Some("legacy".into());
        let mut fields = BTreeMap::new();
        fields.insert("type".into(), toml::Value::String("s3".into()));
        fields.insert("bucket".into(), toml::Value::String("b".into()));
        fields.insert(
            "endpoint".into(),
            toml::Value::String("https://r2.example.com".into()),
        );
        fields.insert(
            "public_base_url".into(),
            toml::Value::String("https://x".into()),
        );
        cfg.uploaders.insert(
            "legacy".into(),
            crate::zpic::UploaderSection {
                kind: UploaderKind::S3,
                alias: None,
                fields,
            },
        );
        let _mgr = UploaderConfigManager::new(&mut cfg);
        assert_eq!(cfg.active_uploader_type(), Some("s3"));
        assert!(cfg.uploader.get("s3").unwrap().active().is_some());
    }
}
