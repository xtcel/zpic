//! `zpic config` — initialize, show, and import-picgo.

use std::path::Path;
use std::path::PathBuf;

use crate::cli::ConfigAction;
use crate::util::load_config;
use zpic_config::loader::ConfigLoader;
use zpic_config::paths::{candidate_picgo_paths, default_zpic_config};
use zpic_config::ZpicConfigFile;
use zpic_core::error::{Result, ZpicError};

pub fn run(action: ConfigAction, explicit_config: Option<PathBuf>, _json: bool) -> Result<i32> {
    match action {
        ConfigAction::Init { force } => cmd_init(force),
        ConfigAction::Show => cmd_show(explicit_config.as_deref()),
        ConfigAction::ImportPicgo { from, to } => cmd_import_picgo(from, to),
    }
}

fn cmd_init(force: bool) -> Result<i32> {
    let dest = default_zpic_config();
    if dest.exists() && !force {
        return Err(ZpicError::ConfigInvalid(format!(
            "config already exists at {}; pass --force to overwrite",
            dest.display()
        )));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let cfg = ZpicConfigFile::default();
    let text = toml::to_string_pretty(&cfg).map_err(|e| ZpicError::Internal(e.to_string()))?;
    std::fs::write(&dest, text)?;
    println!("wrote starter config to {}", dest.display());
    Ok(0)
}

fn cmd_show(explicit: Option<&Path>) -> Result<i32> {
    let config = load_config(explicit)?;
    println!(
        "# Source: {} ({})",
        config.source.label(),
        config.source.path().display()
    );
    println!();
    println!("{}", config.zpic.redacted_toml());
    Ok(0)
}

fn cmd_import_picgo(from: Option<PathBuf>, to: Option<PathBuf>) -> Result<i32> {
    let source = from
        .or_else(|| candidate_picgo_paths().into_iter().find(|p| p.exists()))
        .ok_or_else(|| ZpicError::ConfigNotFound)?;
    let dest = to.unwrap_or_else(default_zpic_config);
    if dest.exists() {
        return Err(ZpicError::ConfigInvalid(format!(
            "refusing to overwrite existing config at {}; pass --to to choose a different path",
            dest.display()
        )));
    }
    let cfg = ConfigLoader::import_picgo(&source, &dest)?;
    println!("imported PicGo config from {}", source.display());
    println!("wrote zpic config to {}", dest.display());
    println!();
    let active_type = cfg.active_uploader_type().unwrap_or("<none>");
    let active_config = cfg
        .active_uploader_type()
        .and_then(|uploader_type| cfg.uploader.get(uploader_type))
        .and_then(|store| store.active())
        .map(|item| item.config_name.as_str())
        .unwrap_or("<none>");
    println!("active uploader: {active_type}");
    println!("active config: {active_config}");
    Ok(0)
}
