//! Small shared helpers used by the CLI commands.

use std::path::{Path, PathBuf};

use zpic_config::UploaderConfigItem;
use zpic_config::loader::{ConfigLoader, ConfigSource, LoadedConfig};
use zpic_config::paths::default_zpic_config;
use zpic_core::error::{Result, ZpicError};
use zpic_plugins::{
    discover_plugin_descriptors, PluginDiagnostic, PluginDiagnosticLevel, UploaderDescriptor,
    UploaderRegistry,
};
use zpic_uploaders::builtin_uploader_descriptors;

pub struct LoadedUploaderRegistry {
    pub registry: UploaderRegistry,
    pub diagnostics: Vec<PluginDiagnostic>,
}

#[derive(Clone)]
pub struct ResolvedUploaderTarget {
    pub configured_type: String,
    pub runtime_type: String,
    pub config_name: String,
    pub item: UploaderConfigItem,
    pub descriptor: UploaderDescriptor,
}

impl ResolvedUploaderTarget {
    pub fn instantiate(&self) -> Result<Box<dyn zpic_core::upload::Uploader>> {
        self.descriptor
            .instantiate(&self.runtime_type, &self.item)
    }

    pub fn validate(&self) -> Result<()> {
        self.descriptor.validate(&self.runtime_type, &self.item)
    }
}

/// Load the zpic config, falling back to default paths. Returns
/// `ConfigNotFound` if nothing usable is on disk.
pub fn load_config(explicit: Option<&Path>) -> Result<LoadedConfig> {
    ConfigLoader::load(explicit.map(|p| p.to_path_buf())).or_else(|e| match e {
        ZpicError::ConfigNotFound => Err(e),
        other => Err(other),
    })
}

/// Resolve the active uploader section, optionally overriding the configured
/// uploader type. Returns the resolved uploader type and a concrete section
/// that the uploader factory can consume.
pub fn load_uploader_registry() -> Result<LoadedUploaderRegistry> {
    let mut registry = UploaderRegistry::default();
    for descriptor in builtin_uploader_descriptors() {
        registry.register(descriptor)?;
    }

    let mut diagnostics = Vec::new();
    let (plugin_descriptors, plugin_diagnostics) = discover_plugin_descriptors(Default::default());
    diagnostics.extend(plugin_diagnostics);
    for descriptor in plugin_descriptors {
        if let Err(err) = registry.register(descriptor.clone()) {
            diagnostics.push(PluginDiagnostic {
                level: PluginDiagnosticLevel::Fail,
                path: descriptor.type_name.clone(),
                plugin_id: None,
                message: err.to_string(),
            });
        }
    }

    Ok(LoadedUploaderRegistry {
        registry,
        diagnostics,
    })
}

pub fn resolve_uploader(
    config: &LoadedConfig,
    registry: &UploaderRegistry,
    override_type: Option<&str>,
) -> Result<ResolvedUploaderTarget> {
    let uploader_type = if let Some(requested) = override_type {
        resolve_registered_type_key(config, registry, requested, false)?
    } else {
        resolve_active_type_key(config, registry)?
    };

    let active = config
        .zpic
        .uploader
        .get(&uploader_type)
        .and_then(|store| store.active())
        .ok_or_else(|| {
            ZpicError::UploaderNotFound(format!(
                "{uploader_type} (no active config; run `zpic set uploader {uploader_type} <name>` first)"
            ))
        })?;
    let descriptor = registry.resolve(&uploader_type).ok_or_else(|| {
        ZpicError::UploaderUnsupported(uploader_type.clone())
    })?;

    Ok(ResolvedUploaderTarget {
        configured_type: uploader_type,
        runtime_type: descriptor.type_name.clone(),
        config_name: active.config_name.clone(),
        item: active.clone(),
        descriptor: descriptor.clone(),
    })
}

pub fn resolve_existing_type_key(config: &LoadedConfig, requested: &str) -> Result<String> {
    resolve_config_type_key(config, requested, false)
}

