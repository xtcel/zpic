//! Small shared helpers used by the CLI commands.

use std::path::{Path, PathBuf};

use zpic_config::loader::{ConfigLoader, ConfigSource, LoadedConfig};
use zpic_config::paths::default_zpic_config;
use zpic_core::config::UploaderKind;
use zpic_core::error::{Result, ZpicError};

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
pub fn resolve_uploader(
    config: &LoadedConfig,
    override_type: Option<&str>,
) -> Result<(String, zpic_config::UploaderSection)> {
    let uploader_type = if let Some(requested) = override_type {
        resolve_type_key(config, requested, false)?
    } else {
        resolve_active_type_key(config)?
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

    Ok((
        uploader_type.clone(),
        active.to_uploader_section_for_type(&uploader_type),
    ))
}

pub fn resolve_existing_type_key(config: &LoadedConfig, requested: &str) -> Result<String> {
    resolve_type_key(config, requested, false)
}

pub fn resolve_or_create_type_key(config: &LoadedConfig, requested: &str) -> Result<String> {
    resolve_type_key(config, requested, true)
}

pub fn resolve_active_type_key(config: &LoadedConfig) -> Result<String> {
    let active = config
        .active_uploader_type()
        .ok_or_else(|| ZpicError::UploaderNotFound("<no active uploader>".into()))?;
    resolve_type_key(config, active, false)
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

fn resolve_type_key(config: &LoadedConfig, requested: &str, allow_create: bool) -> Result<String> {
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

    if let Some(kind) = UploaderKind::from_alias(requested) {
        let mut matches: Vec<String> = config
            .zpic
            .uploader
            .keys()
            .filter(|candidate| {
                UploaderKind::from_alias(candidate)
                    .map(|candidate_kind| candidate_kind == kind)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        matches.sort();
        matches.dedup();
        match matches.len() {
            0 if allow_create => return Ok(requested.to_ascii_lowercase()),
            0 => {}
            1 => return Ok(matches.remove(0)),
            _ => {
                if let Some(canonical) = matches
                    .iter()
                    .find(|candidate| candidate.eq_ignore_ascii_case(kind.as_str()))
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

/// Join two `?`-style error chains for a clearer user message.
#[allow(dead_code)]
pub fn chain_err<E: std::fmt::Display>(prefix: &str, e: E) -> String {
    format!("{prefix}: {e}")
}
