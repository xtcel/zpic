## 1. Plugin Foundations

- [x] 1.1 Add a new workspace crate for plugin manifests, discovery, registry, and runtime abstractions.
- [x] 1.2 Introduce a unified uploader descriptor/registry model that can represent both built-in and plugin uploaders.
- [x] 1.3 Add plugin directory resolution, manifest parsing, and uploader schema metadata support.

## 2. Runtime Integration

- [x] 2.1 Add the Wasmtime-based uploader runtime and the host-plugin request/response contract.
- [x] 2.2 Add focused plugin runtime tests using a fixture plugin module.
- [x] 2.3 Refactor uploader resolution so unknown uploader types no longer fall back to built-in kinds implicitly.

## 3. CLI and Config Integration

- [x] 3.1 Update upload and migrate flows to resolve uploaders through the unified registry.
- [x] 3.2 Update `zpic set uploader` guided setup to include plugin-provided uploader schemas while preserving `--field key=value` support.
- [x] 3.3 Update PicGo config compatibility so installed plugin aliases can satisfy active uploader resolution, but only at the config layer.
- [x] 3.4 Update `zpic doctor` to validate plugin discovery and active plugin uploader health.

## 4. Documentation and Verification

- [x] 4.1 Update user-facing docs to describe the plugin system and the PicGo config-only compatibility boundary.
- [x] 4.2 Add or update integration tests for plugin-backed upload resolution and diagnostics.
- [x] 4.3 Mark the OpenSpec tasks complete after implementation and verification.
