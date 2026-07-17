//! MCP tool surface. Every tool shells out to the `zpic` binary with
//! `--json` rather than linking `zpic-cli`'s command modules directly:
//! those modules `println!` their output, and this process's stdout is
//! the MCP JSON-RPC stream — anything stray on stdout would corrupt the
//! protocol. Spawning `zpic` as a child process keeps its stdout
//! separate and captured, matching the pattern already documented for
//! the Zed adapter in `docs/cli-contract.md`.

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::process::Command;

use crate::config::McpConfig;

#[derive(Clone)]
pub struct ZpicMcpServer {
    config: McpConfig,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadImageParams {
    /// Path to a local image file, absolute or relative to the server's
    /// working directory. Must resolve inside a configured workspace root.
    pub path: String,
    /// Uploader type override (e.g. "github", "s3", "local"). Defaults to
    /// the active uploader from zpic's config.
    #[serde(default)]
    pub uploader: Option<String>,
    /// Output format: "markdown" (default), "url", "html", or "jsx".
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadClipboardParams {
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MigrateParams {
    /// Path to a Markdown file or directory, absolute or relative to the
    /// server's working directory.
    pub path: String,
    /// Report changes without uploading or rewriting anything. Ignored
    /// (forced true) unless the server config sets allow_migrate_write.
    #[serde(default)]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HistoryParams {
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[tool_router]
impl ZpicMcpServer {
    pub fn new(config: McpConfig) -> Self {
        Self { config }
    }

    #[tool(
        description = "Upload a local image file through zpic's active uploader. Returns zpic's upload JSON payload (url, markdown, key, mime, size, width, height)."
    )]
    async fn upload_image(
        &self,
        Parameters(p): Parameters<UploadImageParams>,
    ) -> Result<String, McpError> {
        let path = self.resolve_image_path(&p.path)?;
        let mut args = vec!["upload".to_string(), path_arg(&path), "--json".to_string()];
        push_opt(&mut args, "--uploader", p.uploader);
        push_opt(&mut args, "--format", p.format);
        self.run_zpic(&args).await
    }

    #[tool(
        description = "Upload the current system clipboard image through zpic. Disabled by default; the server config must set allow_clipboard = true."
    )]
    async fn upload_clipboard_image(
        &self,
        Parameters(p): Parameters<UploadClipboardParams>,
    ) -> Result<String, McpError> {
        if !self.config.allow_clipboard {
            return Err(McpError::invalid_request(
                "clipboard uploads are disabled; set `allow_clipboard = true` in the zpic-mcp config to enable them",
                None,
            ));
        }
        let mut args = vec![
            "upload".to_string(),
            "--clipboard".to_string(),
            "--json".to_string(),
        ];
        push_opt(&mut args, "--uploader", p.uploader);
        push_opt(&mut args, "--format", p.format);
        self.run_zpic(&args).await
    }

