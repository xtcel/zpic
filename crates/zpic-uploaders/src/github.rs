//! GitHub contents uploader. PUTs a base64-encoded blob to the
//! `repos/{owner}/{repo}/contents/{path}` API and returns a CDN URL.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use serde_json::json;

use zpic_config::UploaderSection;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{UploadOutput, UploadRequest, Uploader};

const DEFAULT_USER_AGENT: &str = "zpic/0.1";

/// Upload a file to a GitHub repository's contents API.
#[derive(Debug)]
pub struct GitHubUploader {
    repo: String,
    branch: String,
    token: String,
    public_base_url: String,
    path_prefix: String,
    client: reqwest::Client,
}

impl GitHubUploader {
    /// Construct a GitHub uploader from a `[uploaders.<name>]` section.
    pub fn from_config(section: &UploaderSection) -> Result<Self> {
        let repo = section.string_field("repo").trim().to_string();
        if repo.is_empty() || !repo.contains('/') {
            return Err(ZpicError::ConfigInvalid(
                "github uploader requires `repo` in 'owner/repo' form".into(),
            ));
        }
        let branch = {
            let v = section.string_field("branch");
            if v.is_empty() {
                "master".to_string()
            } else {
                v
            }
        };
        let token = section.resolve_string("token").unwrap_or_default();
        if token.is_empty() {
            return Err(ZpicError::AuthMissing(
                "github uploader requires `token` (set GITHUB_TOKEN or configure `token`)".into(),
            ));
        }
        let public_base_url = section
            .field("public_base_url")
            .or_else(|| section.field("customUrl"))
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
            .to_string();
        let path_prefix = section
            .field("path_prefix")
            .or_else(|| section.field("path"))
            .unwrap_or_default()
            .trim()
            .trim_matches('/')
            .to_string();
        let client = reqwest::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| ZpicError::Network(e.to_string()))?;
        Ok(Self {
            repo,
            branch,
            token,
            public_base_url,
            path_prefix,
            client,
        })
    }

    /// Construct with an explicit token (used by tests).
    #[allow(dead_code)]
    pub fn new(
        repo: impl Into<String>,
        branch: impl Into<String>,
        token: impl Into<String>,
        public_base_url: impl Into<String>,
    ) -> Result<Self> {
        let token = token.into();
        if token.is_empty() {
            return Err(ZpicError::AuthMissing("github token".into()));
        }
        let client = reqwest::Client::builder()
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map_err(|e| ZpicError::Network(e.to_string()))?;
        Ok(Self {
            repo: repo.into(),
            branch: branch.into(),
            token,
            public_base_url: public_base_url.into(),
            path_prefix: String::new(),
            client,
        })
    }

    fn storage_key(&self, key: &str) -> String {
        let key = key.trim_start_matches('/');
        if self.path_prefix.is_empty() {
            return key.to_string();
        }
        format!("{}/{}", self.path_prefix, key)
    }

    fn build_url(&self, key: &str) -> String {
        let key = self.storage_key(key);
        if self.public_base_url.ends_with('/') {
            format!("{}{}", self.public_base_url, key)
        } else if !self.public_base_url.is_empty() {
            format!("{}/{}", self.public_base_url, key)
        } else {
            format!(
                "https://raw.githubusercontent.com/{}/{}/{}",
                self.repo, self.branch, key
            )
        }
    }

    fn api_url(&self, key: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/contents/{}",
            self.repo,
            self.storage_key(key)
        )
    }

    fn output_url(&self, key: &str, download_url: Option<String>) -> String {
        if self.public_base_url.is_empty() {
            download_url.unwrap_or_else(|| self.build_url(key))
        } else {
            self.build_url(key)
        }
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", self.token)) {
            h.insert(AUTHORIZATION, v);
        }
        h.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        h.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        h.insert(
            "Accept",
            HeaderValue::from_static("application/vnd.github+json"),
        );
        h
    }
}

#[async_trait]
impl Uploader for GitHubUploader {
    fn name(&self) -> &str {
        "github"
    }

