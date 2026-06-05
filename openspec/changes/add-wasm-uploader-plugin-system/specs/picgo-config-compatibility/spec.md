## MODIFIED Requirements

### Requirement: Supported PicGo uploaders can be used directly
The compatibility layer SHALL detect PicGo-formatted configuration files and resolve the active uploader from the `picBed` section when that uploader has either a native built-in `zpic` implementation or an installed `zpic` plugin implementation mapped through a declared alias.

#### Scenario: Use a supported PicGo GitHub configuration
- **WHEN** the active PicGo uploader is `github` and its required fields are present
- **THEN** `zpic upload` uses those GitHub settings without requiring the user to rewrite the config first

#### Scenario: Use a supported PicGo plugin-alias configuration
- **WHEN** the active PicGo uploader name matches a PicGo alias declared by an installed `zpic` uploader plugin
- **THEN** `zpic upload` resolves that uploader through the installed `zpic` plugin
- **THEN** the user does not need to rename the uploader type before uploading

### Requirement: Unsupported PicGo plugins are rejected safely
The compatibility layer SHALL reject PicGo uploader plugins that do not have a corresponding built-in or installed `zpic` implementation and SHALL return an actionable error naming the unsupported uploader.

#### Scenario: Encounter an unsupported PicGo plugin uploader
- **WHEN** the active PicGo uploader is provided only by a PicGo plugin with no corresponding built-in or installed `zpic` implementation
- **THEN** the command exits with a non-zero status
- **THEN** the error message names the unsupported uploader and instructs the user to switch uploaders, install a corresponding `zpic` plugin, or import a supported configuration
