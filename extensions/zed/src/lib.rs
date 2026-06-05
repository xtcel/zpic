use zed_extension_api::{
    self as zed, process, SlashCommand, SlashCommandArgumentCompletion, SlashCommandOutput,
    SlashCommandOutputSection, Worktree,
};

struct ZpicAssistantExtension;

impl zed::Extension for ZpicAssistantExtension {
    fn new() -> Self {
        Self
    }

    fn complete_slash_command_argument(
        &self,
        command: SlashCommand,
        _args: Vec<String>,
    ) -> Result<Vec<SlashCommandArgumentCompletion>, String> {
        match command.name.as_str() {
            "zpic-upload" => Ok(vec![
                completion("--format markdown", false),
                completion("--format url", false),
                completion("--format html", false),
                completion("--dry-run", true),
            ]),
            "zpic-history" => Ok(vec![
                completion("--limit 20", true),
                completion("--uploader local", true),
                completion("--uploader github", true),
                completion("--uploader s3", true),
            ]),
            "zpic-uploader-list" => Ok(vec![
                completion("local", true),
                completion("github", true),
                completion("s3", true),
            ]),
            "zpic-doctor" => Ok(vec![]),
            other => Err(format!("unknown slash command: \"{other}\"")),
        }
    }

    fn run_slash_command(
        &self,
        command: SlashCommand,
        args: Vec<String>,
        worktree: Option<&Worktree>,
    ) -> Result<SlashCommandOutput, String> {
        let (label, command_args) = match command.name.as_str() {
            "zpic-upload" => {
                let mut command_args = vec!["upload".to_string(), "--clipboard".to_string()];
                command_args.extend(args);
                ("zpic upload --clipboard".to_string(), command_args)
            }
            "zpic-doctor" => {
                let mut command_args = vec!["doctor".to_string()];
                command_args.extend(args);
                ("zpic doctor".to_string(), command_args)
            }
            "zpic-history" => {
                let mut command_args = vec!["history".to_string(), "list".to_string()];
                command_args.extend(args);
                ("zpic history list".to_string(), command_args)
            }
            "zpic-uploader-list" => {
                let mut command_args = vec!["uploader".to_string(), "list".to_string()];
                command_args.extend(args);
                ("zpic uploader list".to_string(), command_args)
            }
            other => return Err(format!("unknown slash command: \"{other}\"")),
        };

        let text = run_zpic(command_args, worktree)?;
        Ok(SlashCommandOutput {
            sections: vec![SlashCommandOutputSection {
                range: (0..text.len()).into(),
                label,
            }],
            text,
        })
    }
}

fn completion(new_text: &str, run_command: bool) -> SlashCommandArgumentCompletion {
    SlashCommandArgumentCompletion {
        label: new_text.to_string(),
        new_text: new_text.to_string(),
        run_command,
    }
}

fn run_zpic(args: Vec<String>, worktree: Option<&Worktree>) -> Result<String, String> {
    ensure_zpic_available(worktree)?;

    let mut command = process::Command::new("zpic").args(args);
    if let Some(worktree) = worktree {
        command = command.envs(worktree.shell_env());
    }

    let output = command.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    match output.status {
        Some(0) => {
            if stdout.is_empty() && stderr.is_empty() {
                Ok("zpic command finished successfully with no output.".to_string())
            } else if stderr.is_empty() {
                Ok(stdout)
            } else if stdout.is_empty() {
                Ok(stderr)
            } else {
                Ok(format!("{stdout}\n\nstderr:\n{stderr}"))
            }
        }
        Some(code) => {
            let message = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("zpic exited with status code {code}")
            };
            Err(message)
        }
        None => Err(if !stderr.is_empty() {
            stderr
        } else {
            "zpic terminated unexpectedly".to_string()
        }),
    }
}

fn ensure_zpic_available(worktree: Option<&Worktree>) -> Result<(), String> {
    if let Some(worktree) = worktree {
        if worktree.which("zpic").is_some() {
            return Ok(());
        }
    } else {
        return Ok(());
    }

    Err(
        "could not find `zpic` on your PATH. Install it first, then restart Zed from that shell."
            .to_string(),
    )
}

zed::register_extension!(ZpicAssistantExtension);
