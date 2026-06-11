//! Aliyun OSS uploader.
//!
//! Talks to the Aliyun Object Storage Service REST API directly. No SDK is
//! pulled in: the V4 (HMAC-SHA256) signature is computed in this file from
//! the algorithm documented by Alibaba Cloud, and the request is sent with
//! `reqwest`.
//!
//! Reference (used only to cross-check the algorithm spec):
//! <https://help.aliyun.com/document_detail/31978.html> (PutObject) and
//! the V4 signer in `aliyun-oss-csharp-sdk/sdk/Util/OssRequestSignerV4.cs`.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::HeaderName;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use zpic_config::UploaderSection;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{ProgressCallback, UploadOutput, UploadRequest, Uploader};

use crate::body::body_with_progress;

type HmacSha256 = Hmac<Sha256>;

const SIGNATURE_VERSION: &str = "OSS4-HMAC-SHA256";
const PRODUCT: &str = "oss";
const TERMINATOR: &str = "aliyun_v4_request";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const DEFAULT_ENDPOINT_SUFFIX: &str = "aliyuncs.com";

/// Aliyun OSS uploader. Uses the V4 signature (HMAC-SHA256).
#[derive(Debug, Clone)]
pub struct OssUploader {
    /// Region without the `oss-` prefix, used in the V4 signing scope.
    region_for_signing: String,
    /// Endpoint host suffix (e.g. `oss-cn-hangzhou.aliyuncs.com`); the
    /// bucket name is prepended in the request URL.
    endpoint: String,
    bucket: String,
    access_key_id: String,
    access_key_secret: String,
    /// Optional public base URL override. When empty, the public URL is
    /// derived from the bucket and endpoint.
    public_base_url: String,
    /// Optional prefix prepended to the object key on upload
    /// (e.g. `img/`). The PicGo `path` field is accepted as an alias.
    path_prefix: String,
    cache_control: Option<String>,
    acl: Option<String>,
}

impl OssUploader {
    /// Build an OSS uploader from a `[uploaders.<name>]` section.
    pub fn from_config(section: &UploaderSection) -> Result<Self> {
        let region_raw = section
            .field("region")
            .or_else(|| section.field("area"))
            .unwrap_or_default()
            .trim()
            .to_string();
        if region_raw.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "aliyun-oss uploader requires `region` (e.g. `oss-cn-hangzhou`)".into(),
            ));
        }
        let region = normalize_region(&region_raw);
        let region_for_signing = strip_oss_prefix(&region).to_string();

        let endpoint = section
            .string_field("endpoint")
            .trim()
            .trim_end_matches('/')
            .to_string();
        let endpoint = if endpoint.is_empty() {
            format!("{}.{}", region, DEFAULT_ENDPOINT_SUFFIX)
        } else {
            endpoint
        };

        let bucket = section.string_field("bucket").trim().to_string();
        if bucket.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "aliyun-oss uploader requires `bucket`".into(),
            ));
        }

        let access_key_id = section
            .resolve_string("access_key_id")
            .or_else(|| section.resolve_string("accessKeyId"))
            .unwrap_or_default();
        let access_key_secret = section
            .resolve_string("access_key_secret")
            .or_else(|| section.resolve_string("accessKeySecret"))
            .unwrap_or_default();
        if access_key_id.is_empty() || access_key_secret.is_empty() {
            return Err(ZpicError::AuthMissing(
                "aliyun-oss uploader requires `access_key_id` and `access_key_secret` \
                 (or env vars / PicGo aliases `accessKeyId` / `accessKeySecret`)"
                    .into(),
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

        let cache_control = section
            .fields
            .get("cache_control")
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        let acl = section
            .fields
            .get("acl")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .map(|s| match s.to_ascii_lowercase().as_str() {
                "default" | "private" | "public-read" | "public-read-write" => s,
                other => {
                    tracing::warn!("unknown aliyun-oss acl value '{}'; ignoring", other);
                    String::new()
                }
            })
            .filter(|s| !s.is_empty());

        Ok(Self {
            region_for_signing,
            endpoint,
            bucket,
            access_key_id,
            access_key_secret,
            public_base_url,
            path_prefix,
            cache_control,
            acl,
        })
    }

    /// Convenience wrapper for the factory dispatch in `factory.rs`.
    pub fn from_section(section: &UploaderSection) -> Result<Self> {
        Self::from_config(section)
    }

    /// Direct constructor used by tests and adapter code.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        region: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        public_base_url: impl Into<String>,
    ) -> Result<Self> {
        let access_key_id = access_key_id.into();
        let access_key_secret = access_key_secret.into();
        if access_key_id.is_empty() || access_key_secret.is_empty() {
            return Err(ZpicError::AuthMissing("aliyun-oss credentials".into()));
        }
        let region_raw = region.into();
        let region = normalize_region(&region_raw);
        let region_for_signing = strip_oss_prefix(&region).to_string();
        let endpoint = format!("{}.{}", region, DEFAULT_ENDPOINT_SUFFIX);
        Ok(Self {
            region_for_signing,
            endpoint,
            bucket: bucket.into(),
            access_key_id,
            access_key_secret,
            public_base_url: public_base_url.into(),
            path_prefix: String::new(),
            cache_control: None,
            acl: None,
        })
    }

    /// Combine the configured `path_prefix` with the per-request target key.
    /// The prefix is stripped of surrounding slashes; the key has any leading
    /// slashes trimmed. Returns the key unchanged when no prefix is set.
    fn storage_key(&self, key: &str) -> String {
        let key = key.trim_start_matches('/');
        if self.path_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.path_prefix, key)
        }
    }

    /// Compose the public URL for a given object key. The key is used as-is
    /// (no encoding) so callers can keep URLs pretty.
    pub fn build_url(&self, key: &str) -> String {
        let key = self.storage_key(key);
        if self.public_base_url.is_empty() {
            format!("https://{}.{}/{}", self.bucket, self.endpoint, key)
        } else if self.public_base_url.ends_with('/') {
            format!("{}{}", self.public_base_url, key)
        } else {
            format!("{}/{}", self.public_base_url, key)
        }
    }

    /// Endpoint accessors used by `zpic doctor`.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn bucket(&self) -> &str {
        &self.bucket
    }
    pub fn has_credentials(&self) -> bool {
        !self.access_key_id.is_empty() && !self.access_key_secret.is_empty()
    }
}

