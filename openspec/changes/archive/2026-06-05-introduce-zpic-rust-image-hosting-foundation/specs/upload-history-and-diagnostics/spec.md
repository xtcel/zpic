## ADDED Requirements

### Requirement: Successful uploads are recorded in history
The system SHALL persist a history record for each successful upload including timestamp, source path, uploader, object key, URL, rendered output, MIME type, and size.

#### Scenario: Record a successful upload
- **WHEN** an upload completes successfully
- **THEN** the system writes one history record for that upload
- **THEN** the record contains the uploader name, URL, and source metadata

### Requirement: Users can inspect prior uploads
The CLI SHALL provide `zpic history list` to display recorded uploads and support uploader-based filtering.

#### Scenario: List all uploads
- **WHEN** the user runs `zpic history list`
- **THEN** the command returns previously recorded upload entries in reverse chronological order

#### Scenario: Filter history by uploader
- **WHEN** the user runs `zpic history list --uploader github`
- **THEN** the command returns only entries uploaded through `github`

### Requirement: Doctor validates local setup
The CLI SHALL provide `zpic doctor` to check config discovery, active uploader credentials, clipboard availability, and history-store writability, and it SHALL report pass or fail per subsystem.

#### Scenario: All checks pass
- **WHEN** the user runs `zpic doctor` with valid config, credentials, clipboard support, and writable local storage
- **THEN** the command reports each subsystem as passing
- **THEN** the command exits successfully

#### Scenario: A credential check fails
- **WHEN** the active uploader is missing required credentials
- **THEN** the command marks that subsystem as failed
- **THEN** the command prints an actionable fix message

### Requirement: Failures return actionable CLI errors
User-facing commands SHALL return non-zero exit codes on failure and SHALL describe the failed subsystem or operation with a suggested remediation when one is known.

#### Scenario: Upload fails because a token is missing
- **WHEN** the user runs an upload command for a backend that requires an absent token
- **THEN** the command exits with a non-zero status
- **THEN** the error explains which credential is missing and where the user can configure it
