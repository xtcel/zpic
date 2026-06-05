//! `zpic zed` — scaffold project-local Zed integration files.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::cli::{ZedAction, ZedInitArgs};
use zpic_core::error::{Result, ZpicError};

const POSIX_UPLOAD_SCRIPT: &str = include_str!("../../templates/zed/zpic-upload-from-clipboard.sh");
const POSIX_MIGRATE_SCRIPT: &str = include_str!("../../templates/zed/zpic-migrate-current-file.sh");
const POWERSHELL_UPLOAD_SCRIPT: &str =
    include_str!("../../templates/zed/zpic-upload-from-clipboard.ps1");
const POWERSHELL_MIGRATE_SCRIPT: &str =
    include_str!("../../templates/zed/zpic-migrate-current-file.ps1");

pub fn run(action: ZedAction, json: bool) -> Result<i32> {
    match action {
        ZedAction::Init(args) => cmd_init(args, json),
    }
}

fn cmd_init(args: ZedInitArgs, json: bool) -> Result<i32> {
    let project_root = args.project_root.unwrap_or(std::env::current_dir()?);
    if !project_root.exists() {
        return Err(ZpicError::InvalidArgument(format!(
            "project root does not exist: {}",
            project_root.display()
        )));
    }
    if !project_root.is_dir() {
        return Err(ZpicError::InvalidArgument(format!(
            "project root is not a directory: {}",
            project_root.display()
        )));
    }

    let shell = ShellKind::current();
    let zed_dir = project_root.join(".zed");
    let files = shell.files(&zed_dir);
    let mut planned_paths = vec![
        files.tasks.clone(),
        files.keymap_example.clone(),
        files.readme.clone(),
        files.upload_script.clone(),
        files.migrate_script.clone(),
    ];
    planned_paths.sort();

    if !args.force {
        for path in &planned_paths {
            if path.exists() {
                return Err(ZpicError::ConfigInvalid(format!(
                    "refusing to overwrite existing file {}; pass --force to overwrite zpic-managed Zed files",
                    path.display()
                )));
            }
        }
    }

    fs::create_dir_all(&zed_dir)?;

    let task_env = args
        .zpic_bin
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    write_text(&files.tasks, &render_tasks(shell, task_env.as_deref())?)?;
    write_text(&files.keymap_example, &render_keymap_example()?)?;
    write_text(&files.readme, &render_readme(shell)?)?;
    write_text(&files.upload_script, shell.upload_script())?;
    write_text(&files.migrate_script, shell.migrate_script())?;

    #[cfg(unix)]
    {
        set_executable(&files.upload_script)?;
        set_executable(&files.migrate_script)?;
    }

    let payload = ZedInitPayload {
        action: "init",
        project_root: project_root.display().to_string(),
        shell: shell.as_str(),
        created: planned_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&payload)
                .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else {
        println!("wrote Zed integration files to {}", zed_dir.display());
        for path in &payload.created {
            println!("- {}", path);
        }
        println!();
        println!("next steps:");
        println!("1. Open this project in Zed.");
        println!("2. Run `task: spawn` and choose `zpic: upload clipboard as markdown`.");
        println!("3. Copy `.zed/zpic-keymap.json.example` into your global Zed keymap if you want shortcuts.");
    }

    Ok(0)
}

fn render_tasks(shell: ShellKind, zpic_bin: Option<&str>) -> Result<String> {
    let zpic_command = zpic_bin.unwrap_or("zpic");
    let mut upload_markdown = shell.upload_task("zpic: upload clipboard as markdown", "markdown");
    let mut upload_url = shell.upload_task("zpic: upload clipboard as url", "url");
    let mut migrate = shell.migrate_task();
    let mut doctor = simple_task("zpic: doctor", zpic_command, &["doctor"]);
    let mut uploader_list = simple_task("zpic: uploader list", zpic_command, &["uploader", "list"]);

    for task in [
        &mut upload_markdown,
        &mut upload_url,
        &mut migrate,
        &mut doctor,
        &mut uploader_list,
    ] {
        if let Some(path) = zpic_bin {
            task.insert("env".into(), json!({ "ZPIC_BIN": path }));
        }
    }

    let tasks = Value::Array(vec![
        Value::Object(upload_markdown),
        Value::Object(upload_url),
        Value::Object(migrate),
        Value::Object(doctor),
        Value::Object(uploader_list),
    ]);
    serde_json::to_string_pretty(&tasks).map_err(|e| ZpicError::Internal(e.to_string()))
}

fn render_keymap_example() -> Result<String> {
    let keymap = json!([
        {
            "context": "Workspace",
            "bindings": {
                "secondary-alt-u": ["task::Spawn", { "task_name": "zpic: upload clipboard as markdown" }],
                "secondary-alt-shift-u": ["task::Spawn", { "task_name": "zpic: upload clipboard as url" }],
                "secondary-alt-m": ["task::Spawn", { "task_name": "zpic: migrate current markdown file" }],
                "secondary-alt-d": ["task::Spawn", { "task_name": "zpic: doctor" }]
            }
        }
    ]);
    serde_json::to_string_pretty(&keymap).map_err(|e| ZpicError::Internal(e.to_string()))
}

