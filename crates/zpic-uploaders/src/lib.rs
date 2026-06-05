//! First-party uploader implementations: local filesystem, GitHub contents
//! API, and S3-compatible object storage.

pub mod factory;
pub mod github;
pub mod local;
pub mod s3;

pub use factory::{build_uploader, UploaderFactory};
pub use github::GitHubUploader;
pub use local::LocalUploader;
pub use s3::S3Uploader;
