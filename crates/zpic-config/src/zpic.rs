//! Native zpic TOML configuration model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use zpic_core::config::{OutputFormat, RenameStrategy, UploaderKind};

/// Top-level zpic config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZpicConfigFile {
    /// Name of the default uploader under `[uploaders.<name>]`.
    pub default_uploader: Option<String>,
    /// Default output format for `zpic upload` and `zpic migrate`.
    #[serde(default)]
    pub default_format: OutputFormat,
    /// Copy rendered output to the system clipboard after each successful upload.
    #[serde(default)]
    pub copy_after_upload: bool,
    /// Persist successful uploads in the SQLite history store.
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,
    /// Rename / path-template strategy.
    #[serde(default)]
    pub rename: RenameSection,
    /// Per-format output templates.
    #[serde(default)]
    pub format: FormatSection,
    /// Named uploaders, keyed by user-chosen name (e.g. `r2`, `github`).
    #[serde(default)]
    pub uploaders: BTreeMap<String, UploaderSection>,
}

fn default_history_enabled() -> bool {
    true
}

impl ZpicConfigFile {
    /// Render a stable TOML representation with secrets redacted. Used by
    /// `zpic config show` and `doctor` output.
    pub fn redacted_toml(&self) -> String {
        let mut clone = self.clone();
        for uploader in clone.uploaders.values_mut() {
            uploader.redact_secrets();
        }
        toml::to_string_pretty(&clone).unwrap_or_else(|_| String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSection {
    #[serde(default)]
    pub strategy: RenameStrategy,
    /// Optional path template. Overrides `strategy` when set.
    pub path: Option<String>,
    /// When true, the source file name is preserved (equivalent to
    /// `RenameStrategy::DateName`).
    #[serde(default)]
    pub keep_original_name: bool,
}

impl Default for RenameSection {
    fn default() -> Self {
        Self {
            strategy: RenameStrategy::default(),
            path: None,
            keep_original_name: false,
        }
    }
}

impl RenameSection {
    /// Resolve the effective template string. `path` (if set) wins over the
    /// built-in strategy; `keep_original_name` switches to a name-based
    /// strategy.
    pub fn effective_template(&self) -> String {
        if let Some(p) = &self.path {
            return p.clone();
        }
        if self.keep_original_name {
            return "images/{yyyy}/{mm}/{dd}/{name}.{ext}".to_string();
        }
        self.strategy.template().to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatSection {
    pub markdown: Option<String>,
    pub html: Option<String>,
    pub jsx: Option<String>,
    pub url: Option<String>,
}

impl FormatSection {
    pub fn template_for(&self, format: OutputFormat) -> Option<&str> {
        match format {
            OutputFormat::Markdown => self.markdown.as_deref(),
            OutputFormat::Html => self.html.as_deref(),
            OutputFormat::Jsx => self.jsx.as_deref(),
            OutputFormat::Url => self.url.as_deref(),
            OutputFormat::Json => None,
        }
    }
}

/// A single named uploader entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploaderSection {
    /// Built-in kind: `local`, `github`, `s3`.
    #[serde(rename = "type")]
    pub kind: UploaderKind,
    /// Display name / alias used by the PicGo compatibility layer.
    #[serde(default)]
    pub alias: Option<String>,
    /// Free-form fields per uploader type. We keep them as a TOML value so
    /// we don't have to enumerate every backend-specific field here.
    #[serde(flatten)]
    pub fields: BTreeMap<String, toml::Value>,
}

impl UploaderSection {
    /// Replace any field whose name suggests a secret with the literal
    /// `"<redacted>"`. Used when rendering the config for human display.
    pub fn redact_secrets(&mut self) {
        let secret_keys = [
            "token",
            "secret",
            "secret_access_key",
            "secretAccessKey",
            "password",
            "access_key_secret",
        ];
        for key in &secret_keys {
            for (k, v) in self.fields.iter_mut() {
                if k.to_ascii_lowercase().contains(key) {
                    *v = toml::Value::String("<redacted>".to_string());
                }
            }
        }
    }

    /// Resolve a string value: env-var expansion (`$FOO` or `${FOO}`) takes
    /// precedence; otherwise the raw value is returned. When the env var is
    /// missing, the original literal is preserved.
    pub fn resolve_string(&self, key: &str) -> Option<String> {
        let v = self.fields.get(key)?;
        let raw = match v {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        Some(expand_env(&raw))
    }

    /// Convenience accessor for `endpoint` / `repo` / etc. Returns an empty
    /// string when the field is absent so callers can chain `.to_string()`.
    pub fn string_field(&self, key: &str) -> String {
        self.fields
            .get(key)
            .map(|v| match v {
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    }
}

/// Expand `$VAR` and `${VAR}` references in a string. Unknown vars are
/// left as-is. This is the same syntax used by PicGo.
pub fn expand_env(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            if let Some(after) = peek_env_ref(input, i + 1) {
                let (name, consumed) = after;
                if let Ok(value) = std::env::var(&name) {
                    out.push_str(&value);
                } else {
                    out.push('$');
                    if input.as_bytes()[i + 1] == b'{' {
                        out.push('{');
                    }
                    out.push_str(&name);
                    if input.as_bytes()[i + 1] == b'{' {
                        out.push('}');
                    }
                }
                i += 1 + consumed;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn peek_env_ref(s: &str, start: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    if bytes[start] == b'{' {
        let close = s[start + 1..].find('}')?;
        let name = &s[start + 1..start + 1 + close];
        return Some((name.to_string(), 1 + close + 1));
    }
    if bytes[start].is_ascii_alphabetic() || bytes[start] == b'_' {
        let mut end = start + 1;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        return Some((s[start..end].to_string(), end - start));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_toml() {
        let toml = r#"
            default_uploader = "r2"
            default_format = "markdown"

            [rename]
            strategy = "date-hash"
            path = "images/{yyyy}/{mm}/{dd}/{hash8}.{ext}"

            [format]
            markdown = "![{alt}]({url})"

            [uploaders.r2]
            type = "s3"
            endpoint = "https://r2.example.com"
            bucket = "blog"
            public_base_url = "https://cdn.example.com"
        "#;
        let parsed: ZpicConfigFile = toml::from_str(toml).unwrap();
        assert_eq!(parsed.default_uploader.as_deref(), Some("r2"));
        assert_eq!(parsed.uploaders.len(), 1);
        assert!(parsed.redacted_toml().contains("r2"));
    }

    #[test]
    fn redacts_secrets() {
        let toml = r#"
            [uploaders.gh]
            type = "github"
            token = "ghp_secretvalue"
        "#;
        let parsed: ZpicConfigFile = toml::from_str(toml).unwrap();
        let redacted = parsed.redacted_toml();
        assert!(!redacted.contains("ghp_secretvalue"));
        assert!(redacted.contains("<redacted>"));
    }

    #[test]
    fn expand_env_keeps_unknown() {
        let out = expand_env("hello $NOT_SET");
        assert_eq!(out, "hello $NOT_SET");
    }

    #[test]
    fn expand_env_handles_braces() {
        std::env::set_var("ZPIC_TEST_VAR", "world");
        let out = expand_env("hello ${ZPIC_TEST_VAR}");
        assert_eq!(out, "hello world");
    }

    #[test]
    fn effective_template_prefers_path() {
        let section = RenameSection {
            strategy: zpic_core::config::RenameStrategy::DateHash,
            path: Some("custom/{name}.{ext}".to_string()),
            keep_original_name: false,
        };
        assert_eq!(section.effective_template(), "custom/{name}.{ext}");
    }
}
