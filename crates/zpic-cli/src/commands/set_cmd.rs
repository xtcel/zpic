//! `zpic set` — create or update module configuration.

use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use cliclack::{input, password, select};
use serde::Serialize;

use crate::cli::{SetAction, SetUploaderArgs};
use crate::util::{load_config, resolve_or_create_type_key, save_loaded_config};
use zpic_config::{UploaderConfigItem, UploaderConfigManager};
use zpic_core::config::UploaderKind;
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
    let interactive = !json && (args.uploader_type.is_none() || args.fields.is_empty());

    let resolved = if interactive {
        collect_interactive_args(&config, args)?
    } else {
        finalize_args(&config, args)?
    };

    let (active_config, saved_to) = {
        let mut manager = UploaderConfigManager::new(&mut config.zpic);
        let mut seed_fields = resolved.base_fields;
        for (key, value) in resolved.fields {
            seed_fields.insert(key, value);
        }
        let active = manager.create_or_update(
            &resolved.uploader_type,
            Some(&resolved.config_name),
            seed_fields,
        )?;
        let active_config = active.config_name.clone();
        let saved_to = save_loaded_config(&config)?;
        (active_config, saved_to)
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&SetPayload {
                action: "set",
                uploader_type: resolved.uploader_type,
                active_config,
                inherited_from: resolved.from,
                saved_to: saved_to.display().to_string(),
            })
            .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "saved `{}` config `{}` and made it active",
            resolved.uploader_type, active_config
        );
        if saved_to != config.source.path() {
            println!("wrote native zpic config to {}", saved_to.display());
        }
    }

    Ok(0)
}

fn finalize_args(
    config: &zpic_config::LoadedConfig,
    args: SetUploaderArgs,
) -> Result<ResolvedSetArgs> {
    let uploader_type = args.uploader_type.ok_or_else(|| {
        ZpicError::InvalidArgument(
            "missing uploader type; run `zpic set uploader` for guided mode".into(),
        )
    })?;
    let uploader_type = resolve_or_create_type_key(config, &uploader_type)?;
    let config_name = args.config_name.ok_or_else(|| {
        ZpicError::InvalidArgument(
            "missing config name; run `zpic set uploader` for guided mode".into(),
        )
    })?;
    let from = args.from.filter(|value| !value.trim().is_empty());
    let base_fields =
        resolve_base_fields(config, &uploader_type, Some(&config_name), from.as_deref())?;
    Ok(ResolvedSetArgs {
        uploader_type,
        config_name,
        from,
        base_fields,
        fields: parse_fields(&args.fields)?,
    })
}

fn collect_interactive_args(
    config: &zpic_config::LoadedConfig,
    args: SetUploaderArgs,
) -> Result<ResolvedSetArgs> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        collect_interactive_args_menu(config, args)
    } else {
        collect_interactive_args_fallback(config, args)
    }
}

fn collect_interactive_args_menu(
    config: &zpic_config::LoadedConfig,
    args: SetUploaderArgs,
) -> Result<ResolvedSetArgs> {
    let current_type = config.active_uploader_type().map(str::to_string);
    let uploader_type = match args.uploader_type {
        Some(uploader_type) => resolve_or_create_type_key(config, &uploader_type)?,
        None => select_uploader_type_menu(current_type.as_deref())?,
    };

    let config_names = config_names_for_type(config, &uploader_type);
    let selected = match args.config_name {
        Some(config_name) => {
            let config_name = config_name.trim();
            if config_name.is_empty() {
                return Err(ZpicError::InvalidArgument(
                    "config name can not be empty".into(),
                ));
            }
            if let Some(existing) = find_config_name(&config_names, config_name) {
                SelectedConfig::Use(existing.to_string(), true)
            } else {
                SelectedConfig::Use(config_name.to_string(), false)
            }
        }
        None => select_config_name_menu(
            &uploader_type,
            &config_names,
            active_config_name_for_type(config, &uploader_type),
        )?,
    };

    let from = args.from.filter(|value| !value.trim().is_empty());
    let base_fields = resolve_base_fields(
        config,
        &uploader_type,
        match &selected {
            SelectedConfig::Use(name, true) => Some(name.as_str()),
            _ => None,
        },
        from.as_deref(),
    )?;

    let fields = if args.fields.is_empty() {
        collect_fields_for_type_menu(&uploader_type, &base_fields)?
    } else {
        parse_fields(&args.fields)?
    };

    Ok(ResolvedSetArgs {
        uploader_type,
        config_name: selected.name().to_string(),
        from,
        base_fields,
        fields,
    })
}

