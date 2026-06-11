//! First-party uploader implementations: local filesystem, GitHub contents
//! API, S3-compatible object storage, and Aliyun OSS.

pub mod body;
pub mod factory;
pub mod github;
pub mod local;
pub mod oss;
pub mod registry;
pub mod s3;

pub use body::body_with_progress;
pub use factory::{build_uploader, UploaderFactory};
pub use github::GitHubUploader;
pub use local::LocalUploader;
pub use oss::OssUploader;
pub use registry::builtin_uploader_descriptors;
pub use s3::S3Uploader;
