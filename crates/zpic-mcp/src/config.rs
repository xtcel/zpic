//! Security-relevant configuration for the MCP server.
//!
//! Every default here is deliberately conservative: the server should be
//! safe to point an AI agent at without any setup. See
//! `crates/zpic-mcp/README.md` for the full field reference.

use std::path::PathBuf;

const DEFAULT_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];
const DEFAULT_MAX_FILE_SIZE_MB: u64 = 20;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct McpConfig {
    /// Absolute directories the server is allowed to read files from.
    /// Defaults to the current working directory the server was launched
    /// in (typically the agent's project root).
    pub workspace_roots: Vec<PathBuf>,
    /// Allow the `upload_clipboard_image` tool. Off by default: clipboard
    /// contents are outside `workspace_roots` and an agent has no way to
    /// know what's on it.
    pub allow_clipboard: bool,
    /// Allow `migrate_markdown_images` to actually upload and rewrite
    /// files. Off by default: the tool always runs as a dry-run report
    /// until this is set.
    pub allow_migrate_write: bool,
    /// Reject files larger than this before shelling out to `zpic`.
    pub max_file_size_mb: u64,
    /// Case-insensitive file extensions `upload_image` will accept.
    pub allowed_extensions: Vec<String>,
    /// The `zpic` binary to invoke. Defaults to resolving `zpic` from
    /// `PATH`; override with an absolute path if it isn't installed
    /// globally.
    pub zpic_bin: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            workspace_roots: vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))],
            allow_clipboard: false,
            allow_migrate_write: false,
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            allowed_extensions: DEFAULT_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
            zpic_bin: "zpic".to_string(),
        }
    }
}

impl McpConfig {
    /// Load from `ZPIC_MCP_CONFIG`, if set, otherwise from the user config
    /// directory (`<config dir>/zpic/mcp.toml`). Falls back to
    /// [`McpConfig::default`] when no file is present so the server works
    /// with zero configuration.
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var_os("ZPIC_MCP_CONFIG")
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("", "", "zpic")
                    .map(|d| d.config_dir().join("mcp.toml"))
            });

        let Some(path) = path else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        let config: Self = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("failed to parse {}: {e}", path.display()))?;
        Ok(config)
    }
}