fn collect_interactive_args_fallback(
    config: &zpic_config::LoadedConfig,
    args: SetUploaderArgs,
) -> Result<ResolvedSetArgs> {
    let mut stdout = io::stdout();
    let mut stdin = io::stdin().lock();

    let current_type = config.active_uploader_type().map(str::to_string);
    let uploader_type = match args.uploader_type {
        Some(uploader_type) => resolve_or_create_type_key(config, &uploader_type)?,
        None => select_uploader_type(&mut stdin, &mut stdout, current_type.as_deref())?,
    };

    let config_names = config_names_for_type(config, &uploader_type);
    let selected = match args.config_name {
        Some(config_name) => {
            let config_name = config_name.trim();
            if config_name.is_empty() {
                return Err(ZpicError::InvalidArgument(
                    "config name can not be empty".into(),
                ));
            }
            if let Some(existing) = find_config_name(&config_names, config_name) {
                SelectedConfig::Use(existing.to_string(), true)
            } else {
                SelectedConfig::Use(config_name.to_string(), false)
            }
        }
        None => select_config_name(
            &mut stdin,
            &mut stdout,
            &uploader_type,
            &config_names,
            active_config_name_for_type(config, &uploader_type),
        )?,
    };

    let from = args.from.filter(|value| !value.trim().is_empty());

    let base_fields = resolve_base_fields(
        config,
        &uploader_type,
        match &selected {
            SelectedConfig::Use(name, true) => Some(name.as_str()),
            _ => None,
        },
        from.as_deref(),
    )?;

    let fields = if args.fields.is_empty() {
        collect_fields_for_type(&mut stdin, &mut stdout, &uploader_type, &base_fields)?
    } else {
        parse_fields(&args.fields)?
    };

    Ok(ResolvedSetArgs {
        uploader_type,
        config_name: selected.name().to_string(),
        from,
        base_fields,
        fields,
    })
}

fn select_uploader_type_menu(current_type: Option<&str>) -> Result<String> {
    let mut prompt = select("Choose uploader type");
    let mut initial_value = None;
    for kind in UploaderKind::all() {
        let value = kind.as_str().to_string();
        let label = if Some(kind.as_str()) == current_type {
            initial_value = Some(value.clone());
            format!("{} (Current)", kind.as_str())
        } else {
            kind.as_str().to_string()
        };
        prompt = prompt.item(value, label, "");
    }
    if let Some(current) = initial_value {
        prompt = prompt.initial_value(current);
    }
    prompt.interact().map_err(io_error)
}

fn select_config_name_menu(
    uploader_type: &str,
    config_names: &[String],
    active_config: Option<&str>,
) -> Result<SelectedConfig> {
    if config_names.is_empty() {
        return Ok(SelectedConfig::Use(
            prompt_config_name_menu(config_names)?,
            false,
        ));
    }

    const CREATE_NEW: &str = "__create_new__";
    let mut prompt = select(format!("Choose a `{uploader_type}` config"));
    prompt = prompt.item(
        CREATE_NEW.to_string(),
        "Create New Config".to_string(),
        "Create another named uploader config".to_string(),
    );

    let mut initial_value = None;
    for config_name in config_names {
        let value = config_name.clone();
        let hint = if Some(config_name.as_str()) == active_config {
            initial_value = Some(value.clone());
            "Active"
        } else {
            ""
        };
        prompt = prompt.item(value.clone(), value, hint);
    }
    if let Some(current) = initial_value {
        prompt = prompt.initial_value(current);
    }

    let choice = prompt.interact().map_err(io_error)?;
    if choice == CREATE_NEW {
        Ok(SelectedConfig::Use(
            prompt_config_name_menu(config_names)?,
            false,
        ))
    } else {
        Ok(SelectedConfig::Use(choice, true))
    }
}

fn prompt_config_name_menu(config_names: &[String]) -> Result<String> {
    let existing_names = config_names.to_vec();
    let mut prompt = input("Enter config name").validate(move |value: &String| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("Config name can not be empty");
        }
        if existing_names
            .iter()
            .any(|name| name.trim().eq_ignore_ascii_case(trimmed))
        {
            return Err("Config name already exists");
        }
        Ok(())
    });
    let config_name: String = prompt.interact().map_err(io_error)?;
    Ok(config_name.trim().to_string())
}

