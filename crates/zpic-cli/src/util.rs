//! Small shared helpers used by the CLI commands.

use std::path::Path;

use zpic_config::loader::{ConfigLoader, LoadedConfig};
use zpic_core::error::{Result, ZpicError};

/// Load the zpic config, falling back to default paths. Returns
/// `ConfigNotFound` if nothing usable is on disk.
pub fn load_config(explicit: Option<&Path>) -> Result<LoadedConfig> {
    ConfigLoader::load(explicit.map(|p| p.to_path_buf())).or_else(|e| match e {
        ZpicError::ConfigNotFound => Err(e),
        other => Err(other),
    })
}

/// Resolve the active uploader section, optionally overriding the
/// configured default. Returns `UploaderNotFound` if the named uploader
/// is not present in the config. Returns the owned name so the caller
/// doesn't have to thread lifetimes through.
pub fn resolve_uploader(
    config: &LoadedConfig,
    override_name: Option<&str>,
) -> Result<(String, zpic_config::UploaderSection)> {
    let name = override_name
        .map(String::from)
        .or_else(|| config.default_uploader_name().map(String::from))
        .ok_or_else(|| ZpicError::UploaderNotFound("<no default uploader>".into()))?;
    let section = config
        .zpic
        .uploaders
        .get(&name)
        .cloned()
        .ok_or_else(|| ZpicError::UploaderNotFound(name.clone()))?;
    Ok((name, section))
}

/// Join two `?`-style error chains for a clearer user message.
#[allow(dead_code)]
pub fn chain_err<E: std::fmt::Display>(prefix: &str, e: E) -> String {
    format!("{prefix}: {e}")
}
