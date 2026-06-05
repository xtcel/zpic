//! `zpic history` — list and delete past uploads.

use std::path::PathBuf;

use crate::cli::HistoryAction;
use crate::util::load_config;
use zpic_core::error::{Result, ZpicError};
use zpic_history::{HistoryStore, ListFilter};

pub fn run(action: HistoryAction, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    // Loading the config keeps the precedence behavior consistent with the
    // other commands (e.g. a project config might disable history).
    let _ = load_config(explicit_config.as_deref());
    let store = HistoryStore::open_default()?;
    match action {
        HistoryAction::List { uploader, limit } => {
            let entries = store.list(ListFilter { uploader, limit })?;
            if json {
                let json_entries: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "created_at": e.created_at.to_rfc3339(),
                            "source_path": e.source_path,
                            "uploader": e.uploader,
                            "key": e.key,
                            "url": e.url,
                            "markdown": e.markdown,
                            "mime": e.mime,
                            "size": e.size,
                            "width": e.width,
                            "height": e.height,
                            "status": e.status,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_entries)
                        .unwrap_or_else(|_| "[]".to_string())
                );
            } else {
                if entries.is_empty() {
                    println!("no uploads recorded yet");
                } else {
                    println!("{:<24} {:<10} {:<60} {}", "WHEN", "UPLOADER", "URL", "KEY");
                    for e in &entries {
                        println!(
                            "{:<24} {:<10} {:<60} {}",
                            e.created_at.format("%Y-%m-%d %H:%M:%S"),
                            e.uploader,
                            truncate(&e.url, 60),
                            e.key,
                        );
                    }
                }
            }
            Ok(0)
        }
        HistoryAction::Delete { id } => {
            if store.delete(&id)? {
                if !json {
                    println!("deleted entry {id}");
                }
                Ok(0)
            } else {
                Err(ZpicError::InvalidArgument(format!(
                    "no history entry with id {id}"
                )))
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