fn collect_fields_for_type_menu(
    uploader_type: &str,
    base_fields: &BTreeMap<String, toml::Value>,
) -> Result<BTreeMap<String, toml::Value>> {
    let mut out = BTreeMap::new();
    let kind = UploaderKind::from_alias(uploader_type).ok_or_else(|| {
        ZpicError::ConfigInvalid(format!(
            "uploader type '{}' is not supported for guided setup",
            uploader_type
        ))
    })?;

    for field in schema_for(kind) {
        let default = base_fields
            .get(field.key)
            .map(toml_value_to_display)
            .or_else(|| field.default.map(str::to_string));
        let value = prompt_field_menu(field, default.as_deref())?;
        if let Some(value) = value {
            out.insert(field.key.to_string(), parse_toml_value(value.trim()));
        }
    }
    Ok(out)
}

fn prompt_field_menu(field: &FieldPrompt, default: Option<&str>) -> Result<Option<String>> {
    if is_secret_field(field.key) {
        let label = if default.is_some() {
            format!("{} (leave empty to keep current value)", field.prompt)
        } else {
            field.prompt.to_string()
        };
        let mut prompt = password(label);
        if !field.required || default.is_some() {
            prompt = prompt.allow_empty();
        }
        let value = prompt.interact().map_err(io_error)?;
        if value.trim().is_empty() {
            return Ok(default.map(|value| value.to_string()));
        }
        return Ok(Some(value));
    }

    let mut prompt = input(field.prompt).required(field.required);
    if let Some(default) = default {
        prompt = prompt.default_input(default);
    }
    let value: String = prompt.interact().map_err(io_error)?;
    let value = value.trim();
    if value.is_empty() {
        Ok(default.map(|value| value.to_string()))
    } else {
        Ok(Some(value.to_string()))
    }
}

fn is_secret_field(key: &str) -> bool {
    matches!(key, "token" | "secret_access_key")
}

fn select_uploader_type<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    current_type: Option<&str>,
) -> Result<String> {
    writeln!(writer, "Available uploader types:").map_err(io_error)?;
    let kinds = UploaderKind::all();
    let default_index = current_type
        .and_then(UploaderKind::from_alias)
        .and_then(|kind| kinds.iter().position(|candidate| *candidate == kind))
        .map(|index| index + 1)
        .unwrap_or(1);
    for (index, kind) in kinds.iter().enumerate() {
        if Some(kind.as_str()) == current_type {
            writeln!(writer, "  {}. {} [Current]", index + 1, kind.as_str()).map_err(io_error)?;
        } else {
            writeln!(writer, "  {}. {}", index + 1, kind.as_str()).map_err(io_error)?;
        }
    }
    writer.flush().map_err(io_error)?;

    loop {
        let input = prompt_optional(
            reader,
            writer,
            "Choose uploader type",
            Some(&default_index.to_string()),
        )?
        .unwrap_or_else(|| default_index.to_string());
        let trimmed = input.trim();
        if let Ok(index) = trimmed.parse::<usize>() {
            if (1..=kinds.len()).contains(&index) {
                return Ok(kinds[index - 1].as_str().to_string());
            }
        }
        if let Some(kind) = UploaderKind::from_alias(trimmed) {
            return Ok(kind.as_str().to_string());
        }
        writeln!(writer, "Invalid selection. Enter a number or a type name.").map_err(io_error)?;
        writer.flush().map_err(io_error)?;
    }
}

