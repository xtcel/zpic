## MODIFIED Requirements

### Requirement: Doctor validates local setup
The CLI SHALL provide `zpic doctor` to check config discovery, active uploader credentials or plugin validation, plugin discovery health, clipboard availability, and history-store writability, and it SHALL report pass or fail per subsystem.

#### Scenario: All checks pass
- **WHEN** the user runs `zpic doctor` with valid config, credentials, clipboard support, writable local storage, and a valid active uploader implementation
- **THEN** the command reports each subsystem as passing
- **THEN** the command exits successfully

#### Scenario: A credential check fails
- **WHEN** the active uploader is missing required credentials
- **THEN** the command marks that subsystem as failed
- **THEN** the command prints an actionable fix message

#### Scenario: Active plugin uploader fails validation
- **WHEN** the active uploader is provided by a plugin and the plugin manifest, runtime load, or plugin config validation fails
- **THEN** the command marks that subsystem as failed
- **THEN** the command prints an actionable fix message that identifies the failing plugin or uploader type
