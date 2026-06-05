use zpic_config::{UploaderConfigItem, UploaderSection};
use zpic_core::config::UploaderKind;
use zpic_core::error::Result;
use zpic_core::upload::Uploader;
use zpic_plugins::{builtin_uploader_descriptor, UploaderDescriptor, UploaderFieldSchema};

use crate::github::GitHubUploader;
use crate::local::LocalUploader;
use crate::s3::S3Uploader;

pub fn builtin_uploader_descriptors() -> Vec<UploaderDescriptor> {
    vec![
        builtin_uploader_descriptor(
            "local",
            "local",
            vec!["local".into()],
            vec![
                field("target_dir", "Target directory", true, false, None),
                field("public_base_url", "Public base URL", true, false, None),
            ],
            instantiate_local,
            validate_local,
        ),
        builtin_uploader_descriptor(
            "github",
            "github",
            vec!["github".into()],
            vec![
                field("repo", "GitHub repo (owner/repo)", true, false, None),
                field("branch", "Branch", true, false, Some("master")),
                field("token", "GitHub token", true, true, None),
                field("path_prefix", "Path prefix", false, false, None),
                field("public_base_url", "Custom public base URL", false, false, None),
            ],
            instantiate_github,
            validate_github,
        ),
        builtin_uploader_descriptor(
            "s3",
            "s3",
            vec!["s3".into(), "aws-s3".into(), "r2".into()],
            vec![
                field("endpoint", "S3 endpoint", true, false, None),
                field("region", "Region", false, false, Some("auto")),
                field("bucket", "Bucket", true, false, None),
                field("access_key_id", "Access key ID", true, false, None),
                field("secret_access_key", "Secret access key", true, true, None),
                field("public_base_url", "Public base URL", true, false, None),
                field("cache_control", "Cache-Control", false, false, None),
                field("acl", "ACL", false, false, None),
            ],
            instantiate_s3,
            validate_s3,
        ),
    ]
}

fn field(
    key: &str,
    label: &str,
    required: bool,
    secret: bool,
    default: Option<&str>,
) -> UploaderFieldSchema {
    UploaderFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        required,
        secret,
        default: default.map(str::to_string),
    }
}

fn instantiate_local(uploader_type: &str, item: &UploaderConfigItem) -> Result<Box<dyn Uploader>> {
    let section = builtin_section(item, uploader_type, UploaderKind::Local)?;
    Ok(Box::new(LocalUploader::from_config(&section)?))
}

fn validate_local(uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
    let section = builtin_section(item, uploader_type, UploaderKind::Local)?;
    LocalUploader::from_config(&section).map(|_| ())
}

fn instantiate_github(
    uploader_type: &str,
    item: &UploaderConfigItem,
) -> Result<Box<dyn Uploader>> {
    let section = builtin_section(item, uploader_type, UploaderKind::Github)?;
    Ok(Box::new(GitHubUploader::from_config(&section)?))
}

fn validate_github(uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
    let section = builtin_section(item, uploader_type, UploaderKind::Github)?;
    GitHubUploader::from_config(&section).map(|_| ())
}

fn instantiate_s3(uploader_type: &str, item: &UploaderConfigItem) -> Result<Box<dyn Uploader>> {
    let section = builtin_section(item, uploader_type, UploaderKind::S3)?;
    Ok(Box::new(S3Uploader::from_config(&section)?))
}

fn validate_s3(uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
    let section = builtin_section(item, uploader_type, UploaderKind::S3)?;
    S3Uploader::from_config(&section).map(|_| ())
}

fn builtin_section(
    item: &UploaderConfigItem,
    uploader_type: &str,
    fallback_kind: UploaderKind,
) -> Result<UploaderSection> {
    item.to_uploader_section_for_type_with_fallback(uploader_type, Some(fallback_kind))
}