fn select_config_name<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    uploader_type: &str,
    config_names: &[String],
    active_config: Option<&str>,
) -> Result<SelectedConfig> {
    if config_names.is_empty() {
        let config_name = prompt_non_empty(reader, writer, "Enter config name", None)?;
        return Ok(SelectedConfig::Use(config_name, false));
    }

    writeln!(writer).map_err(io_error)?;
    writeln!(writer, "Existing configs for `{}`:", uploader_type).map_err(io_error)?;
    writeln!(writer, "  1. [Create New Config]").map_err(io_error)?;
    for (index, config_name) in config_names.iter().enumerate() {
        if Some(config_name.as_str()) == active_config {
            writeln!(writer, "  {}. {} [Active]", index + 2, config_name).map_err(io_error)?;
        } else {
            writeln!(writer, "  {}. {}", index + 2, config_name).map_err(io_error)?;
        }
    }
    writer.flush().map_err(io_error)?;

    loop {
        let choice = prompt_optional(reader, writer, "Choose a config", Some("1"))?
            .unwrap_or_else(|| "1".to_string());
        let trimmed = choice.trim();
        if let Ok(index) = trimmed.parse::<usize>() {
            if index == 1 {
                let config_name = prompt_non_empty(reader, writer, "Enter config name", None)?;
                if find_config_name(config_names, &config_name).is_some() {
                    writeln!(writer, "Config name {} already exists.", config_name)
                        .map_err(io_error)?;
                    writer.flush().map_err(io_error)?;
                    continue;
                }
                return Ok(SelectedConfig::Use(config_name, false));
            }
            if (2..=(config_names.len() + 1)).contains(&index) {
                return Ok(SelectedConfig::Use(config_names[index - 2].clone(), true));
            }
        }
        if let Some(existing) = find_config_name(config_names, trimmed) {
            return Ok(SelectedConfig::Use(existing.to_string(), true));
        }
        writeln!(
            writer,
            "Invalid selection. Enter a number or an existing config name."
        )
        .map_err(io_error)?;
        writer.flush().map_err(io_error)?;
    }
}

fn collect_fields_for_type<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    uploader_type: &str,
    base_fields: &BTreeMap<String, toml::Value>,
) -> Result<BTreeMap<String, toml::Value>> {
    let mut out = BTreeMap::new();
    let kind = UploaderKind::from_alias(uploader_type).ok_or_else(|| {
        ZpicError::ConfigInvalid(format!(
            "uploader type '{}' is not supported for guided setup",
            uploader_type
        ))
    })?;

    writeln!(writer).map_err(io_error)?;
    writeln!(writer, "Setting fields for `{}`:", kind.as_str()).map_err(io_error)?;
    writer.flush().map_err(io_error)?;

    for field in schema_for(kind) {
        let default = base_fields
            .get(field.key)
            .map(toml_value_to_display)
            .or_else(|| field.default.map(str::to_string));
        let value = if field.required {
            prompt_non_empty(reader, writer, field.prompt, default.as_deref())?
        } else {
            match prompt_optional(reader, writer, field.prompt, default.as_deref())? {
                Some(value) => value,
                None => continue,
            }
        };
        out.insert(field.key.to_string(), parse_toml_value(value.trim()));
    }
    Ok(out)
}

fn schema_for(kind: UploaderKind) -> &'static [FieldPrompt] {
    match kind {
        UploaderKind::Local => LOCAL_FIELDS,
        UploaderKind::Github => GITHUB_FIELDS,
        UploaderKind::S3 => S3_FIELDS,
    }
}

fn resolve_base_fields(
    config: &zpic_config::LoadedConfig,
    uploader_type: &str,
    existing_name: Option<&str>,
    from_name: Option<&str>,
) -> Result<BTreeMap<String, toml::Value>> {
    if let Some(from_name) = from_name {
        if let Some(item) = find_config(config, uploader_type, from_name) {
            return Ok(item.fields.clone());
        }
        return Err(ZpicError::ConfigInvalid(format!(
            "config '{}' not found in type '{}'",
            from_name, uploader_type
        )));
    }
    Ok(existing_name
        .and_then(|name| find_config(config, uploader_type, name))
        .map(|item| item.fields.clone())
        .unwrap_or_default())
}

fn find_config<'a>(
    config: &'a zpic_config::LoadedConfig,
    uploader_type: &str,
    config_name: &str,
) -> Option<&'a UploaderConfigItem> {
    config
        .zpic
        .uploader
        .get(uploader_type)
        .and_then(|store| store.find_by_name(config_name))
}

fn config_names_for_type(config: &zpic_config::LoadedConfig, uploader_type: &str) -> Vec<String> {
    config
        .zpic
        .uploader
        .get(uploader_type)
        .map(|store| {
            store
                .config_list
                .iter()
                .map(|item| item.config_name.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn active_config_name_for_type<'a>(
    config: &'a zpic_config::LoadedConfig,
    uploader_type: &str,
) -> Option<&'a str> {
    config
        .zpic
        .uploader
        .get(uploader_type)
        .and_then(|store| store.active())
        .map(|item| item.config_name.as_str())
}

fn find_config_name<'a>(config_names: &'a [String], target: &str) -> Option<&'a str> {
    let normalized = target.trim();
    config_names
        .iter()
        .find(|name| name.trim().eq_ignore_ascii_case(normalized))
        .map(|name| name.as_str())
}

