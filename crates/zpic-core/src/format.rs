//! Output rendering (markdown, url, html, jsx) for upload and migrate results.

use crate::config::OutputFormat;
use crate::upload::UploadOutput;

/// Variables available in user-supplied format templates.
#[derive(Debug, Clone)]
pub struct FormatVars<'a> {
    pub url: &'a str,
    pub alt: Option<&'a str>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mime: &'a str,
    pub size: u64,
    pub key: &'a str,
}

impl<'a> FormatVars<'a> {
    pub fn from_output(out: &'a UploadOutput) -> Self {
        Self {
            url: &out.url,
            alt: out.markdown_alt(),
            width: out.width,
            height: out.height,
            mime: &out.mime,
            size: out.size,
            key: &out.key,
        }
    }
}

/// Helper accessor for `alt` derived from the source filename. The
/// `markdown` field already contains the rendered text, but most callers
/// want the bare alt value; we re-derive it by stripping the `![]()` wrapper.
trait MarkdownAlt {
    fn markdown_alt(&self) -> Option<&str>;
}

impl MarkdownAlt for UploadOutput {
    fn markdown_alt(&self) -> Option<&str> {
        // `markdown` is `![alt](url)`; we try to extract `alt` lazily.
        let md = &self.markdown;
        if md.starts_with("![") {
            if let Some(close) = md.find("](") {
                return Some(&md[2..close]);
            }
        }
        None
    }
}

/// Render `vars` according to a custom template string.
///
/// Supported placeholders: `{url}`, `{alt}`, `{width}`, `{height}`,
/// `{mime}`, `{size}`, `{key}`.
pub fn render_format(template: &str, vars: &FormatVars<'_>) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close) = template[i + 1..].find('}') {
                let key = &template[i + 1..i + 1 + close];
                i = i + 1 + close + 1;
                match key {
                    "url" => out.push_str(vars.url),
                    "alt" => {
                        if let Some(alt) = vars.alt {
                            out.push_str(alt);
                        }
                    }
                    "width" => {
                        if let Some(w) = vars.width {
                            out.push_str(&w.to_string());
                        }
                    }
                    "height" => {
                        if let Some(h) = vars.height {
                            out.push_str(&h.to_string());
                        }
                    }
                    "mime" => out.push_str(vars.mime),
                    "size" => out.push_str(&vars.size.to_string()),
                    "key" => out.push_str(vars.key),
                    other => {
                        // Unknown placeholder: leave it as-is so users notice.
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

/// Default template for a known `OutputFormat`.
pub fn default_template(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Markdown => "![{alt}]({url})",
        OutputFormat::Url => "{url}",
        OutputFormat::Html => "<img src=\"{url}\" alt=\"{alt}\" />",
        OutputFormat::Jsx => "<Image src=\"{url}\" alt=\"{alt}\" width={width} height={height} />",
        OutputFormat::Json => "", // JSON mode is rendered separately.
    }
}

/// Render a single upload using the given format and optional custom
/// template. When `custom_template` is `Some`, it overrides the default for
/// the selected format.
pub fn render_format_for_kind(
    format: OutputFormat,
    custom_template: Option<&str>,
    out: &UploadOutput,
) -> String {
    if matches!(format, OutputFormat::Json) {
        // JSON rendering is the responsibility of the caller.
        return String::new();
    }
    let template = custom_template.unwrap_or_else(|| default_template(format));
    let vars = FormatVars::from_output(out);
    render_format(template, &vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_output() -> UploadOutput {
        UploadOutput {
            source: "cover.png".into(),
            url: "https://cdn.example.com/cover.png".into(),
            key: "images/2026/06/04/cover.png".into(),
            markdown: "![cover](https://cdn.example.com/cover.png)".into(),
            mime: "image/png".into(),
            size: 12345,
            width: Some(800),
            height: Some(600),
            uploader: "local".into(),
        }
    }

    #[test]
    fn renders_markdown_default() {
        let out = sample_output();
        let rendered = render_format_for_kind(OutputFormat::Markdown, None, &out);
        assert_eq!(rendered, "![cover](https://cdn.example.com/cover.png)");
    }

    #[test]
    fn renders_url_format() {
        let out = sample_output();
        let rendered = render_format_for_kind(OutputFormat::Url, None, &out);
        assert_eq!(rendered, "https://cdn.example.com/cover.png");
    }

    #[test]
    fn renders_html_format() {
        let out = sample_output();
        let rendered = render_format_for_kind(OutputFormat::Html, None, &out);
        assert_eq!(
            rendered,
            "<img src=\"https://cdn.example.com/cover.png\" alt=\"cover\" />"
        );
    }

    #[test]
    fn renders_jsx_with_dimensions() {
        let out = sample_output();
        let rendered = render_format_for_kind(OutputFormat::Jsx, None, &out);
        assert!(rendered.contains("width=800"));
        assert!(rendered.contains("height=600"));
    }

    #[test]
    fn custom_template_overrides_default() {
        let out = sample_output();
        let rendered = render_format_for_kind(
            OutputFormat::Markdown,
            Some("![{alt}]({url}?w={width})"),
            &out,
        );
        assert_eq!(
            rendered,
            "![cover](https://cdn.example.com/cover.png?w=800)"
        );
    }
}
