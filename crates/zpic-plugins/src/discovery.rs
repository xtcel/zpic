use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Serialize;

use crate::manifest::PluginManifest;
use crate::registry::UploaderDescriptor;
use crate::runtime::WasmPluginRunner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginDiagnosticLevel {
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginDiagnostic {
    pub level: PluginDiagnosticLevel,
    pub path: String,
    pub plugin_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryOptions {
    pub search_paths: Option<Vec<PathBuf>>,
}

fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("io", "zpic", "zpic")
        .expect("operating system provides a home directory")
}

pub fn plugin_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(raw) = std::env::var_os("ZPIC_PLUGIN_DIRS") {
        paths.extend(std::env::split_paths(&raw));
    }
    if let Some(cwd) = std::env::current_dir().ok() {
        paths.push(cwd.join(".zpic").join("plugins"));
    }
    paths.push(project_dirs().config_dir().join("plugins"));

    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing: &PathBuf| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}

pub fn discover_plugin_descriptors(
    options: DiscoveryOptions,
) -> (Vec<UploaderDescriptor>, Vec<PluginDiagnostic>) {
    let mut descriptors = Vec::new();
    let mut diagnostics = Vec::new();
    let search_paths = options.search_paths.unwrap_or_else(plugin_search_paths);

    for root in search_paths {
        if !root.exists() {
            continue;
        }
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(err) => {
                diagnostics.push(PluginDiagnostic {
                    level: PluginDiagnosticLevel::Warn,
                    path: root.display().to_string(),
                    plugin_id: None,
                    message: format!("could not read plugin directory: {err}"),
                });
                continue;
            }
        };
        for entry in entries.flatten() {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }
            match load_plugin_directory(&plugin_dir) {
                Ok(mut found) => descriptors.append(&mut found),
                Err((plugin_id, message)) => diagnostics.push(PluginDiagnostic {
                    level: PluginDiagnosticLevel::Fail,
                    path: plugin_dir.display().to_string(),
                    plugin_id,
                    message,
                }),
            }
        }
    }

    (descriptors, diagnostics)
}

fn load_plugin_directory(plugin_dir: &Path) -> std::result::Result<Vec<UploaderDescriptor>, (Option<String>, String)> {
    let manifest_path = plugin_dir.join("plugin.toml");
    let manifest_text = fs::read_to_string(&manifest_path).map_err(|err| {
        (
            None,
            format!("missing or unreadable manifest at {}: {err}", manifest_path.display()),
        )
    })?;
    let manifest = PluginManifest::from_toml(&manifest_text).map_err(|err| {
        (
            None,
            format!("invalid plugin manifest at {}: {err}", manifest_path.display()),
        )
    })?;
    if manifest.uploaders.is_empty() {
        return Err((
            Some(manifest.id.clone()),
            "plugin does not declare any uploaders".into(),
        ));
    }
    let wasm_path = plugin_dir.join(&manifest.wasm_file);
    if !wasm_path.exists() {
        return Err((
            Some(manifest.id.clone()),
            format!("referenced WASM module not found: {}", wasm_path.display()),
        ));
    }

    let mut descriptors = Vec::new();
    for uploader in manifest.uploaders {
        let display_name = uploader.display_name().to_string();
        let type_name = uploader.type_name.clone();
        let aliases = uploader.picgo_aliases.clone();
        let fields = uploader.fields.clone();
        let runner = WasmPluginRunner::new(manifest.id.clone(), wasm_path.clone()).map_err(|err| {
            (
                Some(manifest.id.clone()),
                format!("could not initialize WASM runtime: {err}"),
            )
        })?;
        descriptors.push(runner.into_descriptor(type_name, display_name, aliases, fields));
    }

    Ok(descriptors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_plugin_descriptors_from_search_path() {
        let dir = tempdir().unwrap();
        let plugin_dir = dir.path().join("demo");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.toml"),
            r#"
id = "demo"
name = "Demo"
version = "0.1.0"

[[uploaders]]
type = "demo-uploader"
display_name = "Demo Uploader"
"#,
        )
        .unwrap();
        fs::write(
            plugin_dir.join("plugin.wasm"),
            r#"(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 2048))
  (func (export "zpic_alloc") (param $len i32) (result i32)
    (local $ptr i32)
    global.get $heap
    local.set $ptr
    global.get $heap
    local.get $len
    i32.add
    global.set $heap
    local.get $ptr)
  (func (export "zpic_upload") (param $ptr i32) (param $len i32) (result i64)
    i64.const 38654705664)
  (data (i32.const 0) "{\"url\":\"https://demo.invalid/example.png\"}")
)"#,
        )
        .unwrap();

        let (descriptors, diagnostics) = discover_plugin_descriptors(DiscoveryOptions {
            search_paths: Some(vec![dir.path().to_path_buf()]),
        });

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].type_name, "demo-uploader");
    }
}
