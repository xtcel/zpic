## ADDED Requirements

### Requirement: Config resolution follows a defined precedence
The system SHALL resolve configuration sources in the following order: explicit `--config`, `ZPIC_CONFIG`, project `.zpic/config.toml`, user `zpic` config, PicGo core config, and PicGo GUI config.

#### Scenario: Explicit config overrides fallback discovery
- **WHEN** the user runs a `zpic` command with `--config /tmp/custom.toml`
- **THEN** the command loads `/tmp/custom.toml`
- **THEN** the command does not continue searching fallback config locations

#### Scenario: PicGo config is used as a fallback source
- **WHEN** no explicit or native `zpic` config file is available and a PicGo core config exists
- **THEN** the command loads the PicGo config through the compatibility layer
- **THEN** the resolved uploader settings come from the active `picBed` configuration

### Requirement: Supported PicGo uploaders can be used directly
The compatibility layer SHALL detect PicGo-formatted configuration files and resolve the active uploader from the `picBed` section when that uploader has a native `zpic` implementation.

#### Scenario: Use a supported PicGo GitHub configuration
- **WHEN** the active PicGo uploader is `github` and its required fields are present
- **THEN** `zpic upload` uses those GitHub settings without requiring the user to rewrite the config first

### Requirement: PicGo config can be imported without mutating the source file
The `zpic config import-picgo` command SHALL convert a supported PicGo configuration into native `zpic` TOML and SHALL NOT modify the source PicGo configuration file.

#### Scenario: Import PicGo config into native TOML
- **WHEN** the user runs `zpic config import-picgo`
- **THEN** the command creates a `zpic` TOML configuration file from the supported PicGo settings
- **THEN** the original PicGo file remains unchanged

### Requirement: Unsupported PicGo plugins are rejected safely
The compatibility layer SHALL reject PicGo uploader plugins that do not have native `zpic` implementations and SHALL return an actionable error naming the unsupported uploader.

#### Scenario: Encounter an unsupported PicGo plugin uploader
- **WHEN** the active PicGo uploader is provided only by a Node plugin with no native `zpic` implementation
- **THEN** the command exits with a non-zero status
- **THEN** the error message names the unsupported uploader and instructs the user to switch uploaders or import a supported configuration
