//! Output rendering (markdown, url, html, jsx) for upload and migrate results.

use crate::config::OutputFormat;
use crate::upload::UploadOutput;

/// Coarse media classification. The CLI uses this to pick sensible
/// default HTML / JSX templates — `<img>` for images, `<audio>` for
/// audio, `<video>` for video. The classification is intentionally
/// driven by the MIME top-level type so it stays correct even for
/// formats whose extensions are ambiguous (e.g. `webm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Audio,
    Video,
    /// Anything that is not a top-level `image/*`, `audio/*`, or `video/*`.
    /// Falls back to a plain link in the HTML/JSX renderers.
    Other,
}

/// Classify a MIME string into a [`MediaKind`]. Strips parameters (the
/// `; charset=...` part) before matching, and is case-insensitive.
pub fn media_kind_for(mime: &str) -> MediaKind {
    let top = mime
        .split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase();
    if top.starts_with("image/") {
        MediaKind::Image
    } else if top.starts_with("audio/") {
        MediaKind::Audio
    } else if top.starts_with("video/") {
        MediaKind::Video
    } else {
        MediaKind::Other
    }
}

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

/// Default template for a known `OutputFormat` when the media is an
/// image. Kept as a thin wrapper for backwards compatibility with
/// callers (and tests) that don't care about audio/video.
pub fn default_template(format: OutputFormat) -> &'static str {
    default_template_for(format, MediaKind::Image)
}

/// Default template for a known `OutputFormat` and media kind.
///
/// Markdown is intentionally format-agnostic — the `![alt](url)` form
/// works in Obsidian, VS Code, and most static-site generators for
/// images; many renderers fall back to a link for non-images. Users
/// who want audio/video specific Markdown can pass `--format` with a
/// custom template.
pub fn default_template_for(format: OutputFormat, kind: MediaKind) -> &'static str {
    match (format, kind) {
        (OutputFormat::Markdown, _) => "![{alt}]({url})",
        (OutputFormat::Url, _) => "{url}",
        (OutputFormat::Html, MediaKind::Image) => "<img src=\"{url}\" alt=\"{alt}\" />",
        (OutputFormat::Html, MediaKind::Audio) => "<audio controls src=\"{url}\"></audio>",
        (OutputFormat::Html, MediaKind::Video) => "<video controls src=\"{url}\"></video>",
        (OutputFormat::Html, MediaKind::Other) => "<a href=\"{url}\">{alt}</a>",
        (OutputFormat::Jsx, MediaKind::Image) => {
            "<Image src=\"{url}\" alt=\"{alt}\" width={width} height={height} />"
        }
        (OutputFormat::Jsx, MediaKind::Audio) => "<audio controls src=\"{url}\" />",
        (OutputFormat::Jsx, MediaKind::Video) => {
            "<video controls src=\"{url}\" width={width} height={height} />"
        }
        (OutputFormat::Jsx, MediaKind::Other) => "<a href=\"{url}\">{alt}</a>",
        (OutputFormat::Json, _) => "", // JSON mode is rendered separately.
    }
}

/// Render a single upload using the given format and optional custom
/// template. When `custom_template` is `Some`, it overrides the default for
/// the selected format (and media kind).
pub fn render_format_for_kind(
    format: OutputFormat,
    custom_template: Option<&str>,
    out: &UploadOutput,
) -> String {
    if matches!(format, OutputFormat::Json) {
        // JSON rendering is the responsibility of the caller.
        return String::new();
    }
    let kind = media_kind_for(&out.mime);
    let template =
        custom_template.unwrap_or_else(|| default_template_for(format, kind));
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

    #[test]
    fn classifies_mime_into_media_kind() {
        assert_eq!(media_kind_for("image/png"), MediaKind::Image);
        assert_eq!(media_kind_for("image/svg+xml"), MediaKind::Image);
        assert_eq!(media_kind_for("audio/mpeg"), MediaKind::Audio);
        assert_eq!(media_kind_for("audio/ogg; codecs=opus"), MediaKind::Audio);
        assert_eq!(media_kind_for("video/mp4"), MediaKind::Video);
        assert_eq!(media_kind_for("video/webm"), MediaKind::Video);
        assert_eq!(media_kind_for("text/plain"), MediaKind::Other);
        assert_eq!(media_kind_for("application/octet-stream"), MediaKind::Other);
        // Case-insensitive.
        assert_eq!(media_kind_for("IMAGE/PNG"), MediaKind::Image);
    }

    #[test]
    fn html_renders_audio_tag_for_audio_mime() {
        let mut out = sample_output();
        out.mime = "audio/mpeg".into();
        out.url = "https://cdn.example.com/track.mp3".into();
        out.key = "audio/track.mp3".into();
        out.markdown = "![track.mp3](https://cdn.example.com/track.mp3)".into();
        out.width = None;
        out.height = None;
        let rendered = render_format_for_kind(OutputFormat::Html, None, &out);
        assert_eq!(
            rendered,
            "<audio controls src=\"https://cdn.example.com/track.mp3\"></audio>"
        );
    }

    #[test]
    fn html_renders_video_tag_for_video_mime() {
        let mut out = sample_output();
        out.mime = "video/mp4".into();
        out.url = "https://cdn.example.com/clip.mp4".into();
        out.key = "video/clip.mp4".into();
        out.markdown = "![clip.mp4](https://cdn.example.com/clip.mp4)".into();
        out.width = None;
        out.height = None;
        let rendered = render_format_for_kind(OutputFormat::Html, None, &out);
        assert_eq!(
            rendered,
            "<video controls src=\"https://cdn.example.com/clip.mp4\"></video>"
        );
    }

    #[test]
    fn jsx_renders_video_with_dimensions_when_known() {
        let mut out = sample_output();
        out.mime = "video/webm".into();
        out.markdown = "![clip.webm](https://cdn.example.com/clip.webm)".into();
        out.width = Some(1920);
        out.height = Some(1080);
        let rendered = render_format_for_kind(OutputFormat::Jsx, None, &out);
        assert!(rendered.contains("<video"));
        assert!(rendered.contains("width=1920"));
        assert!(rendered.contains("height=1080"));
    }
}
