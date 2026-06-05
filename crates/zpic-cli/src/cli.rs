//! Top-level CLI definition and dispatcher.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::commands::{config, doctor, history, migrate, set_cmd, upload, uploader, use_cmd, zed};
use zpic_core::error::ZpicError;

#[derive(Debug, Parser)]
#[command(
    name = "zpic",
    version,
    about = "Rust-native image hosting CLI compatible with PicGo",
    long_about = None
)]
pub struct Cli {
    /// Path to a config file (overrides all other sources).
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Emit machine-readable JSON to stdout for `upload`, `migrate`, `history`, and `doctor`.
    #[arg(long, global = true)]
    pub json: bool,

    /// Verbose logging; can be repeated.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Upload one or more image files (or the clipboard) to the active uploader.
    #[command(alias = "u")]
    Upload(UploadArgs),
    /// Rewrite local image references in a Markdown file or directory to remote URLs.
    Migrate(MigrateArgs),
    /// Inspect, initialize, or import configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Browse, copy, or delete past uploads.
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },
    /// Manage named uploader configurations.
    Uploader {
        #[command(subcommand)]
        action: UploaderAction,
    },
    /// Activate a module selection.
    Use {
        #[command(subcommand)]
        action: UseAction,
    },
    /// Create or update module configuration.
    Set {
        #[command(subcommand)]
        action: SetAction,
    },
    /// Run diagnostic checks for config, credentials, clipboard, and the history store.
    Doctor(DoctorArgs),
    /// Scaffold Zed editor integration into the current project.
    Zed {
        #[command(subcommand)]
        action: ZedAction,
    },
    /// Print version information.
    Version,
}

#[derive(Debug, Args)]
pub struct UploadArgs {
    /// Image files to upload. Ignored when `--clipboard` is set.
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// Read image data from the system clipboard instead of `files`.
    #[arg(long)]
    pub clipboard: bool,

    /// Override the active uploader type.
    #[arg(long, value_name = "TYPE")]
    pub uploader: Option<String>,

    /// Override the configured default output format (markdown, url, html, jsx).
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<String>,

    /// Override the alt text used by the markdown/html formatter.
    #[arg(long)]
    pub alt: Option<String>,

    /// Override the file name used in the rendered markdown link.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Copy the rendered output to the system clipboard on success.
    #[arg(long)]
    pub copy: bool,

    /// Validate the upload pipeline without writing any files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// Markdown file or directory to scan.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Recurse into directories.
    #[arg(long, short = 'r')]
    pub recursive: bool,

    /// Report what would change without uploading or rewriting anything.
    #[arg(long)]
    pub dry_run: bool,

    /// Write a structured JSON report to this path.
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,

    /// Upload only local image references.
    #[arg(long)]
    pub local_only: bool,

    /// Leave remote image references alone in the rewritten file.
    #[arg(long)]
    pub ignore_remote: bool,

    /// Override the active uploader type.
    #[arg(long, value_name = "TYPE")]
    pub uploader: Option<String>,