    #[tool(
        description = "Scan a Markdown file or directory for local image references and rewrite them to remote URLs. Runs as a dry-run report unless the server config sets allow_migrate_write = true."
    )]
    async fn migrate_markdown_images(
        &self,
        Parameters(p): Parameters<MigrateParams>,
    ) -> Result<String, McpError> {
        let path = self.resolve_within_roots(&p.path)?;
        let effective_dry_run = if self.config.allow_migrate_write {
            p.dry_run.unwrap_or(false)
        } else {
            true
        };
        let mut args = vec!["migrate".to_string(), path_arg(&path), "--json".to_string()];
        if effective_dry_run {
            args.push("--dry-run".to_string());
        }
        self.run_zpic(&args).await
    }

    #[tool(description = "List previously recorded uploads from zpic's local history store.")]
    async fn list_upload_history(
        &self,
        Parameters(p): Parameters<HistoryParams>,
    ) -> Result<String, McpError> {
        let mut args = vec![
            "history".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ];
        push_opt(&mut args, "--uploader", p.uploader);
        push_opt(&mut args, "--limit", p.limit.map(|n| n.to_string()));
        self.run_zpic(&args).await
    }

    #[tool(
        description = "List configured uploader types and named configs, and which one is active. Never includes credentials."
    )]
    async fn list_uploaders(&self) -> Result<String, McpError> {
        self.run_zpic(&[
            "uploader".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ])
        .await
    }

    #[tool(
        description = "Run zpic's local diagnostic checks: config discovery, active uploader credentials, clipboard availability, history store health."
    )]
    async fn doctor(&self) -> Result<String, McpError> {
        self.run_zpic(&["doctor".to_string(), "--json".to_string()])
            .await
    }

    /// Canonicalize `input` and confirm it falls inside a configured
    /// workspace root. Used for any path a tool will hand to `zpic`.
    fn resolve_within_roots(&self, input: &str) -> Result<PathBuf, McpError> {
        let candidate = PathBuf::from(input);
        let absolute = if candidate.is_absolute() {
            candidate
        } else {
            std::env::current_dir()
                .map_err(|e| McpError::internal_error(format!("cannot read cwd: {e}"), None))?
                .join(candidate)
        };
        let canonical = absolute.canonicalize().map_err(|e| {
            McpError::invalid_params(format!("cannot resolve path `{input}`: {e}"), None)
        })?;

        let allowed = self.config.workspace_roots.iter().any(|root| {
            root.canonicalize()
                .map(|r| canonical.starts_with(r))
                .unwrap_or(false)
        });
        if !allowed {
            let roots = self
                .config
                .workspace_roots
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(McpError::invalid_params(
                format!(
                    "`{}` is outside the allowed workspace roots ({roots})",
                    canonical.display()
                ),
                None,
            ));
        }
        Ok(canonical)
    }

    /// Like [`Self::resolve_within_roots`] but also enforces the image
    /// extension allowlist and `max_file_size_mb`.
    fn resolve_image_path(&self, input: &str) -> Result<PathBuf, McpError> {
        let canonical = self.resolve_within_roots(input)?;

        let ext = canonical
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !self
            .config
            .allowed_extensions
            .iter()
            .any(|a| a.eq_ignore_ascii_case(ext))
        {
            return Err(McpError::invalid_params(
                format!(
                    "extension `.{ext}` is not in allowed_extensions ({})",
                    self.config.allowed_extensions.join(", ")
                ),
                None,
            ));
        }

        let meta = std::fs::metadata(&canonical).map_err(|e| {
            McpError::invalid_params(format!("cannot stat `{}`: {e}", canonical.display()), None)
        })?;
        let max_bytes = self.config.max_file_size_mb.saturating_mul(1024 * 1024);
        if meta.len() > max_bytes {
            return Err(McpError::invalid_params(
                format!(
                    "`{}` is {} bytes, exceeds max_file_size_mb={}",
                    canonical.display(),
                    meta.len(),
                    self.config.max_file_size_mb
                ),
                None,
            ));
        }
        Ok(canonical)
    }

    /// Run `zpic <args>`, returning its stdout verbatim (the JSON payload
    /// documented in `docs/cli-contract.md`). Every invocation is logged
    /// to stderr for audit purposes; nothing from `zpic` or this function
    /// ever touches our own stdout.
    async fn run_zpic(&self, args: &[String]) -> Result<String, McpError> {
        tracing::info!(target: "zpic_mcp::audit", bin = %self.config.zpic_bin, args = ?args, "invoking zpic");

        let output = Command::new(&self.config.zpic_bin)
            .args(args)
            .output()
            .await
            .map_err(|e| {
                McpError::internal_error(
                    format!("failed to launch `{}`: {e}", self.config.zpic_bin),
                    None,
                )
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        tracing::info!(
            target: "zpic_mcp::audit",
            status = output.status.code(),
            stderr = %stderr,
            "zpic exited"
        );

        if stdout.trim().is_empty() {
            return Err(McpError::internal_error(
                format!(
                    "`{} {}` produced no output (exit {:?}): {stderr}",
                    self.config.zpic_bin,
                    args.join(" "),
                    output.status.code()
                ),
                None,
            ));
        }
        Ok(stdout)
    }
}

#[tool_handler(
    name = "zpic-mcp",
    instructions = "zpic uploads images to configured hosts (local disk, GitHub, S3-compatible storage, Aliyun OSS) and returns their remote URL / Markdown / HTML. File-taking tools only accept paths inside this server's configured workspace_roots, under max_file_size_mb, with an allowed extension. upload_clipboard_image and migrate_markdown_images' write mode are disabled by default; ask the user before enabling them. Tool results are the raw JSON zpic prints on stdout (see docs/cli-contract.md) — check the top-level `success` field, since a non-zero exit still returns a payload describing which items failed."
)]
impl ServerHandler for ZpicMcpServer {}

fn path_arg(path: &std::path::Path) -> String {
    path.display().to_string()
}

fn push_opt(args: &mut Vec<String>, flag: &str, value: Option<String>) {
    if let Some(v) = value {
        args.push(flag.to_string());
        args.push(v);
    }
}
