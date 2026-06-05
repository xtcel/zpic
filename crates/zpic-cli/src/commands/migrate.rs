//! `zpic migrate` — scan markdown files for local images, optionally
//! upload them, and rewrite the references.

use std::path::{Path, PathBuf};

use crate::cli::MigrateArgs;
use crate::migrate::{scan_markdown, PlannedChange};
use crate::output::UploadPayload;
use crate::pipeline::{self, PendingUpload};
use crate::util::{load_config, load_uploader_registry, resolve_uploader};
use serde::Serialize;
use zpic_core::config::OutputFormat;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::UploadItem;

pub async fn run(args: MigrateArgs, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    let config = load_config(explicit_config.as_deref())?;
    let loaded_registry = load_uploader_registry()?;
    let resolved = resolve_uploader(&config, &loaded_registry.registry, args.uploader.as_deref())?;
    let uploader = resolved.instantiate()?;

    let files = collect_markdown_files(&args.path, args.recursive)?;
    if files.is_empty() {
        return Err(ZpicError::Migration(format!(
            "no markdown files found under {}",
            args.path.display()
        )));
    }

    let mut report = MigrateReport::default();
    for file in files {
        process_file(&file, &config, uploader.as_ref(), &args, &mut report).await?;
    }

    if let Some(path) = &args.report {
        let text = serde_json::to_string_pretty(&report)
            .map_err(|e| ZpicError::Internal(e.to_string()))?;
        std::fs::write(path, text)?;
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| ZpicError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "scanned {} file(s); found {} image(s); uploaded {}; written {}",
            report.scanned_files, report.found, report.uploaded, report.rewritten_files
        );
    }
    if report.uploaded < report.found {
        Ok(1)
    } else {
        Ok(0)
    }
}

#[derive(Debug, Default, Serialize)]
struct MigrateReport {
    scanned_files: usize,
    found: usize,
    uploaded: usize,
    rewritten_files: usize,
    changes: Vec<ChangeRecord>,
    /// Mirrors of failed uploads so the JSON surface stays consistent.
    #[serde(default)]
    items: Vec<UploadItem>,
}

#[derive(Debug, Serialize)]
struct ChangeRecord {
    file: String,
    from: String,
    to: String,
    markdown: String,
}

async fn process_file(
    file: &Path,
    config: &zpic_config::loader::LoadedConfig,
    uploader: &dyn zpic_core::upload::Uploader,
    args: &MigrateArgs,
    report: &mut MigrateReport,
) -> Result<()> {
    let base = file.parent().unwrap_or_else(|| Path::new("."));
    let source = std::fs::read_to_string(file)?;
    let images = scan_markdown(&source, base);
    report.scanned_files += 1;
    report.found += images.len();

    let mut planned: Vec<PlannedChange> = Vec::new();
    for img in images {
        // Skip missing files rather than failing the whole migration.
        if !img.resolved.exists() {
            if !args.dry_run {
                eprintln!(
                    "warning: local image {} not found, skipping",
                    img.resolved.display()
                );
            }
            continue;
        }
        if args.ignore_remote && img.dest.starts_with("http") {
            continue;
        }
        let pending = PendingUpload::from_path(&img.resolved)?;
        match pipeline::run_upload(config, uploader, pending, args.dry_run).await {
            Ok(out) => {
                let format = args
                    .format
                    .as_deref()
                    .and_then(OutputFormat::from_str)
                    .unwrap_or(config.zpic.default_format);
                let template = config.zpic.format.template_for(format);
                let rendered = pipeline::render_output(&out, format, template);
                report.items.push(UploadItem::success(out.clone()));
                planned.push(PlannedChange {
                    source: file.to_string_lossy().into_owned(),
                    from: img.dest.clone(),
                    to: out.url.clone(),
                    markdown: rendered,
                });
                report.changes.push(ChangeRecord {
                    file: file.to_string_lossy().into_owned(),
                    from: img.dest,
                    to: out.url,
                    markdown: out.markdown,
                });
                report.uploaded += 1;
            }
            Err(e) => {
                report.items.push(UploadItem::failure(
                    img.resolved.to_string_lossy().into_owned(),
                    e.to_string(),
                ));
            }
        }
    }

    if !args.dry_run && !planned.is_empty() {
        let new_source = crate::migrate::rewrite_markdown(&source, &planned);
        std::fs::write(file, new_source)?;
        report.rewritten_files += 1;
    }
    Ok(())
}

fn collect_markdown_files(path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(ZpicError::FileNotFound(path.to_path_buf()));
    }
    let mut out = Vec::new();
    if recursive {
        for entry in walkdir::WalkDir::new(path) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            if is_markdown(p) {
                out.push(p.to_path_buf());
            }
        }
    } else {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() && is_markdown(&p) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

fn is_markdown(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()),
        Some(ref ext) if ext == "md" || ext == "markdown" || ext == "mdx"
    )
}

#[allow(dead_code)]
fn _payload_used(_: &UploadPayload) {}