    async fn upload(&self, req: UploadRequest) -> Result<UploadOutput> {
        let storage_key = self.storage_key(&req.context.target_key);
        let url = self.build_url(&req.context.target_key);
        if req.context.dry_run {
            return Ok(UploadOutput {
                source: req.input.source_path.to_string_lossy().into_owned(),
                url: url.clone(),
                key: storage_key,
                markdown: format!("![{}]({})", req.input.file_name, url),
                mime: req.input.mime,
                size: req.input.size,
                width: None,
                height: None,
                uploader: self.name().to_string(),
            });
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(&req.input.bytes);
        let body = json!({
            "message": format!("zpic: upload {}", req.input.file_name),
            "branch": self.branch,
            "content": encoded,
        });
        let api = self.api_url(&req.context.target_key);
        let resp = self
            .client
            .put(&api)
            .headers(self.auth_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| ZpicError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                return Err(ZpicError::AuthFailed(format!(
                    "github responded with {}: {}",
                    status, body
                )));
            }
            if status == reqwest::StatusCode::CONFLICT {
                return Err(ZpicError::UploadFailed(format!(
                    "github: file already exists at {} (use a new path or remove the existing one)",
                    req.context.target_key
                )));
            }
            if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                let url = self.build_url(&req.context.target_key);
                return Ok(UploadOutput {
                    source: req.input.source_path.to_string_lossy().into_owned(),
                    url: url.clone(),
                    key: storage_key,
                    markdown: format!("![{}]({})", req.input.file_name, url),
                    mime: req.input.mime,
                    size: req.input.size,
                    width: None,
                    height: None,
                    uploader: self.name().to_string(),
                });
            }
            return Err(ZpicError::UploadFailed(format!(
                "github: HTTP {}: {}",
                status, body
            )));
        }
        let body: GitHubContentResponse = resp
            .json()
            .await
            .map_err(|e| ZpicError::UploadFailed(format!("github: invalid response: {e}")))?;
        let url = self.output_url(
            &req.context.target_key,
            body.content.and_then(|content| content.download_url),
        );
        Ok(UploadOutput {
            source: req.input.source_path.to_string_lossy().into_owned(),
            url: url.clone(),
            key: storage_key,
            markdown: format!("![{}]({})", req.input.file_name, url),
            mime: req.input.mime,
            size: req.input.size,
            width: None,
            height: None,
            uploader: self.name().to_string(),
        })
    }
}

#[allow(dead_code)]
fn _force_arc<T>(t: T) -> Arc<T> {
    Arc::new(t)
}

#[derive(Debug, Deserialize)]
struct GitHubContentResponse {
    #[serde(default)]
    content: Option<GitHubContent>,
}

#[derive(Debug, Deserialize)]
struct GitHubContent {
    #[serde(default)]
    download_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_joins_base_and_key() {
        let u = GitHubUploader::new("o/r", "main", "t", "https://cdn.jsdelivr.net/gh/o/r").unwrap();
        assert_eq!(
            u.build_url("images/2026/06/04/cover.png"),
            "https://cdn.jsdelivr.net/gh/o/r/images/2026/06/04/cover.png"
        );
    }

    #[test]
    fn build_url_handles_trailing_slash() {
        let u = GitHubUploader::new("o/r", "main", "t", "https://example.com/").unwrap();
        assert_eq!(u.build_url("x.png"), "https://example.com/x.png");
    }

    #[test]
    fn build_url_falls_back_to_raw_github_when_custom_url_missing() {
        let u = GitHubUploader::new("o/r", "main", "t", "").unwrap();
        assert_eq!(
            u.build_url("x.png"),
            "https://raw.githubusercontent.com/o/r/main/x.png"
        );
    }

    #[test]
    fn from_config_accepts_optional_custom_url_and_path_prefix() {
        let mut section_map = std::collections::BTreeMap::new();
        section_map.insert("repo".to_string(), toml::Value::String("o/r".to_string()));
        section_map.insert(
            "branch".to_string(),
            toml::Value::String("main".to_string()),
        );
        section_map.insert(
            "token".to_string(),
            toml::Value::String("ghp_x".to_string()),
        );
        section_map.insert(
            "path_prefix".to_string(),
            toml::Value::String("img/".to_string()),
        );
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::Github,
            alias: None,
            fields: section_map,
        };
        let uploader = GitHubUploader::from_config(&section).unwrap();
        assert_eq!(
            uploader.build_url("cover.png"),
            "https://raw.githubusercontent.com/o/r/main/img/cover.png"
        );
    }

    #[test]
    fn missing_token_rejected() {
        let mut section_map = std::collections::BTreeMap::new();
        section_map.insert("repo".to_string(), toml::Value::String("o/r".to_string()));
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::Github,
            alias: None,
            fields: section_map,
        };
        let err = GitHubUploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::AuthMissing(_)));
    }
}
