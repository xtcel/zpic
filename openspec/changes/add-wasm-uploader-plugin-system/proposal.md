## Why

`zpic` currently ships only built-in uploaders wired at compile time, which makes every new backend a core-code change. We need a `zpic`-native plugin system so uploader backends can be added independently while keeping the CLI, migration flow, and config model stable.

## What Changes

- Add a WASM-based uploader plugin system for `zpic`, with manifest-driven discovery and runtime loading.
- Introduce a shared uploader registry so built-in and plugin uploaders are resolved through one path.
- Extend guided uploader setup and diagnostics to understand plugin-provided schemas and validation.
- Narrow PicGo compatibility to configuration compatibility only; `zpic` will not attempt to emulate PicGo plugin commands or load PicGo Node plugins.
- Preserve the current CLI and JSON contracts for upload, migrate, history, and doctor while allowing the active uploader to come from a plugin.

## Capabilities

### New Capabilities
- `wasm-uploader-plugins`: Discover, validate, and execute WASM uploader plugins through a manifest-defined contract.

### Modified Capabilities
- `image-upload-cli`: Upload and migration flows can resolve their uploader from either a built-in implementation or an installed plugin.
- `picgo-config-compatibility`: PicGo compatibility is limited to configuration discovery/import, with optional alias mapping to installed `zpic` plugins.
- `upload-history-and-diagnostics`: `zpic doctor` validates plugin discovery, plugin config, and plugin runtime health for the active uploader.

## Impact

- Adds a new plugin runtime/discovery layer and a new workspace crate for plugin management.
- Refactors uploader resolution in `zpic-config`, `zpic-uploaders`, and `zpic-cli` to stop assuming every uploader type is built in.
- Introduces a new WASM runtime dependency and plugin fixture tests.
- Updates docs and OpenSpec contracts to make the PicGo compatibility boundary explicit.
