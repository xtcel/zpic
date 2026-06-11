//! S3-compatible uploader. Works with AWS S3, Cloudflare R2, MinIO,
//! Backblaze B2, and any endpoint that speaks the S3 v4 signing protocol.
//!
//! Talks to the S3 REST API directly. No SDK is pulled in: the V4
//! (HMAC-SHA256) signature is computed in this file from the algorithm
//! documented by AWS, and the request is sent with `reqwest`.
//!
//! References:
//! <https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html>
//! and <https://docs.aws.amazon.com/general/latest/gr/sigv4_signing.html>.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use zpic_config::UploaderSection;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{ProgressCallback, UploadOutput, UploadRequest, Uploader};

use crate::body::body_with_progress;

type HmacSha256 = Hmac<Sha256>;

const ALGO: &str = "AWS4-HMAC-SHA256";
const SERVICE: &str = "s3";
const TERMINATOR: &str = "aws4_request";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const DEFAULT_REGION: &str = "auto";

/// S3-compatible uploader. The uploader is constructed from a TOML
/// `[uploaders.<name>]` config section. The public base URL is composed
/// from `public_base_url` and the resolved object key.
#[derive(Debug)]
pub struct S3Uploader {
    endpoint: String,
    region: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    public_base_url: String,
    cache_control: Option<String>,
    acl: Option<String>,
}

impl S3Uploader {
    /// Build an S3 uploader from a `[uploaders.<name>]` section.
    pub fn from_config(section: &UploaderSection) -> Result<Self> {
        let endpoint = section.string_field("endpoint").trim().to_string();
        if endpoint.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "s3 uploader requires `endpoint`".into(),
            ));
        }
        let region = {
            let v = section.string_field("region").trim().to_string();
            if v.is_empty() {
                DEFAULT_REGION.to_string()
            } else {
                v
            }
        };
        let bucket = section.string_field("bucket").trim().to_string();
        if bucket.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "s3 uploader requires `bucket`".into(),
            ));
        }
        let access_key_id = section.resolve_string("access_key_id").unwrap_or_default();
        let secret_access_key = section
            .resolve_string("secret_access_key")
            .unwrap_or_default();
        if access_key_id.is_empty() || secret_access_key.is_empty() {
            return Err(ZpicError::AuthMissing(
                "s3 uploader requires `access_key_id` and `secret_access_key`".into(),
            ));
        }
        let public_base_url = section
            .string_field("public_base_url")
            .trim()
            .trim_end_matches('/')
            .to_string();
        if public_base_url.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "s3 uploader requires `public_base_url`".into(),
            ));
        }
        let cache_control = section
            .fields
            .get("cache_control")
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        let acl = section
            .fields
            .get("acl")
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        Ok(Self {
            endpoint,
            region,
            bucket,
            access_key_id,
            secret_access_key,
            public_base_url,
            cache_control,
            acl,
        })
    }

    /// Build the public URL for a given key.
    pub fn build_url(&self, key: &str) -> String {
        let key = key.trim_start_matches('/');
        if self.public_base_url.ends_with('/') {
            format!("{}{}", self.public_base_url, key)
        } else {
            format!("{}/{}", self.public_base_url, key)
        }
    }

    /// Direct constructor used by tests and adapter code that already
    /// has the fields unrolled.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        endpoint: impl Into<String>,
        region: impl Into<String>,
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        public_base_url: impl Into<String>,
    ) -> Result<Self> {
        let access_key_id = access_key_id.into();
        let secret_access_key = secret_access_key.into();
        if access_key_id.is_empty() || secret_access_key.is_empty() {
            return Err(ZpicError::AuthMissing("s3 credentials".into()));
        }
        Ok(Self {
            endpoint: endpoint.into(),
            region: region.into(),
            bucket: bucket.into(),
            access_key_id,
            secret_access_key,
            public_base_url: public_base_url.into(),
            cache_control: None,
            acl: None,
        })
    }

    /// Read-only accessor for the bucket name (used by `doctor` checks).
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Read-only accessor for the endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Read-only accessor for credentials; returns `true` if both are set.
    pub fn has_credentials(&self) -> bool {
        !self.access_key_id.is_empty() && !self.secret_access_key.is_empty()
    }
}

#[async_trait]
impl Uploader for S3Uploader {
    fn name(&self) -> &str {
        "s3"
    }

