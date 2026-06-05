//! Configuration loading, PicGo compatibility, and env-var resolution.

pub mod loader;
pub mod manager;
pub mod paths;
pub mod picgo;
pub mod zpic;

pub use loader::{ConfigLoader, ConfigSource, LoadedConfig};
pub use manager::{
    format_list_output, format_type_output, resolve_active_field, UploaderConfigManager,
};
pub use paths::{candidate_picgo_paths, candidate_zpic_paths};
pub use picgo::{PicBed, PicGoConfig, PicGoUploaderBlock};
pub use zpic::{
    expand_env, migrate_legacy, FormatSection, PicBedSection, PicBedUploaderMirror, RenameSection,
    UploaderConfigItem, UploaderSection, UploaderTypeConfigs, ZpicConfigFile,
};