fn render_readme(shell: ShellKind) -> Result<String> {
    let readme = format!(
        "# zpic Zed Tasks\n\
\n\
These files were generated by `zpic zed init`.\n\
\n\
Available tasks:\n\
- `zpic: upload clipboard as markdown` uploads the current clipboard image and copies Markdown output.\n\
- `zpic: upload clipboard as url` uploads the current clipboard image and copies the raw URL.\n\
- `zpic: migrate current markdown file` rewrites local image links in the active file.\n\
- `zpic: doctor` runs local diagnostics.\n\
- `zpic: uploader list` shows configured uploaders.\n\
\n\
Files generated for this platform:\n\
- `{}`\n\
- `{}`\n\
\n\
If you want keyboard shortcuts, open Zed's global keymap file with `zed: open keymap file`\n\
and merge in the bindings from `.zed/zpic-keymap.json.example`.\n",
        shell.upload_script_name(),
        shell.migrate_script_name()
    );
    Ok(readme)
}

fn simple_task(label: &str, command: &str, args: &[&str]) -> Map<String, Value> {
    let mut task = Map::new();
    task.insert("label".into(), json!(label));
    task.insert("command".into(), json!(command));
    task.insert(
        "args".into(),
        Value::Array(args.iter().map(|arg| json!(arg)).collect()),
    );
    task.insert("cwd".into(), json!("$ZED_WORKTREE_ROOT"));
    task.insert("use_new_terminal".into(), json!(false));
    task.insert("allow_concurrent_runs".into(), json!(true));
    task.insert("reveal".into(), json!("always"));
    task.insert("hide".into(), json!("never"));
    task.insert("show_summary".into(), json!(false));
    task.insert("show_command".into(), json!(true));
    task.insert("save".into(), json!("none"));
    task
}

fn write_text(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum ShellKind {
    Posix,
    PowerShell,
}

impl ShellKind {
    fn current() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Posix
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Posix => "posix",
            Self::PowerShell => "powershell",
        }
    }

    fn upload_script_name(self) -> &'static str {
        match self {
            Self::Posix => "zpic-upload-from-clipboard.sh",
            Self::PowerShell => "zpic-upload-from-clipboard.ps1",
        }
    }

    fn migrate_script_name(self) -> &'static str {
        match self {
            Self::Posix => "zpic-migrate-current-file.sh",
            Self::PowerShell => "zpic-migrate-current-file.ps1",
        }
    }

    fn upload_script(self) -> &'static str {
        match self {
            Self::Posix => POSIX_UPLOAD_SCRIPT,
            Self::PowerShell => POWERSHELL_UPLOAD_SCRIPT,
        }
    }

    fn migrate_script(self) -> &'static str {
        match self {
            Self::Posix => POSIX_MIGRATE_SCRIPT,
            Self::PowerShell => POWERSHELL_MIGRATE_SCRIPT,
        }
    }

    fn files(self, zed_dir: &Path) -> ZedFiles {
        ZedFiles {
            tasks: zed_dir.join("tasks.json"),
            keymap_example: zed_dir.join("zpic-keymap.json.example"),
            readme: zed_dir.join("zpic-README.md"),
            upload_script: zed_dir.join(self.upload_script_name()),
            migrate_script: zed_dir.join(self.migrate_script_name()),
        }
    }

    fn upload_task(self, label: &str, format: &str) -> Map<String, Value> {
        match self {
            Self::Posix => task_with_script(
                label,
                &format!(".zed/{}", self.upload_script_name()),
                &[format, "${ZED_SELECTED_TEXT:}"],
                "current",
                "no_focus",
                "on_success",
            ),
            Self::PowerShell => task_with_script(
                label,
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &format!(".zed\\{}", self.upload_script_name()),
                    format,
                    "${ZED_SELECTED_TEXT:}",
                ],
                "current",
                "no_focus",
                "on_success",
            ),
        }
    }

    fn migrate_task(self) -> Map<String, Value> {
        match self {
            Self::Posix => task_with_script(
                "zpic: migrate current markdown file",
                &format!(".zed/{}", self.migrate_script_name()),
                &["$ZED_FILE"],
                "current",
                "always",
                "never",
            ),
            Self::PowerShell => task_with_script(
                "zpic: migrate current markdown file",
                "powershell",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    &format!(".zed\\{}", self.migrate_script_name()),
                    "$ZED_FILE",
                ],
                "current",
                "always",
                "never",
            ),
        }
    }
}

#[derive(Debug)]
struct ZedFiles {
    tasks: PathBuf,
    keymap_example: PathBuf,
    readme: PathBuf,
    upload_script: PathBuf,
    migrate_script: PathBuf,
}

#[derive(Debug, Serialize)]
struct ZedInitPayload {
    action: &'static str,
    project_root: String,
    shell: &'static str,
    created: Vec<String>,
}

fn task_with_script(
    label: &str,
    command: &str,
    args: &[&str],
    save: &str,
    reveal: &str,
    hide: &str,
) -> Map<String, Value> {
    let mut task = Map::new();
    task.insert("label".into(), json!(label));
    task.insert("command".into(), json!(command));
    task.insert(
        "args".into(),
        Value::Array(args.iter().map(|arg| json!(arg)).collect()),
    );
    task.insert("cwd".into(), json!("$ZED_WORKTREE_ROOT"));
    task.insert("use_new_terminal".into(), json!(false));
    task.insert("allow_concurrent_runs".into(), json!(true));
    task.insert("reveal".into(), json!(reveal));
    task.insert("hide".into(), json!(hide));
    task.insert("show_summary".into(), json!(false));
    task.insert("show_command".into(), json!(true));
    task.insert("save".into(), json!(save));
    task
}