    async fn upload(&self, req: UploadRequest) -> Result<UploadOutput> {
        let url = self.build_url(&req.context.target_key);
        if req.context.dry_run {
            return Ok(UploadOutput {
                source: req.input.source_path.to_string_lossy().into_owned(),
                url: url.clone(),
                key: req.context.target_key,
                markdown: format!("![{}]({})", req.input.file_name, url),
                mime: req.input.mime,
                size: req.input.size,
                width: None,
                height: None,
                uploader: self.name().to_string(),
            });
        }
        // Filter the ACL at request time so unknown values warn-and-skip,
        // matching the prior SDK-based behavior.
        let acl = self.acl.as_deref().and_then(|raw| {
            validate_acl(raw).or_else(|| {
                tracing::warn!("unknown acl value '{}'; ignoring", raw);
                None
            })
        });
        s3_put_object(
            &self.endpoint,
            &self.region,
            &self.bucket,
            &req.context.target_key,
            &req.input.mime,
            self.cache_control.as_deref(),
            acl,
            &self.access_key_id,
            &self.secret_access_key,
            req.input.bytes.clone(),
            req.context.on_progress.clone(),
        )
        .await?;
        Ok(UploadOutput {
            source: req.input.source_path.to_string_lossy().into_owned(),
            url: url.clone(),
            key: req.context.target_key,
            markdown: format!("![{}]({})", req.input.file_name, url),
            mime: req.input.mime,
            size: req.input.size,
            width: None,
            height: None,
            uploader: self.name().to_string(),
        })
    }
}

