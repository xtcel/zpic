//! Render output for the CLI: human-readable text and `--json` mode.

use serde::Serialize;

use zpic_core::config::OutputFormat;
use zpic_core::upload::UploadItem;

#[derive(Debug, Serialize)]
pub struct UploadPayload {
    pub success: bool,
    pub items: Vec<UploadItem>,
}

impl UploadPayload {
    pub fn from_items(items: Vec<UploadItem>) -> Self {
        let success = items.iter().all(|i| i.error.is_none());
        Self { success, items }
    }
}

pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("error serializing json output: {e}"),
    }
}

/// Render a single `UploadItem` for the default text mode (markdown).
pub fn render_item_text(item: &UploadItem, format: OutputFormat) -> String {
    if let Some(err) = &item.error {
        return format!("[error] {}: {}", item.source, err);
    }
    let url = item.url.as_deref().unwrap_or("");
    let key = item.key.as_deref().unwrap_or("");
    let alt = item
        .source
        .rsplit('/')
        .next()
        .unwrap_or("image")
        .rsplit_once('.')
        .map(|(n, _)| n)
        .unwrap_or("image");
    match format {
        OutputFormat::Url => url.to_string(),
        OutputFormat::Html => format!("<img src=\"{}\" alt=\"{}\" />", url, alt),
        OutputFormat::Jsx => format!("<Image src=\"{}\" alt=\"{}\" />", url, alt),
        OutputFormat::Markdown | OutputFormat::Json => {
            format!("![{}]({})", alt, url)
        }
    }
    .replace("{key}", key)
}
