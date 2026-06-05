//! Configuration loading, PicGo compatibility, and env-var resolution.

pub mod loader;
pub mod paths;
pub mod picgo;
pub mod zpic;

pub use loader::{ConfigLoader, ConfigSource, LoadedConfig};
pub use paths::{candidate_picgo_paths, candidate_zpic_paths};
pub use picgo::{PicBed, PicGoConfig, PicGoUploaderBlock};
pub use zpic::{FormatSection, RenameSection, UploaderSection, ZpicConfigFile};
