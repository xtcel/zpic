//! Path template rendering.
//!
//! Supports these placeholders:
//! `{yyyy} {yy} {mm} {dd} {hh} {min} {ss} {timestamp} {unix}
//!  {name} {slug} {hash} {hash8} {uuid} {ext} {random}`

use chrono::{Local, Utc};
use uuid::Uuid;

/// All the variables a template can reference. `name` and `slug` are derived
/// from the source file name; `hash`/`hash8` come from the content hash.
#[derive(Debug, Clone)]
pub struct TemplateContext<'a> {
    pub file_name: &'a str,
    pub extension: &'a str,
    pub content_hash_hex: &'a str,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<'a> TemplateContext<'a> {
    pub fn new(file_name: &'a str, extension: &'a str, content_hash_hex: &'a str) -> Self {
        Self {
            file_name,
            extension,
            content_hash_hex,
            timestamp: Utc::now(),
        }
    }
}

/// Render `template` using the variables in `ctx`.
pub fn render_template(template: &str, ctx: &TemplateContext<'_>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    let local = ctx.timestamp.with_timezone(&Local);
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close_rel) = template[i + 1..].find('}') {
                let key = &template[i + 1..i + 1 + close_rel];
                i = i + 1 + close_rel + 1;
                match key {
                    "yyyy" => out.push_str(&local.format("%Y").to_string()),
                    "yy" => out.push_str(&local.format("%y").to_string()),
                    "mm" => out.push_str(&local.format("%m").to_string()),
                    "dd" => out.push_str(&local.format("%d").to_string()),
                    "hh" => out.push_str(&local.format("%H").to_string()),
                    "min" => out.push_str(&local.format("%M").to_string()),
                    "ss" => out.push_str(&local.format("%S").to_string()),
                    "timestamp" => out.push_str(&ctx.timestamp.timestamp_millis().to_string()),
                    "unix" => out.push_str(&ctx.timestamp.timestamp().to_string()),
                    "name" => out.push_str(ctx.file_name),
                    "slug" => out.push_str(&slugify(ctx.file_name)),
                    "hash" => out.push_str(ctx.content_hash_hex),
                    "hash8" => {
                        out.push_str(&ctx.content_hash_hex.chars().take(8).collect::<String>())
                    }
                    "uuid" => out.push_str(&Uuid::new_v4().to_string()),
                    "ext" => out.push_str(ctx.extension),
                    "random" => out.push_str(&Uuid::new_v4().simple().to_string()[..12]),
                    other => {
                        out.push('{');
                        out.push_str(other);
                        out.push('}');
                    }
                }
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Produce a filesystem-friendly slug from an arbitrary string.
fn slugify(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if ch == '-' || ch == '_' {
            s.push(ch);
            last_dash = false;
        } else if !last_dash && !s.is_empty() {
            s.push('-');
            last_dash = true;
        }
    }
    while s.ends_with('-') {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(name: &'a str, ext: &'a str, hash: &'a str) -> TemplateContext<'a> {
        TemplateContext::new(name, ext, hash)
    }

    #[test]
    fn renders_default_date_hash_template() {
        let template = "images/{yyyy}/{mm}/{dd}/{hash8}.{ext}";
        let c = ctx("cover", "png", "abcdef0123456789");
        let out = render_template(template, &c);
        assert!(out.starts_with("images/"));
        assert!(out.ends_with("/abcdef01.png"));
    }

    #[test]
    fn renders_name_and_slug() {
        let template = "blog/{name}-{slug}.{ext}";
        let c = ctx("Hello World!", "jpg", "deadbeef");
        let out = render_template(template, &c);
        assert_eq!(out, "blog/Hello World!-hello-world.jpg");
    }

    #[test]
    fn unknown_placeholder_is_preserved() {
        let template = "x/{unknown}/{name}.{ext}";
        let c = ctx("a", "png", "h");
        let out = render_template(template, &c);
        assert_eq!(out, "x/{unknown}/a.png");
    }
}
