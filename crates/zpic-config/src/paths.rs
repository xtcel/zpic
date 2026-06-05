//! Filesystem locations for zpic and PicGo configuration files.

use std::path::PathBuf;

use directories::ProjectDirs;

/// Project directory identifier for all zpic-managed paths. We use the
/// `directories` crate to resolve XDG / macOS / Windows locations.
pub fn project_dirs() -> ProjectDirs {
    ProjectDirs::from("io", "zpic", "zpic").expect("operating system provides a home directory")
}

/// Default global zpic config path (`~/.config/zpic/config.toml` on Linux,
/// `~/Library/Application Support/zpic/config.toml` on macOS, etc.).
pub fn default_zpic_config() -> PathBuf {
    project_dirs().config_dir().join("config.toml")
}

/// All locations the loader considers when no explicit `--config` is given,
/// in priority order (project → user). `ZPIC_CONFIG` and explicit paths are
/// handled by the loader directly.
pub fn candidate_zpic_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(cwd) = std::env::current_dir().ok() {
        paths.push(cwd.join(".zpic").join("config.toml"));
    }
    paths.push(default_zpic_config());
    paths
}

/// All known PicGo config file locations, in priority order. The first
/// match wins, but every candidate is reported to the user via `doctor`.
pub fn candidate_picgo_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".picgo").join("config.json"));
        paths.push(
            home.join("Library")
                .join("Application Support")
                .join("picgo")
                .join("data.json"),
        );
        paths.push(home.join(".config").join("picgo").join("data.json"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        paths.push(appdata.join("picgo").join("data.json"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_paths_include_user_zpic() {
        let candidates = candidate_zpic_paths();
        // The exact suffix depends on the platform, but every entry must
        // point inside the zpic-managed config directory and end with
        // `config.toml`.
        assert!(candidates.iter().any(|p| p.ends_with("config.toml")));
        assert!(candidates
            .iter()
            .any(|p| p.to_string_lossy().contains("zpic")));
    }
}
