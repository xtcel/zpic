//! `zpic doctor` — diagnostic checks for the local setup.

use std::path::PathBuf;

use crate::util::{load_config, load_uploader_registry, resolve_uploader, LoadedUploaderRegistry};
use zpic_config::paths::{candidate_picgo_paths, default_zpic_config};
use zpic_core::error::Result;
use zpic_history::HistoryStore;

pub fn run(explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    let mut report = DoctorReport::default();
    let registry = load_uploader_registry()?;
    check_config(&explicit_config, &mut report);
    check_picgo(&mut report);
    check_plugin_discovery(&registry, &mut report);
    check_active_uploader(&explicit_config, &registry, &mut report);
    check_clipboard(&mut report);
    check_history(&mut report);

    let ok = report.failed() == 0;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        print_text(&report);
    }
    if ok {
        Ok(0)
    } else {
        Ok(1)
    }
}

#[derive(Debug, Default, serde::Serialize)]
struct DoctorReport {
    checks: Vec<Check>,
}

impl DoctorReport {
    fn failed(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| matches!(c.status, CheckStatus::Fail))
            .count()
    }
}

#[derive(Debug, serde::Serialize)]
struct Check {
    name: String,
    status: CheckStatus,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    fn glyph(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "✓",
            CheckStatus::Warn => "!",
            CheckStatus::Fail => "✗",
        }
    }
}

fn check_config(explicit: &Option<PathBuf>, report: &mut DoctorReport) {
    if explicit.is_some() {
        report.checks.push(Check {
            name: "config (explicit --config)".into(),
            status: if explicit.as_ref().unwrap().exists() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            message: format!("path: {}", explicit.as_ref().unwrap().display()),
            fix: None,
        });
        return;
    }
    let user = default_zpic_config();
    if user.exists() {
        report.checks.push(Check {
            name: "config (user)".into(),
            status: CheckStatus::Pass,
            message: format!("path: {}", user.display()),
            fix: None,
        });
        return;
    }
    // No native config: try to fall back to PicGo. The actual loading is
    // best-effort here; the user gets a clearer message in `load_config`.
    let picgo = candidate_picgo_paths().into_iter().find(|p| p.exists());
    match picgo {
        Some(p) => report.checks.push(Check {
            name: "config (PicGo fallback)".into(),
            status: CheckStatus::Pass,
            message: format!("path: {}", p.display()),
            fix: Some("run `zpic config import-picgo` to convert this to native TOML".into()),
        }),
        None => report.checks.push(Check {
            name: "config (any source)".into(),
            status: CheckStatus::Fail,
            message: "no zpic or PicGo config found".into(),
            fix: Some("run `zpic config init` to create a starter config".into()),
        }),
    }
}

fn check_picgo(report: &mut DoctorReport) {
    let found: Vec<_> = candidate_picgo_paths()
        .into_iter()
        .filter(|p| p.exists())
        .collect();
    if found.is_empty() {
        report.checks.push(Check {
            name: "picgo (compatibility)".into(),
            status: CheckStatus::Warn,
            message: "no PicGo config detected".into(),
            fix: Some("PicGo config improves portability; see `zpic config import-picgo`".into()),
        });
    } else {
        let paths = found
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        report.checks.push(Check {
            name: "picgo (compatibility)".into(),
            status: CheckStatus::Pass,
            message: format!("found: {paths}"),
            fix: None,
        });
    }
}

fn check_plugin_discovery(registry: &LoadedUploaderRegistry, report: &mut DoctorReport) {
    if registry.diagnostics.is_empty() {
        report.checks.push(Check {
            name: "plugins".into(),
            status: CheckStatus::Pass,
            message: "plugin discovery succeeded".into(),
            fix: None,
        });
        return;
    }

    for diagnostic in &registry.diagnostics {
        report.checks.push(Check {
            name: "plugins".into(),
            status: match diagnostic.level {
                zpic_plugins::PluginDiagnosticLevel::Warn => CheckStatus::Warn,
                zpic_plugins::PluginDiagnosticLevel::Fail => CheckStatus::Fail,
            },
            message: format!("{}: {}", diagnostic.path, diagnostic.message),
            fix: Some("fix or remove the invalid plugin, then rerun `zpic doctor`".into()),
        });
    }
}

fn check_active_uploader(
    explicit: &Option<PathBuf>,
    registry: &LoadedUploaderRegistry,
    report: &mut DoctorReport,
) {
    let loaded = match load_config(explicit.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            report.checks.push(Check {
                name: "uploader".into(),
                status: CheckStatus::Fail,
                message: e.to_string(),
                fix: None,
            });
            return;
        }
    };
    let resolved = match resolve_uploader(&loaded, &registry.registry, None) {
        Ok(resolved) => resolved,
        Err(e) => {
            report.checks.push(Check {
                name: "uploader".into(),
                status: CheckStatus::Fail,
                message: e.to_string(),
                fix: Some(
                    "run `zpic set uploader <type> <name>` to create one, then `zpic use uploader <type> <name>` if needed".into(),
                ),
            });
            return;
        }
    };
    report.checks.push(Check {
        name: format!("uploader ({})", resolved.configured_type),
        status: CheckStatus::Pass,
        message: format!(
            "runtime type: {}, config: {}, source: {}",
            resolved.runtime_type,
            resolved.config_name,
            loaded.source.label()
        ),
        fix: None,
    });

    if let Err(err) = resolved.validate() {
        report.checks.push(Check {
            name: format!("uploader ({}) validation", resolved.configured_type),
            status: CheckStatus::Fail,
            message: err.to_string(),
            fix: Some("check the uploader config fields or plugin installation".into()),
        });
    }
}

fn check_clipboard(report: &mut DoctorReport) {
    match arboard::Clipboard::new() {
        Ok(_) => report.checks.push(Check {
            name: "clipboard".into(),
            status: CheckStatus::Pass,
            message: "available".into(),
            fix: None,
        }),
        Err(e) => report.checks.push(Check {
            name: "clipboard".into(),
            status: CheckStatus::Warn,
            message: format!("not available: {e}"),
            fix: Some("`zpic upload --clipboard` requires a working clipboard backend".into()),
        }),
    }
}

fn check_history(report: &mut DoctorReport) {
    match HistoryStore::open_default() {
        Ok(_) => report.checks.push(Check {
            name: "history".into(),
            status: CheckStatus::Pass,
            message: "writable".into(),
            fix: None,
        }),
        Err(e) => report.checks.push(Check {
            name: "history".into(),
            status: CheckStatus::Fail,
            message: e.to_string(),
            fix: Some("check filesystem permissions on the history store path".into()),
        }),
    }
}

fn print_text(report: &DoctorReport) {
    println!("zpic doctor");
    println!();
    let mut last_section = "";
    for c in &report.checks {
        let section = c.name.split_whitespace().next().unwrap_or("");
        if section != last_section {
            if !last_section.is_empty() {
                println!();
            }
            println!("{}:", capitalize(section));
            last_section = section;
        }
        println!("  {} {}", c.status.glyph(), c.name);
        println!("    {}", c.message);
        if let Some(fix) = &c.fix {
            println!("    fix: {fix}");
        }
    }
    println!();
    if report.failed() == 0 {
        println!("result: all checks passed");
    } else {
        println!("result: {} check(s) failed", report.failed());
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
