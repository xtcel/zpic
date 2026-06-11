//! `zpic upload` — single or multi-file upload with optional clipboard input.

use std::path::PathBuf;

use crate::cli::UploadArgs;
use crate::output::{render_item_text, UploadPayload};
use crate::pipeline::{self, ClipboardImage, PendingUpload};
use crate::progress::ProgressSink;
use crate::util::{load_config, load_uploader_registry, resolve_uploader};
use zpic_core::config::OutputFormat;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::UploadItem;

const CLIPBOARD_SOURCE: &str = "<clipboard>";

pub async fn run(args: UploadArgs, explicit_config: Option<PathBuf>, json: bool) -> Result<i32> {
    let config = load_config(explicit_config.as_deref())?;
    let loaded_registry = load_uploader_registry()?;
    let resolved = resolve_uploader(&config, &loaded_registry.registry, args.uploader.as_deref())?;
    let uploader = resolved.instantiate()?;

    let mut inputs: Vec<PendingUpload> = Vec::new();
    if args.clipboard {
        let image = read_clipboard().map_err(|e| {
            ZpicError::Clipboard(format!(
                "could not read clipboard image: {e}; copy an image to the clipboard first"
            ))
        })?;
        inputs.push(PendingUpload::from_clipboard(image));
    } else {
        if args.files.is_empty() {
            return Err(ZpicError::InvalidArgument(
                "no input files supplied; pass at least one path or use --clipboard".into(),
            ));
        }
        for path in &args.files {
            inputs.push(PendingUpload::from_path(path)?);
        }
    }

    // The progress sink renders a real-time progress line on stderr when
    // the terminal supports it. JSON mode stays silent to keep stdout
    // machine-readable; `--no-progress` also forces the sink off.
    let sink = ProgressSink::new(
        json || args.no_progress,
        uploader.name(),
        inputs.len(),
        args.dry_run,
    );
    sink.start();

    let mut items: Vec<UploadItem> = Vec::with_capacity(inputs.len());
    let mut last_text: Option<String> = None;
    for (idx, mut pending) in inputs.into_iter().enumerate() {
        pending.explicit_name = args.name.clone();
        pending.explicit_alt = args.alt.clone();
        let label = pending_label(&pending, idx + 1);
        let total_bytes = pending.bytes.len() as u64;
        sink.begin_file(&label, total_bytes);
        let on_progress = sink.callback_for(&label, total_bytes);
        match pipeline::run_upload(
            &config,
            uploader.as_ref(),
            pending,
            args.dry_run,
            Some(on_progress),
        )
        .await
        {
            Ok(out) => {
                sink.finish_file(&label, total_bytes);
                if let Some(text) = rendered_text(&out, &config, &args, json) {
                    last_text = Some(text);
                }
                items.push(UploadItem::success(out));
            }
            Err(err) => {
                sink.fail_file(&label);
                items.push(UploadItem::failure(
                    items_source_label(&args.clipboard, &items),
                    err.to_string(),
                ));
            }
        }
    }

    sink.finish();

    // History persistence.
    if !args.dry_run && config.zpic.history_enabled {
        if let Ok(store) = open_history() {
            for item in &items {
                if item.error.is_some() {
                    continue;
                }
                if let Some(out) = item_to_output(item) {
                    let source = item.source.clone();
                    let _ = store.record(&out, Some(&source));
                }
            }
        }
    }

    if args.copy {
        if let Some(text) = &last_text {
            if let Err(e) = copy_to_clipboard(text) {
                if !json {
                    eprintln!("warning: could not copy to clipboard: {e}");
                }
            }
        }
    }

    print_output(&args, &config, &items, json);
    if items.iter().any(|i| i.error.is_some()) {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn rendered_text(
    out: &zpic_core::upload::UploadOutput,
    config: &zpic_config::loader::LoadedConfig,
    args: &UploadArgs,
    json: bool,
) -> Option<String> {
    if json {
        return None;
    }
    let format = args
        .format
        .as_deref()
        .and_then(OutputFormat::from_str)
        .unwrap_or(config.zpic.default_format);
    let template = config.zpic.format.template_for(format);
    Some(pipeline::render_output(out, format, template))
}

fn print_output(
    args: &UploadArgs,
    config: &zpic_config::loader::LoadedConfig,
    items: &[UploadItem],
    json: bool,
) {
    if json {
        let payload = UploadPayload::from_items(items.to_vec());
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return;
    }
    let format = args
        .format
        .as_deref()
        .and_then(OutputFormat::from_str)
        .unwrap_or(config.zpic.default_format);
    for item in items {
        println!("{}", render_item_text(item, format));
    }
}

fn items_source_label(clipboard: &bool, _items: &[UploadItem]) -> String {
    if *clipboard {
        CLIPBOARD_SOURCE.to_string()
    } else {
        "<unknown>".to_string()
    }
}

/// Best-effort human-readable label for an in-flight upload. Prefers the
/// original file name (so users see `cover.png` instead of the rendered
/// target key) and falls back to the source path.
fn pending_label(pending: &PendingUpload, idx: usize) -> String {
    let name = pending
        .source_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| pending.file_name.clone());
    if name.is_empty() || name == CLIPBOARD_SOURCE {
        format!("[{idx}] {CLIPBOARD_SOURCE}")
    } else {
        format!("[{idx}] {name}")
    }
}

fn item_to_output(item: &UploadItem) -> Option<zpic_core::upload::UploadOutput> {
    Some(zpic_core::upload::UploadOutput {
        source: item.source.clone(),
        url: item.url.clone()?,
        key: item.key.clone()?,
        markdown: item.markdown.clone()?,
        mime: item.mime.clone()?,
        size: item.size? as u64,
        width: item.width,
        height: item.height,
        uploader: item.uploader.clone()?,
    })
}

fn open_history() -> Result<zpic_history::HistoryStore> {
    zpic_history::HistoryStore::open_default()
}

fn read_clipboard() -> std::result::Result<ClipboardImage, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let img = cb.get_image().map_err(|e| e.to_string())?;
    let bytes = bytes::Bytes::from(img.bytes.to_vec());
    let mime = match img.width * img.height {
        // The `infer` crate will give us the right type from bytes; for
        // a generic RGBA buffer from arboard, image/png is the safe bet.
        _ => "image/png".to_string(),
    };
    let file_name = format!("clipboard-{}", chrono::Utc::now().timestamp());
    Ok(ClipboardImage {
        bytes,
        file_name,
        mime,
    })
}

fn copy_to_clipboard(text: &str) -> std::result::Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}
