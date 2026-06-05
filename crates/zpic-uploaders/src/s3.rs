//! S3-compatible uploader. Works with AWS S3, Cloudflare R2, MinIO,
//! Backblaze B2, and any endpoint that speaks the S3 v4 signing protocol.

use async_trait::async_trait;
use bytes::Bytes;

use zpic_config::UploaderSection;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{UploadOutput, UploadRequest, Uploader};

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
            let v = section.string_field("region");
            if v.is_empty() {
                "auto".to_string()
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
        s3_put_object(
            &self.endpoint,
            &self.region,
            &self.bucket,
            &req.context.target_key,
            &req.input.mime,
            self.cache_control.as_deref(),
            self.acl.as_deref(),
            &self.access_key_id,
            &self.secret_access_key,
            req.input.bytes.clone(),
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

/// Issue a `PutObject` against an S3-compatible endpoint. Kept as a free
/// function so it can be reused by `doctor` for permission probes.
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
) -> Result<()> {
    use aws_credential_types::Credentials;
    use aws_sdk_s3::config::Region;
    use aws_sdk_s3::primitives::ByteStream;
    use aws_sdk_s3::Client;

    let creds = Credentials::new(access_key_id, secret_access_key, None, None, "zpic");
    let shared = aws_config::BehaviorVersion::latest();
    let cfg = aws_config::defaults(shared)
        .region(Region::new(region.to_string()))
        .endpoint_url(endpoint)
        .credentials_provider(creds)
        .load()
        .await;
    let client = Client::new(&cfg);

    let mut op = client
        .put_object()
        .bucket(bucket)
        .key(key)
        .content_type(content_type)
        .body(ByteStream::from(body));
    if let Some(cc) = cache_control {
        op = op.cache_control(cc);
    }
    if let Some(acl_str) = acl {
        if let Some(parsed) = parse_acl(acl_str) {
            op = op.acl(parsed);
        } else {
            tracing::warn!("unknown acl value '{}'; ignoring", acl_str);
        }
    }
    op.send()
        .await
        .map_err(|e| ZpicError::Network(format!("s3 put_object: {e}")))?;
    Ok(())
}

/// Map a TOML-side ACL string to the AWS SDK's `ObjectCannedAcl` enum.
fn parse_acl(s: &str) -> Option<aws_sdk_s3::types::ObjectCannedAcl> {
    use aws_sdk_s3::types::ObjectCannedAcl;
    Some(match s.to_ascii_lowercase().as_str() {
        "private" => ObjectCannedAcl::Private,
        "public-read" => ObjectCannedAcl::PublicRead,
        "public-read-write" => ObjectCannedAcl::PublicReadWrite,
        "authenticated-read" => ObjectCannedAcl::AuthenticatedRead,
        "aws-exec-read" => ObjectCannedAcl::AwsExecRead,
        "bucket-owner-read" => ObjectCannedAcl::BucketOwnerRead,
        "bucket-owner-full-control" => ObjectCannedAcl::BucketOwnerFullControl,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            kind: zpic_core::config::UploaderKind::S3,
            alias: None,
            fields,
        };
        let err = S3Uploader::from_config(&section).unwrap_err();
        assert!(matches!(err, ZpicError::AuthMissing(_)));
    }

    #[test]
    fn parse_acl_known_values() {
        assert!(parse_acl("private").is_some());
        assert!(parse_acl("public-read").is_some());
        assert!(parse_acl("bogus").is_none());
    }
}