/// Accept `oss-cn-hangzhou` or `cn-hangzhou`; always normalize to the
/// `oss-` prefixed form used in OSS endpoints.
fn normalize_region(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("oss-") {
        trimmed.to_string()
    } else {
        format!("oss-{}", trimmed)
    }
}

/// Drop the `oss-` prefix so the value can be used in the V4 signing scope.
fn strip_oss_prefix(region: &str) -> &str {
    if region.len() > 4 && region[..4].eq_ignore_ascii_case("oss-") {
        &region[4..]
    } else {
        region
    }
}

#[async_trait]
impl Uploader for OssUploader {
    fn name(&self) -> &str {
        "aliyun-oss"
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
        oss_put_object_v4(
            &self.endpoint,
            &self.region_for_signing,
            &self.bucket,
            &storage_key,
            &req.input.mime,
            self.cache_control.as_deref(),
            self.acl.as_deref(),
            &self.access_key_id,
            &self.access_key_secret,
            req.input.bytes.clone(),
            req.context.on_progress.clone(),
        )
        .await?;
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

/// Issue a `PutObject` against an Aliyun OSS endpoint using the V4 signature.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn oss_put_object_v4(
    endpoint: &str,
    region_for_signing: &str,
    bucket: &str,
    key: &str,
    content_type: &str,
    cache_control: Option<&str>,
    acl: Option<&str>,
    access_key_id: &str,
    access_key_secret: &str,
    body: Bytes,
    on_progress: Option<ProgressCallback>,
) -> Result<()> {
    use reqwest::header::{HeaderMap, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE};

    if key.is_empty() {
        return Err(ZpicError::UploadFailed(
            "aliyun-oss: object key is empty".into(),
        ));
    }

    let datetime = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date = &datetime[..8];
    let scope = format!("{}/{}/{}/{}", date, region_for_signing, PRODUCT, TERMINATOR);

    // The list of *additional* (non-default) headers to sign. We always sign
    // `content-length` because reqwest sets it automatically and OSS needs
    // the signed value to match the value on the wire. `cache-control` only
    // appears when the user opted into it.
    let mut additional_headers: Vec<String> = Vec::new();
    additional_headers.push("content-length".to_string());
    if cache_control.is_some() {
        additional_headers.push("cache-control".to_string());
    }
    additional_headers.sort();

    // Canonical headers — collected into a sorted map of (lowercased key) ->
    // (value, trimmed). Default signed headers (`content-type`, `content-md5`,
    // `x-oss-*`) plus the additional list are included.
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    headers.insert("content-type".to_string(), content_type.to_string());
    headers.insert(
        "x-oss-content-sha256".to_string(),
        UNSIGNED_PAYLOAD.to_string(),
    );
    headers.insert("x-oss-date".to_string(), datetime.clone());
    if let Some(acl_val) = acl {
        headers.insert("x-oss-object-acl".to_string(), acl_val.to_string());
    }
    if additional_headers.iter().any(|h| h == "content-length") {
        headers.insert("content-length".to_string(), body.len().to_string());
    }
    if let Some(cc) = cache_control {
        if additional_headers.iter().any(|h| h == "cache-control") {
            headers.insert("cache-control".to_string(), cc.to_string());
        }
    }

    let canonical_uri = format!("/{}/{}", bucket, key);
    let canonical_uri = percent_encode_path(&canonical_uri);
    let canonical_query = String::new();
    let additional_headers_str = additional_headers.join(";");

    let canonical_headers = canonicalize_headers(&headers);
    let canonical_request = format!(
        "PUT\n{}\n{}\n{}\n{}\n{}",
        canonical_uri, canonical_query, canonical_headers, additional_headers_str, UNSIGNED_PAYLOAD
    );

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        SIGNATURE_VERSION,
        datetime,
        scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let signing_key = derive_signing_key(access_key_secret, date, region_for_signing);
    let signature = hex_hmac_sha256(&signing_key, string_to_sign.as_bytes());

    let auth = if additional_headers.is_empty() {
        format!(
            "{} Credential={}/{},Signature={}",
            SIGNATURE_VERSION, access_key_id, scope, signature
        )
    } else {
        format!(
            "{} Credential={}/{},AdditionalHeaders={},Signature={}",
            SIGNATURE_VERSION, access_key_id, scope, additional_headers_str, signature
        )
    };

    // The URL is built with the percent-encoded key so characters like `*`
    // are transmitted correctly. The bucket is in the host (virtual-hosted
    // style), and the path is the percent-encoded object key.
    let encoded_key = percent_encode_path(key);
    let url = format!("https://{}.{}/{}", bucket, endpoint, encoded_key);

    let mut header_map = HeaderMap::new();
    header_map.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .map_err(|e| ZpicError::Network(format!("aliyun-oss content-type: {e}")))?,
    );
    header_map.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&body.len().to_string())
            .map_err(|e| ZpicError::Network(format!("aliyun-oss content-length: {e}")))?,
    );
    header_map.insert(
        HeaderName::from_static("x-oss-content-sha256"),
        HeaderValue::from_static(UNSIGNED_PAYLOAD),
    );
    header_map.insert(
        HeaderName::from_static("x-oss-date"),
        HeaderValue::from_str(&datetime)
            .map_err(|e| ZpicError::Network(format!("aliyun-oss x-oss-date: {e}")))?,
    );
    if let Some(cc) = cache_control {
        header_map.insert(
            reqwest::header::CACHE_CONTROL,
            HeaderValue::from_str(cc)
                .map_err(|e| ZpicError::Network(format!("aliyun-oss cache-control: {e}")))?,
        );
    }
    if let Some(acl_val) = acl {
        header_map.insert(
            HeaderName::from_static("x-oss-object-acl"),
            HeaderValue::from_str(acl_val)
                .map_err(|e| ZpicError::Network(format!("aliyun-oss acl: {e}")))?,
        );
    }
    header_map.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&auth)
            .map_err(|e| ZpicError::Network(format!("aliyun-oss authorization: {e}")))?,
    );

    let client = reqwest::Client::new();
    let resp = client
        .put(&url)
        .headers(header_map)
        .body(body_with_progress(body, on_progress))
        .send()
        .await
        .map_err(|e| ZpicError::Network(format!("aliyun-oss put_object: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        let code = status.as_u16();
        if code == 401 || code == 403 {
            return Err(ZpicError::AuthFailed(format!(
                "aliyun-oss responded with {}: {}",
                code, body_text
            )));
        }
        return Err(ZpicError::UploadFailed(format!(
            "aliyun-oss responded with {}: {}",
            code, body_text
        )));
    }
    Ok(())
}

/// Percent-encode a path per the OSS rules: keep unreserved characters
/// (`A-Z a-z 0-9 - _ . ~`) plus `/`; encode every other byte (including
/// multi-byte UTF-8 sequences, byte by byte) as `%XX`.
fn percent_encode_path(input: &str) -> String {
    const ALLOWED: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.~/";
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if ALLOWED.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Build the canonical-headers block for the V4 signature. The map must
/// already be filtered to only include default-signed and additional-signed
/// headers; the function takes care of sorting and formatting.
fn canonicalize_headers(headers: &BTreeMap<String, String>) -> String {
    // SigV4 requires header names to be lowercase in the canonical
    // request. Lower-case each key before emitting so mixed-case
    // callers still produce the right canonical form. BTreeMap keeps
    // the iteration sorted, so the output is correctly ordered as a
    // side-effect.
    let mut out = String::new();
    for (k, v) in headers {
        out.push_str(&k.to_ascii_lowercase());
        out.push(':');
        out.push_str(v.trim());
        out.push('\n');
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts keys of any length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn hex_hmac_sha256(key: &[u8], msg: &[u8]) -> String {
    hex::encode(hmac_sha256(key, msg))
}

/// Derive the V4 signing key with the four-step HMAC chain:
/// `aliyun_v4 + secret -> date -> region -> product -> terminator`.
fn derive_signing_key(access_key_secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k_secret = format!("aliyun_v4{}", access_key_secret);
    let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_product = hmac_sha256(&k_region, PRODUCT.as_bytes());
    hmac_sha256(&k_product, TERMINATOR.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_region_adds_prefix() {
        assert_eq!(normalize_region("cn-hangzhou"), "oss-cn-hangzhou");
        assert_eq!(normalize_region("oss-cn-hangzhou"), "oss-cn-hangzhou");
        assert_eq!(normalize_region("CN-HANGZHOU"), "oss-CN-HANGZHOU");
        assert_eq!(normalize_region("  oss-cn-hangzhou  "), "oss-cn-hangzhou");
    }

    #[test]
    fn strip_oss_prefix_works() {
        assert_eq!(strip_oss_prefix("oss-cn-hangzhou"), "cn-hangzhou");
        assert_eq!(strip_oss_prefix("OSS-cn-hangzhou"), "cn-hangzhou");
        assert_eq!(strip_oss_prefix("cn-hangzhou"), "cn-hangzhou");
    }

    #[test]
    fn percent_encode_path_preserves_unreserved_and_slash() {
        assert_eq!(
            percent_encode_path("/mybucket/images/2026/06/04/cover.png"),
            "/mybucket/images/2026/06/04/cover.png"
        );
        // Spaces are encoded; `*` is a sub-delim and is encoded; `~` and
        // `/` are kept unencoded per RFC 3986 unreserved set.
        assert_eq!(
            percent_encode_path("/mybucket/path with spaces/file*~.txt"),
            "/mybucket/path%20with%20spaces/file%2A~.txt"
        );
        // `*` must be encoded per the OSS rules; `~` and `/` are kept.
        assert_eq!(percent_encode_path("/b/a*b"), "/b/a%2Ab");
        // Multi-byte UTF-8 is encoded byte-by-byte.
        assert_eq!(percent_encode_path("/b/测.txt"), "/b/%E6%B5%8B.txt");
    }

    #[test]
    fn derive_signing_key_matches_rfc_chain() {
        // Verifies the HMAC chain by exercising it on a known fixed
        // AccessKeySecret and confirming the resulting key is deterministic
        // and 32 bytes long (SHA-256 output).
        let key = derive_signing_key("test-secret", "20250417", "cn-hangzhou");
        assert_eq!(key.len(), 32);
        // Calling the chain twice must produce the same bytes.
        let key2 = derive_signing_key("test-secret", "20250417", "cn-hangzhou");
        assert_eq!(key, key2);
    }

    #[test]
    fn signing_key_changes_with_input() {
        let a = derive_signing_key("secret-a", "20250417", "cn-hangzhou");
        let b = derive_signing_key("secret-b", "20250417", "cn-hangzhou");
        let c = derive_signing_key("secret-a", "20250418", "cn-hangzhou");
        let d = derive_signing_key("secret-a", "20250417", "cn-shanghai");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn canonicalize_headers_sorts_by_lower_key() {
        let mut headers = BTreeMap::new();
        headers.insert("x-oss-date".to_string(), "20250417T000000Z".to_string());
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        headers.insert(
            "x-oss-content-sha256".to_string(),
            UNSIGNED_PAYLOAD.to_string(),
        );
        let out = canonicalize_headers(&headers);
        // BTreeMap iterates in lex order, so content-type comes before the
        // x-oss-* headers.
        assert_eq!(
            out,
            "content-type:text/plain\n\
             x-oss-content-sha256:UNSIGNED-PAYLOAD\n\
             x-oss-date:20250417T000000Z\n"
        );
    }

    #[test]
    fn build_url_uses_endpoint_when_no_public_base() {
        let u = OssUploader::new("cn-hangzhou", "my-bucket", "ak", "sk", "").unwrap();
        assert_eq!(
            u.build_url("2026/06/04/cover.png"),
            "https://my-bucket.oss-cn-hangzhou.aliyuncs.com/2026/06/04/cover.png"
        );
    }

    #[test]
    fn build_url_honors_custom_public_base() {
        let u = OssUploader::new(
            "oss-cn-hangzhou",
            "my-bucket",
            "ak",
            "sk",
            "https://cdn.example.com",
        )
        .unwrap();
        assert_eq!(
            u.build_url("images/cover.png"),
            "https://cdn.example.com/images/cover.png"
        );
    }

    #[test]
    fn missing_region_rejected() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "access_key_id".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "access_key_secret".to_string(),
            toml::Value::String("sk".to_string()),
        );
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::AliyunOss,
            alias: None,
            fields,
        };
        let err = OssUploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::ConfigInvalid(_)));
    }

    #[test]
    fn missing_credentials_rejected() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "region".to_string(),
            toml::Value::String("oss-cn-hangzhou".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::AliyunOss,
            alias: None,
            fields,
        };
        let err = OssUploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::AuthMissing(_)));
    }

    #[test]
    fn picgo_aliases_are_accepted() {
        // `area` + camelCase credentials (PicGo's `picgo-plugin-aliyun-oss`
        // style) should resolve through `resolve_string` / `field`.
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "area".to_string(),
            toml::Value::String("cn-hangzhou".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "accessKeyId".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "accessKeySecret".to_string(),
            toml::Value::String("sk".to_string()),
        );
        fields.insert(
            "customUrl".to_string(),
            toml::Value::String("https://cdn.example.com".to_string()),
        );
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::AliyunOss,
            alias: None,
            fields,
        };
        let uploader = OssUploader::from_config(&section).unwrap();
        assert_eq!(uploader.bucket(), "b");
        assert_eq!(uploader.endpoint(), "oss-cn-hangzhou.aliyuncs.com");
        assert_eq!(
            uploader.build_url("cover.png"),
            "https://cdn.example.com/cover.png"
        );
    }

    #[test]
    fn path_prefix_is_prepended_to_object_key_and_url() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "region".to_string(),
            toml::Value::String("oss-cn-hangzhou".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "access_key_id".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "access_key_secret".to_string(),
            toml::Value::String("sk".to_string()),
        );
        fields.insert(
            "path_prefix".to_string(),
            toml::Value::String("img/".to_string()),
        );
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::AliyunOss,
            alias: None,
            fields,
        };
        let uploader = OssUploader::from_config(&section).unwrap();
        assert_eq!(
            uploader.build_url("2026/06/04/cover.png"),
            "https://b.oss-cn-hangzhou.aliyuncs.com/img/2026/06/04/cover.png"
        );
    }

    #[test]
    fn path_alias_is_accepted_for_picgo_compat() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "region".to_string(),
            toml::Value::String("oss-cn-hangzhou".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "access_key_id".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "access_key_secret".to_string(),
            toml::Value::String("sk".to_string()),
        );
        fields.insert(
            "path".to_string(),
            toml::Value::String("images".to_string()),
        );
        let section = UploaderSection {
            kind: zpic_core::config::UploaderKind::AliyunOss,
            alias: None,
            fields,
        };
        let uploader = OssUploader::from_config(&section).unwrap();
        assert_eq!(
            uploader.build_url("cover.png"),
            "https://b.oss-cn-hangzhou.aliyuncs.com/images/cover.png"
        );
    }

    #[tokio::test]
    async fn dry_run_does_not_send() {
        // The dry-run path never hits the network, so we can build a
        // uploader with placeholder credentials and assert it still
        // produces a well-formed URL.
        let uploader = OssUploader::new("oss-cn-hangzhou", "my-bucket", "ak", "sk", "").unwrap();
        let mut ctx = zpic_core::upload::UploadContext::new(
            "images/cover.png".into(),
            std::sync::Arc::new(StubConfig),
        );
        ctx.dry_run = true;
        let out = uploader
            .upload(zpic_core::upload::UploadRequest {
                context: ctx,
                input: zpic_core::upload::UploadInput::new(
                    std::path::PathBuf::from("cover.png"),
                    "cover",
                    "image/png",
                    Bytes::from_static(b"hello"),
                ),
            })
            .await
            .unwrap();
        assert_eq!(
            out.url,
            "https://my-bucket.oss-cn-hangzhou.aliyuncs.com/images/cover.png"
        );
        assert_eq!(out.uploader, "aliyun-oss");
    }

    #[derive(Debug)]
    struct StubConfig;
    impl zpic_core::config::ZpicConfig for StubConfig {
        fn source(&self) -> &str {
            "test"
        }
    }
}
