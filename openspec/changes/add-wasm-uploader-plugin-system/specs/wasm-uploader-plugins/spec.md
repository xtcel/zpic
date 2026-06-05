## ADDED Requirements

### Requirement: zpic discovers uploader plugins from manifest-defined plugin directories
The system SHALL discover uploader plugins from standard `zpic` plugin directories by reading a manifest file and loading the referenced WASM module for each valid plugin.

#### Scenario: Discover a valid uploader plugin
- **WHEN** a plugin directory contains a valid `plugin.toml` manifest and the referenced `plugin.wasm` file
- **THEN** `zpic` registers the uploader types declared by that plugin
- **THEN** those uploader types are available for uploader resolution and guided setup

#### Scenario: Ignore an invalid plugin
- **WHEN** a plugin directory contains a malformed manifest or a missing WASM module
- **THEN** `zpic` does not register that plugin
- **THEN** `zpic doctor` reports the discovery or validation failure with an actionable message

### Requirement: zpic executes uploader plugins through a sandboxed WASM runtime
The system SHALL execute uploader plugins inside a WASM runtime and SHALL exchange structured upload requests and upload results through the host-plugin contract.

#### Scenario: Upload through a plugin uploader
- **WHEN** the active uploader type resolves to an installed plugin uploader
- **THEN** `zpic upload` invokes that plugin through the WASM runtime
- **THEN** the plugin returns the uploaded asset URL, key, uploader identity, and metadata in the same shape required by the host upload pipeline

#### Scenario: Plugin upload fails
- **WHEN** a plugin uploader returns a structured failure
- **THEN** `zpic upload` exits with a non-zero status
- **THEN** the failure is surfaced through the existing error and JSON reporting channels

### Requirement: plugin manifests define uploader schemas and aliases
The system SHALL use plugin manifest metadata to define guided setup fields, display names, and optional PicGo uploader aliases for each plugin uploader type.

#### Scenario: Guided setup uses plugin-defined fields
- **WHEN** the user runs `zpic set uploader` for a plugin uploader type
- **THEN** the guided flow prompts for the fields declared by that plugin manifest
- **THEN** the resulting config is stored in the existing `uploader.<type>.configList` shape

#### Scenario: Plugin advertises a PicGo alias
- **WHEN** a discovered plugin uploader declares a PicGo alias matching the active uploader in a PicGo config
- **THEN** the PicGo compatibility layer resolves that config to the installed `zpic` plugin uploader
- **THEN** `zpic` does not require the user to rewrite the uploader type manually before uploading