pub fn resolve_or_create_type_key(
    config: &LoadedConfig,
    registry: &UploaderRegistry,
    requested: &str,
) -> Result<String> {
    resolve_registered_type_key(config, registry, requested, true)
}

pub fn resolve_active_type_key(
    config: &LoadedConfig,
    registry: &UploaderRegistry,
) -> Result<String> {
    let active = config
        .active_uploader_type()
        .ok_or_else(|| ZpicError::UploaderNotFound("<no active uploader>".into()))?;
    resolve_registered_type_key(config, registry, active, false)
}

/// Persist the loaded config as native TOML. When the current source is a
/// PicGo JSON file, zpic writes a native config to the default zpic location
/// instead of mutating the PicGo source.
pub fn save_loaded_config(config: &LoadedConfig) -> Result<PathBuf> {
    let path = writable_config_path(&config.source);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text =
        toml::to_string_pretty(&config.zpic).map_err(|e| ZpicError::Internal(e.to_string()))?;
    std::fs::write(&path, text)?;
    Ok(path)
}

fn writable_config_path(source: &ConfigSource) -> PathBuf {
    match source {
        ConfigSource::Explicit(path)
        | ConfigSource::EnvVar(path)
        | ConfigSource::Project(path)
        | ConfigSource::User(path)
            if !path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false) =>
        {
            path.clone()
        }
        _ => default_zpic_config(),
    }
}

fn resolve_registered_type_key(
    config: &LoadedConfig,
    registry: &UploaderRegistry,
    requested: &str,
    allow_create: bool,
) -> Result<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(ZpicError::InvalidArgument(
            "uploader type can not be empty".into(),
        ));
    }

    if let Some(existing) = config
        .zpic
        .uploader
        .keys()
        .find(|candidate| candidate.eq_ignore_ascii_case(requested))
    {
        return Ok(existing.clone());
    }

    if let Some(canonical) = registry.canonical_type(requested) {
        let mut matches: Vec<String> = config
            .zpic
            .uploader
            .keys()
            .filter(|candidate| {
                registry
                    .canonical_type(candidate)
                    .map(|candidate_kind| candidate_kind.eq_ignore_ascii_case(canonical))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        matches.sort();
        matches.dedup();
        match matches.len() {
            0 if allow_create => return Ok(canonical.to_string()),
            0 => {}
            1 => return Ok(matches.remove(0)),
            _ => {
                if let Some(canonical) = matches
                    .iter()
                    .find(|candidate| candidate.eq_ignore_ascii_case(canonical))
                {
                    return Ok(canonical.clone());
                }
                if let Some(active) = config.active_uploader_type() {
                    if let Some(active_match) = matches
                        .iter()
                        .find(|candidate| candidate.eq_ignore_ascii_case(active))
                    {
                        return Ok(active_match.clone());
                    }
                }
                return Err(ZpicError::ConfigInvalid(format!(
                    "uploader type '{requested}' is ambiguous; matching config groups: {}",
                    matches.join(", ")
                )));
            }
        }
    }

    if allow_create {
        Ok(requested.to_ascii_lowercase())
    } else {
        Err(ZpicError::UploaderNotFound(requested.to_string()))
    }
}

fn resolve_config_type_key(config: &LoadedConfig, requested: &str, allow_create: bool) -> Result<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(ZpicError::InvalidArgument(
            "uploader type can not be empty".into(),
        ));
    }

    if let Some(existing) = config
        .zpic
        .uploader
        .keys()
        .find(|candidate| candidate.eq_ignore_ascii_case(requested))
    {
        return Ok(existing.clone());
    }

    if allow_create {
        Ok(requested.to_ascii_lowercase())
    } else {
        Err(ZpicError::UploaderNotFound(requested.to_string()))
    }
}

/// Join two `?`-style error chains for a clearer user message.
#[allow(dead_code)]
pub fn chain_err<E: std::fmt::Display>(prefix: &str, e: E) -> String {
    format!("{prefix}: {e}")
}