fn toml_value_to_display(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => value.clone(),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Datetime(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn prompt_non_empty<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: Option<&str>,
) -> Result<String> {
    loop {
        if let Some(value) = prompt_optional(reader, writer, label, default)? {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
        writeln!(writer, "{} can not be empty.", label).map_err(io_error)?;
        writer.flush().map_err(io_error)?;
    }
}

fn prompt_optional<R: io::BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: Option<&str>,
) -> Result<Option<String>> {
    match default {
        Some(default) => write!(writer, "{} [{}]: ", label, default).map_err(io_error)?,
        None => write!(writer, "{}: ", label).map_err(io_error)?,
    }
    writer.flush().map_err(io_error)?;

    let mut buffer = String::new();
    let read = reader.read_line(&mut buffer).map_err(io_error)?;
    if read == 0 {
        return Err(ZpicError::InvalidArgument(
            "interactive input was interrupted".into(),
        ));
    }
    let value = buffer.trim().to_string();
    if value.is_empty() {
        return Ok(default.map(|value| value.to_string()));
    }
    Ok(Some(value))
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
        out.insert(normalize_field_key(key), parse_toml_value(raw_value.trim()));
    }
    Ok(out)
}

fn normalize_field_key(key: &str) -> String {
    match key.trim() {
        "customUrl" => "public_base_url".to_string(),
        "path" => "path_prefix".to_string(),
        other => other.to_string(),
    }
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

fn io_error(err: io::Error) -> ZpicError {
    ZpicError::Io(err)
}

#[derive(Debug)]
struct ResolvedSetArgs {
    uploader_type: String,
    config_name: String,
    from: Option<String>,
    base_fields: BTreeMap<String, toml::Value>,
    fields: BTreeMap<String, toml::Value>,
}

#[derive(Debug)]
enum SelectedConfig {
    Use(String, bool),
}

impl SelectedConfig {
    fn name(&self) -> &str {
        match self {
            SelectedConfig::Use(name, _) => name.as_str(),
        }
    }
}

#[derive(Debug)]
struct FieldPrompt {
    key: &'static str,
    prompt: &'static str,
    required: bool,
    default: Option<&'static str>,
}

impl FieldPrompt {
    const fn required(
        key: &'static str,
        prompt: &'static str,
        default: Option<&'static str>,
    ) -> Self {
        Self {
            key,
            prompt,
            required: true,
            default,
        }
    }

    const fn optional(
        key: &'static str,
        prompt: &'static str,
        default: Option<&'static str>,
    ) -> Self {
        Self {
            key,
            prompt,
            required: false,
            default,
        }
    }
}

const LOCAL_FIELDS: &[FieldPrompt] = &[
    FieldPrompt::required("target_dir", "Target directory", None),
    FieldPrompt::required("public_base_url", "Public base URL", None),
];

const GITHUB_FIELDS: &[FieldPrompt] = &[
    FieldPrompt::required("repo", "GitHub repo (owner/repo)", None),
    FieldPrompt::required("branch", "Branch", Some("master")),
    FieldPrompt::required("token", "GitHub token", None),
    FieldPrompt::optional("path_prefix", "Path prefix", None),
    FieldPrompt::optional("public_base_url", "Custom public base URL", None),
];

const S3_FIELDS: &[FieldPrompt] = &[
    FieldPrompt::required("endpoint", "S3 endpoint", None),
    FieldPrompt::optional("region", "Region", Some("auto")),
    FieldPrompt::required("bucket", "Bucket", None),
    FieldPrompt::required("access_key_id", "Access key ID", None),
    FieldPrompt::required("secret_access_key", "Secret access key", None),
    FieldPrompt::required("public_base_url", "Public base URL", None),
    FieldPrompt::optional("cache_control", "Cache-Control", None),
    FieldPrompt::optional("acl", "ACL", None),
];

#[derive(Debug, Serialize)]
struct SetPayload {
    action: &'static str,
    #[serde(rename = "type")]
    uploader_type: String,
    active_config: String,
    inherited_from: Option<String>,
    saved_to: String,
}
