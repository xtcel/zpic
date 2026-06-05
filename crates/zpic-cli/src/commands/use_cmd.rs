//! `zpic use` — activate a module selection.

use std::path::PathBuf;

use serde::Serialize;

use crate::cli::UseAction;
use crate::util::{load_config, resolve_existing_type_key, save_loaded_config};
use zpic_config::UploaderConfigManager;
use zpic_core::error::{Result, ZpicError};

pub fn run(action: UseAction, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    match action {
        UseAction::Uploader {
            uploader_type,
            config_name,
        } => cmd_use_uploader(uploader_type, config_name, explicit_config, json),
    }
}

fn cmd_use_uploader(
    uploader_type: String,
    config_name: Option<String>,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let mut config = load_config(explicit_config.as_deref())?;
    let uploader_type = resolve_existing_type_key(&config, &uploader_type)?;
    let (active_config, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        let active = manager.use_config(&uploader_type, config_name.as_deref())?;
        let payload_name = active.config_name.clone();
        let saved_to = save_loaded_config(&config)?;
        (payload_name, saved_to)
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&UsePayload {
                action: "use",
                uploader_type,
                active_config,
                saved_to: saved_to.display().to_string(),
            })
            .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "active uploader set to `{}` (config: `{}`)",
            uploader_type, active_config
        );
        if saved_to != config.source.path() {
            println!("wrote native zpic config to {}", saved_to.display());
        }
    }

    Ok(0)
}

#[derive(Debug, Serialize)]
struct UsePayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    active_config: String,
    saved_to: String,
}
