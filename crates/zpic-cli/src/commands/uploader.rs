//! `zpic uploader` — manage named uploader configurations.

use std::path::PathBuf;

use serde::Serialize;

use crate::cli::UploaderAction;
use crate::util::{load_config, resolve_existing_type_key, save_loaded_config};
use zpic_config::{format_list_output, format_type_output, UploaderConfigManager};
use zpic_core::error::{Result, ZpicError};

pub fn run(action: UploaderAction, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    match action {
        UploaderAction::List { uploader_type } => cmd_list(uploader_type, explicit_config, json),
        UploaderAction::Rename {
            uploader_type,
            old_name,
            new_name,
        } => cmd_rename(uploader_type, old_name, new_name, explicit_config, json),
        UploaderAction::Copy {
            uploader_type,
            config_name,
            new_config_name,
        } => cmd_copy(
            uploader_type,
            config_name,
            new_config_name,
            explicit_config,
            json,
        ),
        UploaderAction::Rm {
            uploader_type,
            config_name,
        } => cmd_remove(uploader_type, config_name, explicit_config, json),
    }
}

fn cmd_list(
    uploader_type: Option<String>,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let config = load_config(explicit_config.as_deref())?;
    let selected_type = match uploader_type {
        Some(requested) => Some(resolve_existing_type_key(&config, &requested)?),
        None => None,
    };

    if json {
        let payload = UploaderListPayload::from_config(&config.zpic, selected_type.as_deref());
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else if let Some(selected_type) = selected_type {
        let output = format_type_output(&config.zpic, &selected_type).ok_or_else(|| {
            ZpicError::UploaderNotFound(format!("{selected_type} (no configs found)"))
        })?;
        println!("{output}");
    } else {
        println!("{}", format_list_output(&config.zpic));
    }

    Ok(0)
}

fn cmd_rename(
    uploader_type: String,
    old_name: String,
    new_name: String,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let mut config = load_config(explicit_config.as_deref())?;
    let uploader_type = resolve_existing_type_key(&config, &uploader_type)?;
    let (active_config, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        manager.rename(&uploader_type, &old_name, &new_name)?;
        let active_config = manager
            .get_active(&uploader_type)
            .map(|item| item.config_name.clone());
        let saved_to = save_loaded_config(&config)?;
        (active_config, saved_to)
    };

    if json {
        print_json(&RenamePayload {
            action: "rename",
            uploader_type,
            old_name,
            new_name,
            active_config,
            saved_to: saved_to.display().to_string(),
        })?;
    } else {
        println!(
            "renamed `{}` config `{}` to `{}`",
            uploader_type, old_name, new_name
        );
        maybe_print_saved_path(&config.source.path().display().to_string(), &saved_to);
    }

    Ok(0)
}

fn cmd_copy(
    uploader_type: String,
    config_name: String,
    new_config_name: String,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let mut config = load_config(explicit_config.as_deref())?;
    let uploader_type = resolve_existing_type_key(&config, &uploader_type)?;
    let (active_config, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        manager.copy(&uploader_type, &config_name, &new_config_name)?;
        let active_config = manager
            .get_active(&uploader_type)
            .map(|item| item.config_name.clone());
        let saved_to = save_loaded_config(&config)?;
        (active_config, saved_to)
    };

    if json {
        print_json(&CopyPayload {
            action: "copy",
            uploader_type,
            config_name,
            new_config_name,
            active_config,
            saved_to: saved_to.display().to_string(),
        })?;
    } else {
        println!(
            "copied `{}` config `{}` to `{}`",
            uploader_type, config_name, new_config_name
        );
        maybe_print_saved_path(&config.source.path().display().to_string(), &saved_to);
    }

    Ok(0)
}

fn cmd_remove(
    uploader_type: String,
    config_name: String,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let mut config = load_config(explicit_config.as_deref())?;
    let uploader_type = resolve_existing_type_key(&config, &uploader_type)?;
    let (active_config, remaining, current_uploader, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        manager.remove(&uploader_type, &config_name)?;
        let active_config = manager
            .get_active(&uploader_type)
            .map(|item| item.config_name.clone());
        let remaining = manager.list_configs(&uploader_type).len();
        let current_uploader = config.active_uploader_type().map(str::to_string);
        let saved_to = save_loaded_config(&config)?;
        (active_config, remaining, current_uploader, saved_to)
    };

    if json {
        print_json(&RemovePayload {
            action: "remove",
            uploader_type,
            removed_name: config_name,
            active_config,
            remaining_configs: remaining,
            current_uploader,
            saved_to: saved_to.display().to_string(),
        })?;
    } else {
        println!("removed `{}` config `{}`", uploader_type, config_name);
        maybe_print_saved_path(&config.source.path().display().to_string(), &saved_to);
    }

    Ok(0)
}

fn maybe_print_saved_path(original_path: &str, saved_to: &std::path::Path) {
    if saved_to.display().to_string() != original_path {
        println!("wrote native zpic config to {}", saved_to.display());
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| ZpicError::Internal(e.to_string()))?
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct UploaderListPayload {
    current_uploader: Option<String>,
    types: Vec<UploaderTypePayload>,
}

impl UploaderListPayload {
    fn from_config(config: &zpic_config::ZpicConfigFile, selected_type: Option<&str>) -> Self {
        let mut types = Vec::new();
        let mut keys: Vec<&String> = config.uploader.keys().collect();
        keys.sort();
        for key in keys {
            if selected_type
                .map(|selected| !key.eq_ignore_ascii_case(selected))
                .unwrap_or(false)
            {
                continue;
            }
            let store = &config.uploader[key];
            let default_id = store.default_id.clone();
            let configs = store
                .config_list
                .iter()
                .map(|item| UploaderConfigPayload {
                    id: item.id.clone(),
                    name: item.config_name.clone(),
                    is_default: item.id == default_id,
                    created_at: item.created_at,
                    updated_at: item.updated_at,
                })
                .collect();
            let default_config = store.active().map(|item| item.config_name.clone());
            types.push(UploaderTypePayload {
                uploader_type: key.clone(),
                is_current: config.active_uploader_type() == Some(key.as_str()),
                default_config,
                configs,
            });
        }
        Self {
            current_uploader: config.active_uploader_type().map(str::to_string),
            types,
        }
    }
}

#[derive(Debug, Serialize)]
struct UploaderTypePayload {
    #[serde(rename = "type")]
    uploader_type: String,
    is_current: bool,
    default_config: Option<String>,
    configs: Vec<UploaderConfigPayload>,
}

#[derive(Debug, Serialize)]
struct UploaderConfigPayload {
    id: String,
    name: String,
    is_default: bool,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
struct RenamePayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    old_name: String,
    new_name: String,
    active_config: Option<String>,
    saved_to: String,
}

#[derive(Debug, Serialize)]
struct CopyPayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    config_name: String,
    new_config_name: String,
    active_config: Option<String>,
    saved_to: String,
}

#[derive(Debug, Serialize)]
struct RemovePayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    removed_name: String,
    active_config: Option<String>,
    remaining_configs: usize,
    current_uploader: Option<String>,
    saved_to: String,
}
