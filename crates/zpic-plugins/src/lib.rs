//! Plugin manifests, discovery, registry, and runtime support for zpic.

mod discovery;
mod manifest;
mod registry;
mod runtime;

pub use discovery::{
    discover_plugin_descriptors, plugin_search_paths, DiscoveryOptions, PluginDiagnostic,
    PluginDiagnosticLevel,
};
pub use manifest::{
    FutureCapabilityManifest, PluginManifest, PluginUploaderManifest, UploaderFieldSchema,
};
pub use registry::{
    builtin_uploader_descriptor, plugin_uploader_descriptor, UploaderDescriptor,
    UploaderDescriptorKind, UploaderRegistry,
};
pub use runtime::{
    WasmConfigValidationRequest, WasmConfigValidationResponse, WasmPluginRunner,
    WasmUploadRequest, WasmUploadResponse,
};