    /// Override the configured default output format.
    #[arg(long, value_name = "FORMAT")]
    pub format: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Create a starter zpic config at the default location.
    Init {
        /// Overwrite an existing file at the destination.
        #[arg(long)]
        force: bool,
    },
    /// Print the resolved config (with secrets redacted) to stdout.
    Show,
    /// Convert a PicGo config into a native zpic TOML file.
    ImportPicgo {
        /// Source PicGo config path (defaults to `~/.picgo/config.json`).
        #[arg(long, value_name = "FILE")]
        from: Option<PathBuf>,
        /// Destination zpic config path (defaults to the user config dir).
        #[arg(long, value_name = "FILE")]
        to: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum HistoryAction {
    /// List past uploads.
    List {
        /// Filter by uploader name (e.g. `r2`, `github`, `local`).
        #[arg(long)]
        uploader: Option<String>,
        /// Limit the number of rows returned.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Delete a single history entry.
    Delete {
        /// Entry id (UUID).
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum UploaderAction {
    /// List configured uploader types or configs for one type.
    List {
        #[arg(value_name = "TYPE")]
        uploader_type: Option<String>,
    },
    /// Rename a named config within one uploader type.
    Rename {
        #[arg(value_name = "TYPE")]
        uploader_type: String,
        #[arg(value_name = "OLD_NAME")]
        old_name: String,
        #[arg(value_name = "NEW_NAME")]
        new_name: String,
    },
    /// Copy a named config without activating the copy.
    Copy {
        #[arg(value_name = "TYPE")]
        uploader_type: String,
        #[arg(value_name = "CONFIG_NAME")]
        config_name: String,
        #[arg(value_name = "NEW_CONFIG_NAME")]
        new_config_name: String,
    },
    /// Remove a named config from one uploader type.
    Rm {
        #[arg(value_name = "TYPE")]
        uploader_type: String,
        #[arg(value_name = "CONFIG_NAME")]
        config_name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum UseAction {
    /// Activate an uploader type and, optionally, a named config.
    Uploader {
        #[arg(value_name = "TYPE")]
        uploader_type: String,
        #[arg(value_name = "CONFIG_NAME")]
        config_name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SetAction {
    /// Create or update a named uploader config and make it active.
    Uploader(SetUploaderArgs),
}

#[derive(Debug, Args)]
pub struct SetUploaderArgs {
    #[arg(value_name = "TYPE")]
    pub uploader_type: Option<String>,
    #[arg(value_name = "CONFIG_NAME")]
    pub config_name: Option<String>,
    /// Seed the new config from an existing config in the same uploader type.
    #[arg(long, value_name = "NAME")]
    pub from: Option<String>,
    /// Set one config field. Repeat for multiple fields.
    #[arg(long = "field", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
    pub fields: Vec<String>,
}

#[derive(Debug, Args, Default)]
pub struct DoctorArgs {}

#[derive(Debug, Subcommand)]
pub enum ZedAction {
    /// Create project-local `.zed` tasks and helper scripts for zpic.
    Init(ZedInitArgs),
}

#[derive(Debug, Args)]
pub struct ZedInitArgs {
    /// Project root where `.zed` should be written. Defaults to the current directory.
    #[arg(long, value_name = "DIR")]
    pub project_root: Option<PathBuf>,
    /// Override the zpic binary path written into generated task env vars.
    #[arg(long, value_name = "PATH")]
    pub zpic_bin: Option<PathBuf>,
    /// Overwrite previously generated files.
    #[arg(long)]
    pub force: bool,
}

impl Cli {
    /// Parse argv using `clap::Parser`.
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

/// Top-level entry point used by `main`. Returns an exit code; `Ok(0)` means
/// success.
pub async fn run(cli: Cli) -> Result<i32, i32> {
    init_logging(cli.verbose);
    let json = cli.json;
    let config_path = cli.config.clone();
    let res: zpic_core::error::Result<i32> = match cli.command {
        Command::Upload(args) => upload::run(args, config_path, json).await,
        Command::Migrate(args) => migrate::run(args, config_path, json).await,
        Command::Config { action } => config::run(action, config_path, json),
        Command::History { action } => history::run(action, config_path, json),
        Command::Uploader { action } => uploader::run(action, config_path, json),
        Command::Use { action } => use_cmd::run(action, config_path, json),
        Command::Set { action } => set_cmd::run(action, config_path, json),
        Command::Doctor(_) => doctor::run(config_path, json),
        Command::Zed { action } => zed::run(action, json),
        Command::Version => {
            println!("zpic {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
    };
    match res {
        Ok(code) => Ok(code),
        Err(err) => {
            if json {
                let payload = serde_json::json!({
                    "success": false,
                    "error": err.to_string(),
                    "remediation": err.remediation(),
                });
                println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            } else {
                eprintln!("error: {err}");
                if let Some(fix) = err.remediation() {
                    eprintln!("\nfix:\n  {fix}");
                }
                if matches!(err, ZpicError::ConfigNotFound) {
                    eprintln!("\nrun `zpic doctor` for a full diagnostic.");
                }
            }
            Err(1)
        }
    }
}

fn init_logging(verbose: u8) {
    let filter = match verbose {
        0 => "zpic=warn",
        1 => "zpic=info",
        _ => "zpic=debug",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