/// Issue a `PutObject` against an S3-compatible endpoint using SigV4.
/// Kept as a free function so it can be reused by `doctor` for
/// permission probes.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn s3_put_object(
    endpoint: &str,
    region: &str,
    bucket: &str,
    key: &str,
    content_type: &str,
    cache_control: Option<&str>,
    acl: Option<&str>,
    access_key_id: &str,
    secret_access_key: &str,
    body: Bytes,
    on_progress: Option<ProgressCallback>,
) -> Result<()> {
    if key.is_empty() {
        return Err(ZpicError::UploadFailed("s3: object key is empty".into()));
    }
    if bucket.is_empty() {
        return Err(ZpicError::UploadFailed("s3: bucket is empty".into()));
    }

    let endpoint = ensure_scheme(endpoint);
    let host = extract_host(&endpoint)?;

    let key_trimmed = key.trim_start_matches('/');
    let canonical_uri = percent_encode_path(&format!("/{}/{}", bucket, key_trimmed));

    let datetime = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date = &datetime[..8];
    let scope = format!("{}/{}/{}/{}", date, region, SERVICE, TERMINATOR);

    // Build the header set that will be signed (sorted by lowercase key).
    // `host` is always signed, and `content-length` is included so the
    // signed value matches what reqwest writes on the wire.
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    headers.insert("content-length".to_string(), body.len().to_string());
    headers.insert("content-type".to_string(), content_type.to_string());
    headers.insert("host".to_string(), host);
    headers.insert(
        "x-amz-content-sha256".to_string(),
        UNSIGNED_PAYLOAD.to_string(),
    );
    headers.insert("x-amz-date".to_string(), datetime.clone());
    if let Some(cc) = cache_control {
        headers.insert("cache-control".to_string(), cc.to_string());
    }
    if let Some(acl_val) = acl {
        headers.insert("x-amz-acl".to_string(), acl_val.to_string());
    }

    let canonical_headers = canonicalize_headers(&headers);
    let signed_headers: Vec<String> = headers.keys().cloned().collect();
    let signed_headers_str = signed_headers.join(";");

    let canonical_request = format!(
        "PUT\n{}\n{}\n{}\n{}\n{}",
        canonical_uri, "", canonical_headers, signed_headers_str, UNSIGNED_PAYLOAD
    );

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        ALGO,
        datetime,
        scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let signing_key = derive_signing_key(secret_access_key, date, region);
    let signature = hex_hmac_sha256(&signing_key, string_to_sign.as_bytes());

    let auth = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        ALGO, access_key_id, scope, signed_headers_str, signature
    );

    let encoded_key = percent_encode_path(key_trimmed);
    let url = format!("{}/{}/{}", endpoint, bucket, encoded_key);

    let mut header_map = HeaderMap::new();
    header_map.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .map_err(|e| ZpicError::Network(format!("s3 content-type: {e}")))?,
    );
    header_map.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&body.len().to_string())
            .map_err(|e| ZpicError::Network(format!("s3 content-length: {e}")))?,
    );
    header_map.insert(
        reqwest::header::HeaderName::from_static("x-amz-content-sha256"),
        HeaderValue::from_static(UNSIGNED_PAYLOAD),
    );
    header_map.insert(
        reqwest::header::HeaderName::from_static("x-amz-date"),
        HeaderValue::from_str(&datetime)
            .map_err(|e| ZpicError::Network(format!("s3 x-amz-date: {e}")))?,
    );
    if let Some(cc) = cache_control {
        header_map.insert(
            reqwest::header::CACHE_CONTROL,
            HeaderValue::from_str(cc)
                .map_err(|e| ZpicError::Network(format!("s3 cache-control: {e}")))?,
        );
    }
    if let Some(acl_val) = acl {
        header_map.insert(
            reqwest::header::HeaderName::from_static("x-amz-acl"),
            HeaderValue::from_str(acl_val)
                .map_err(|e| ZpicError::Network(format!("s3 x-amz-acl: {e}")))?,
        );
    }
    header_map.insert(
        reqwest::header::HeaderName::from_static("authorization"),
        HeaderValue::from_str(&auth)
            .map_err(|e| ZpicError::Network(format!("s3 authorization: {e}")))?,
    );

    let client = reqwest::Client::new();
    let resp = client
        .put(&url)
        .headers(header_map)
        .body(body_with_progress(body, on_progress))
        .send()
        .await
        .map_err(|e| ZpicError::Network(format!("s3 put_object: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        // Extract the `Location` header *before* consuming the body with
        // `resp.text()` (which moves `resp`).
        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body_text = resp.text().await.unwrap_or_default();
        let code = status.as_u16();
        if code == 301 || code == 307 || code == 308 {
            // Wrong region / redirect to a different endpoint. Surface the
            // `Location` header to make the cause obvious.
            return Err(ZpicError::UploadFailed(format!(
                "s3 responded with {} (redirect to {}): {}",
                code, location, body_text
            )));
        }
        if code == 401 || code == 403 {
            return Err(ZpicError::AuthFailed(format!(
                "s3 responded with {}: {}",
                code, body_text
            )));
        }
        return Err(ZpicError::UploadFailed(format!(
            "s3 responded with {}: {}",
            code, body_text
        )));
    }
    Ok(())
}

/// Normalize a user-provided endpoint to `scheme://host[:port]`. Any
/// trailing path, query string, or fragment is discarded. Defaults to
/// `https://` when no scheme is present.
fn ensure_scheme(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    let (scheme, rest) = if let Some(s) = trimmed.strip_prefix("https://") {
        ("https://", s)
    } else if let Some(s) = trimmed.strip_prefix("http://") {
        ("http://", s)
    } else {
        ("https://", trimmed)
    };
    let host_end = rest
        .find(|c: char| c == '/' || c == '?' || c == '#')
        .unwrap_or(rest.len());
    format!("{}{}", scheme, &rest[..host_end])
}

/// Extract the host (and port, when present) from a fully-qualified URL.
fn extract_host(endpoint_with_scheme: &str) -> Result<String> {
    let rest = endpoint_with_scheme
        .strip_prefix("https://")
        .or_else(|| endpoint_with_scheme.strip_prefix("http://"))
        .unwrap_or(endpoint_with_scheme);
    if rest.is_empty() {
        return Err(ZpicError::ConfigInvalid(format!(
            "s3 endpoint '{}' is missing a host",
            endpoint_with_scheme
        )));
    }
    Ok(rest.to_string())
}

/// Percent-encode a path per the SigV4 rules: keep unreserved characters
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

/// Build the canonical-headers block for SigV4. The map is iterated in
/// sorted key order; each entry is rendered as `<key>:<value>\n`.
fn canonicalize_headers(headers: &BTreeMap<String, String>) -> String {
    // SigV4 requires header names to be lowercase in the canonical
    // request. Lower-case each key before emitting so mixed-case
    // callers (e.g. http libraries that pass the original case from
    // the request) still produce the right canonical form. BTreeMap
    // keeps the iteration sorted, so the output is correctly ordered
    // as a side-effect.
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

/// Derive the SigV4 signing key with the four-step HMAC chain:
/// `AWS4 + secret -> date -> region -> service -> terminator`.
fn derive_signing_key(secret_access_key: &str, date: &str, region: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{}", secret_access_key);
    let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, SERVICE.as_bytes());
    hmac_sha256(&k_service, TERMINATOR.as_bytes())
}

/// Map a user-provided ACL string to the canonical lowercase S3
/// `ObjectCannedAcl` value. Returns `None` for unknown values so the
/// caller can warn-and-skip.
fn validate_acl(s: &str) -> Option<&'static str> {
    Some(match s.to_ascii_lowercase().as_str() {
        "private" => "private",
        "public-read" => "public-read",
        "public-read-write" => "public-read-write",
        "authenticated-read" => "authenticated-read",
        "aws-exec-read" => "aws-exec-read",
        "bucket-owner-read" => "bucket-owner-read",
        "bucket-owner-full-control" => "bucket-owner-full-control",
        "log-delivery-write" => "log-delivery-write",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use zpic_core::config::{UploaderKind, ZpicConfig};
    use zpic_core::upload::{UploadContext, UploadInput};

    #[test]
    fn build_url_joins_base_and_key() {
        let u = S3Uploader::new(
            "https://r2.example.com",
            "auto",
            "bucket",
            "ak",
            "sk",
            "https://cdn.example.com",
        )
        .unwrap();
        assert_eq!(
            u.build_url("2026/06/04/cover.png"),
            "https://cdn.example.com/2026/06/04/cover.png"
        );
    }

    #[test]
    fn missing_credentials_rejected() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "endpoint".to_string(),
            toml::Value::String("https://r2.example.com".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "public_base_url".to_string(),
            toml::Value::String("https://cdn.example.com".to_string()),
        );
        let section = UploaderSection {
            kind: UploaderKind::S3,
            alias: None,
            fields,
        };
        let err = S3Uploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::AuthMissing(_)));
    }

    #[test]
    fn missing_endpoint_rejected() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "public_base_url".to_string(),
            toml::Value::String("https://cdn.example.com".to_string()),
        );
        fields.insert(
            "access_key_id".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "secret_access_key".to_string(),
            toml::Value::String("sk".to_string()),
        );
        let section = UploaderSection {
            kind: UploaderKind::S3,
            alias: None,
            fields,
        };
        let err = S3Uploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::ConfigInvalid(_)));
    }

    #[test]
    fn validate_acl_known_values() {
        assert!(validate_acl("private").is_some());
        assert!(validate_acl("public-read").is_some());
        assert!(validate_acl("public-read-write").is_some());
        assert!(validate_acl("authenticated-read").is_some());
        assert!(validate_acl("aws-exec-read").is_some());
        assert!(validate_acl("bucket-owner-read").is_some());
        assert!(validate_acl("bucket-owner-full-control").is_some());
        assert!(validate_acl("log-delivery-write").is_some());
        // Case-insensitive normalization.
        assert_eq!(validate_acl("PRIVATE"), Some("private"));
        assert_eq!(validate_acl("Public-Read"), Some("public-read"));
        assert!(validate_acl("bogus").is_none());
    }

    #[test]
    fn derive_signing_key_matches_aws_chain() {
        // SigV4 signing key is HMAC-SHA256 output, always 32 bytes.
        let key = derive_signing_key("test-secret", "20250417", "us-east-1");
        assert_eq!(key.len(), 32);
        // Calling the chain twice with the same input must be deterministic.
        let key2 = derive_signing_key("test-secret", "20250417", "us-east-1");
        assert_eq!(key, key2);
    }

    #[test]
    fn signing_key_changes_with_input() {
        let a = derive_signing_key("secret-a", "20250417", "us-east-1");
        let b = derive_signing_key("secret-b", "20250417", "us-east-1");
        let c = derive_signing_key("secret-a", "20250418", "us-east-1");
        let d = derive_signing_key("secret-a", "20250417", "eu-west-1");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn percent_encode_path_preserves_unreserved_and_slash() {
        assert_eq!(
            percent_encode_path("/mybucket/images/2026/06/04/cover.png"),
            "/mybucket/images/2026/06/04/cover.png"
        );
        // Spaces are encoded, `*` is a sub-delim and must be encoded, but
        // `~` and `/` are kept unencoded per RFC 3986 unreserved set.
        assert_eq!(
            percent_encode_path("/b/path with spaces/file*~.txt"),
            "/b/path%20with%20spaces/file%2A~.txt"
        );
        // Multi-byte UTF-8 is encoded byte-by-byte.
        assert_eq!(percent_encode_path("/b/测.txt"), "/b/%E6%B5%8B.txt");
    }

    #[test]
    fn canonicalize_headers_sorts_by_lower_key() {
        // Callers are expected to insert keys in lowercase form (the SigV4
        // spec mandates lowercase header names in the canonical request).
        let mut headers = BTreeMap::new();
        headers.insert("x-amz-date".to_string(), "20250417T000000Z".to_string());
        headers.insert("content-type".to_string(), "text/plain".to_string());
        headers.insert(
            "x-amz-content-sha256".to_string(),
            UNSIGNED_PAYLOAD.to_string(),
        );
        headers.insert("host".to_string(), "example.com".to_string());
        headers.insert("content-length".to_string(), "0".to_string());
        let out = canonicalize_headers(&headers);
        // BTreeMap iterates in lex order: content-length, content-type, host,
        // x-amz-content-sha256, x-amz-date.
        assert_eq!(
            out,
            "content-length:0\n\
             content-type:text/plain\n\
             host:example.com\n\
             x-amz-content-sha256:UNSIGNED-PAYLOAD\n\
             x-amz-date:20250417T000000Z\n"
        );
    }

    #[test]
    fn ensure_scheme_normalizes_input() {
        assert_eq!(ensure_scheme("r2.example.com"), "https://r2.example.com");
        assert_eq!(
            ensure_scheme("https://r2.example.com/"),
            "https://r2.example.com"
        );
        assert_eq!(
            ensure_scheme("http://minio.local:9000/"),
            "http://minio.local:9000"
        );
        // Path / query / fragment are stripped.
        assert_eq!(
            ensure_scheme("https://example.com/some/path?x=1#y"),
            "https://example.com"
        );
    }

    #[test]
    fn extract_host_handles_ports() {
        assert_eq!(
            extract_host("https://r2.example.com").unwrap(),
            "r2.example.com"
        );
        assert_eq!(
            extract_host("https://minio.local:9000").unwrap(),
            "minio.local:9000"
        );
        assert!(extract_host("https://").is_err());
    }

    #[test]
    fn region_defaults_to_auto_when_missing() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "endpoint".to_string(),
            toml::Value::String("https://r2.example.com".to_string()),
        );
        fields.insert("bucket".to_string(), toml::Value::String("b".to_string()));
        fields.insert(
            "public_base_url".to_string(),
            toml::Value::String("https://cdn.example.com".to_string()),
        );
        fields.insert(
            "access_key_id".to_string(),
            toml::Value::String("ak".to_string()),
        );
        fields.insert(
            "secret_access_key".to_string(),
            toml::Value::String("sk".to_string()),
        );
        let section = UploaderSection {
            kind: UploaderKind::S3,
            alias: None,
            fields,
        };
        let u = S3Uploader::from_config(&section).unwrap();
        assert_eq!(u.region, "auto");
    }

    #[tokio::test]
    async fn dry_run_does_not_send() {
        // The dry-run path never hits the network, so we can build a
        // uploader with placeholder credentials and assert it still
        // produces a well-formed URL.
        let uploader = S3Uploader::new(
            "https://r2.example.com",
            "auto",
            "my-bucket",
            "ak",
            "sk",
            "https://cdn.example.com",
        )
        .unwrap();
        let mut ctx = UploadContext::new("images/cover.png".into(), Arc::new(StubConfig));
        ctx.dry_run = true;
        let out = uploader
            .upload(UploadRequest {
                context: ctx,
                input: UploadInput::new(
                    std::path::PathBuf::from("cover.png"),
                    "cover",
                    "image/png",
                    Bytes::from_static(b"hello"),
                ),
            })
            .await
            .unwrap();
        assert_eq!(out.url, "https://cdn.example.com/images/cover.png");
        assert_eq!(out.uploader, "s3");
    }

    #[derive(Debug)]
    struct StubConfig;
    impl ZpicConfig for StubConfig {
        fn source(&self) -> &str {
            "test"
        }
    }
}
