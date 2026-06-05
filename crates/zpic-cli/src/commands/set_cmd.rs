//! `zpic set` — create or update module configuration.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::cli::{SetAction, SetUploaderArgs};
use crate::util::{load_config, resolve_or_create_type_key, save_loaded_config};
use zpic_config::UploaderConfigManager;
use zpic_core::error::{Result, ZpicError};

pub fn run(action: SetAction, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    match action {
        SetAction::Uploader(args) => cmd_set_uploader(args, explicit_config, json),
    }
}

fn cmd_set_uploader(
    args: SetUploaderArgs,
    explicit_config: Option<PathBuf>,
    json: bool,
) -> Result<i32> {
    let mut config = load_config(explicit_config.as_deref())?;
    let uploader_type = resolve_or_create_type_key(&config, &args.uploader_type)?;
    let patch_fields = parse_fields(&args.fields)?;

    let (active_config, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        let mut seed_fields = if let Some(from_name) = args.from.as_deref() {
            manager
                .get_by_name(&uploader_type, from_name)
                .ok_or_else(|| {
                    ZpicError::ConfigInvalid(format!(
                        "config '{}' not found in type '{}'",
                        from_name, uploader_type
                    ))
                })?
                .fields
                .clone()
        } else {
            BTreeMap::new()
        };
        for (key, value) in patch_fields {
            seed_fields.insert(key, value);
        }
        let active =
            manager.create_or_update(&uploader_type, Some(&args.config_name), seed_fields)?;
        let active_config = active.config_name.clone();
        let saved_to = save_loaded_config(&config)?;
        (active_config, saved_to)
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&SetPayload {
                action: "set",
                uploader_type,
                active_config,
                inherited_from: args.from,
                saved_to: saved_to.display().to_string(),
            })
            .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "saved `{}` config `{}` and made it active",
            uploader_type, active_config
        );
        if saved_to != config.source.path() {
            println!("wrote native zpic config to {}", saved_to.display());
        }
    }

    Ok(0)
}

fn parse_fields(items: &[String]) -> Result<BTreeMap<String, toml::Value>> {
    let mut out = BTreeMap::new();
    for item in items {
        let (key, raw_value) = item.split_once('=').ok_or_else(|| {
            ZpicError::InvalidArgument(format!("invalid --field '{}'; expected KEY=VALUE", item))
        })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(ZpicError::InvalidArgument(format!(
                "invalid --field '{}'; key can not be empty",
                item
            )));
        }
        out.insert(key.to_string(), parse_toml_value(raw_value.trim()));
    }
    Ok(out)
}

fn parse_toml_value(raw: &str) -> toml::Value {
    if raw.is_empty() {
        return toml::Value::String(String::new());
    }
    let snippet = format!("value = {raw}");
    if let Ok(table) = toml::from_str::<toml::Table>(&snippet) {
        if let Some(value) = table.get("value") {
            return value.clone();
        }
    }
    toml::Value::String(raw.to_string())
}

#[derive(Debug, Serialize)]
struct SetPayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    active_config: String,
    inherited_from: Option<String>,
    saved_to: String,
}
