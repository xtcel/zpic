//! Markdown image discovery and rewrite logic.
//!
//! Uses `pulldown-cmark` to tokenize a Markdown document, finds inline and
//! reference-style image links that point to local files, uploads them
//! through the active uploader, and rewrites the file by replacing the
//! original target with the new remote URL.

use std::path::{Path, PathBuf};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// One local image reference discovered in a markdown document.
#[derive(Debug, Clone)]
pub struct LocalImage {
    /// The raw destination string as it appears in the markdown source.
    pub dest: String,
    /// The local file path on disk, resolved relative to `base`.
    pub resolved: PathBuf,
    /// Byte offset into the source where the image target begins.
    pub span_start: usize,
    /// Byte offset into the source where the image target ends.
    pub span_end: usize,
    /// Original full image syntax span, used for context.
    pub full_start: usize,
    pub full_end: usize,
}

/// A planned rewrite: the destination string and its replacement URL.
#[derive(Debug, Clone)]
pub struct PlannedChange {
    pub source: String,
    pub from: String,
    pub to: String,
    pub markdown: String,
}

/// Scan a single markdown document. Returns the set of local image
/// references that are eligible for upload.
pub fn scan_markdown(source: &str, base: &Path) -> Vec<LocalImage> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(source, opts);
    let mut images = Vec::new();
    let mut current_link: Option<(String, PathBuf, usize, usize)> = None;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                let raw = dest_url.to_string();
                if let Some(resolved) = resolve_local(&raw, base) {
                    current_link = Some((raw, resolved, range.start, range.end));
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some((dest, resolved, start, end)) = current_link.take() {
                    images.push(LocalImage {
                        dest,
                        resolved,
                        full_start: start,
                        full_end: end,
                        span_start: start,
                        span_end: end,
                    });
                }
            }
            _ => {}
        }
    }
    // Also handle the reference-style definitions (`[foo]: ./pic.png`).
    images.extend(scan_reference_defs(source, base));
    // Deduplicate by `resolved` path.
    images.sort_by_key(|img| img.full_start);
    images.dedup_by(|a, b| a.resolved == b.resolved && a.span_start == b.span_start);
    images
}

fn scan_reference_defs(source: &str, base: &Path) -> Vec<LocalImage> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('[') {
            continue;
        }
        if let Some(colon) = trimmed.find("]:") {
            let after = trimmed[colon + 2..].trim();
            if after.is_empty() || after.starts_with('<') {
                continue;
            }
            // Skip explicit titles in `"..."` or `'...'` form.
            let dest = after
                .split_whitespace()
                .next()
                .unwrap_or(after)
                .trim_matches(|c| c == '"' || c == '\'');
            if let Some(resolved) = resolve_local(dest, base) {
                let start = line.as_ptr() as usize - source.as_ptr() as usize;
                let end = start + line.len();
                out.push(LocalImage {
                    dest: dest.to_string(),
                    resolved,
                    full_start: start,
                    full_end: end,
                    span_start: start,
                    span_end: end,
                });
            }
        }
    }
    out
}

/// Resolve a markdown target to a local file path if it looks local.
/// Returns `None` for absolute URLs (http/https/data).
pub fn resolve_local(dest: &str, base: &Path) -> Option<PathBuf> {
    if dest.is_empty() {
        return None;
    }
    if dest.contains("://") {
        return None;
    }
    if dest.starts_with("data:") {
        return None;
    }
    if dest.starts_with('/') {
        return Some(PathBuf::from(dest));
    }
    let stripped = dest
        .split('?')
        .next()
        .unwrap_or(dest)
        .split('#')
        .next()
        .unwrap_or(dest);
    let joined = base.join(stripped);
    Some(joined)
}

/// Rewrite `source` by replacing each `change.from` substring with the
/// new markdown. The result is a rewritten markdown document.
pub fn rewrite_markdown(source: &str, changes: &[PlannedChange]) -> String {
    if changes.is_empty() {
        return source.to_string();
    }
    let mut out = source.to_string();
    let mut ranges: Vec<(usize, usize, String)> = Vec::new();
    for c in changes {
        // Find the rightmost occurrence of `c.from` in `out` that does
        // not overlap an already-recorded range.
        let mut search_from = out.len();
        let mut found: Option<(usize, usize)> = None;
        while let Some(pos) = out[..search_from].rfind(&c.from) {
            let end = pos + c.from.len();
            let overlaps = ranges.iter().any(|(s, e, _)| !(end <= *s || pos >= *e));
            if !overlaps {
                found = Some((pos, end));
                break;
            }
            if pos == 0 {
                break;
            }
            search_from = pos;
        }
        if let Some((start, end)) = found {
            ranges.push((start, end, c.markdown.clone()));
        }
    }
    // Apply right-to-left so earlier offsets stay valid.
    ranges.sort_by(|a, b| b.0.cmp(&a.0));
    for (start, end, replacement) in ranges {
        out.replace_range(start..end, &replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_inline_image() {
        let md = "Hello\n\n![cover](./img/cover.png)\n\nWorld";
        let base = std::path::Path::new("/docs");
        let imgs = scan_markdown(md, base);
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].dest, "./img/cover.png");
        assert!(imgs[0].resolved.ends_with("img/cover.png"));
    }

    #[test]
    fn ignores_remote_image() {
        let md = "![cover](https://cdn.example.com/cover.png)";
        let imgs = scan_markdown(md, Path::new("/docs"));
        assert!(imgs.is_empty());
    }

    #[test]
    fn scans_reference_def() {
        let md = "Text\n\n[logo]: ./logo.png \"logo\"\n\n![logo][logo]\n";
        let imgs = scan_markdown(md, Path::new("/docs"));
        assert!(!imgs.is_empty());
    }

    #[test]
    fn rewrite_replaces_url() {
        let md = "A\n\n![cover](./cover.png)\n\nB";
        let change = PlannedChange {
            source: "./cover.png".to_string(),
            from: "./cover.png".to_string(),
            to: "https://cdn/cover.png".to_string(),
            markdown: "![cover](https://cdn/cover.png)".to_string(),
        };
        let rewritten = rewrite_markdown(md, &[change]);
        assert!(rewritten.contains("https://cdn/cover.png"));
    }
}
