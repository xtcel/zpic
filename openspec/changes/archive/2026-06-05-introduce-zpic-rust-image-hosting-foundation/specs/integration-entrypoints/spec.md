## ADDED Requirements

### Requirement: Core commands are non-interactive
The `upload`, `migrate`, `doctor`, and `config import-picgo` commands SHALL accept the inputs required for automation through arguments and flags and SHALL NOT require interactive prompts to complete.

#### Scenario: Editor integration calls upload directly
- **WHEN** an editor or agent process runs `zpic upload ./cover.png --uploader r2 --format markdown --json`
- **THEN** the command completes without requesting interactive input from a TTY
- **THEN** the caller can determine success from the process exit code and returned payload

### Requirement: Integration commands support stable JSON payloads
The `upload`, `migrate`, and `doctor` commands SHALL provide a `--json` mode with deterministic top-level payload shapes for successful and failed executions.

#### Scenario: Upload returns structured metadata
- **WHEN** a caller runs `zpic upload ./cover.png --json`
- **THEN** the JSON payload includes upload result items with source path, URL, key, uploader, and metadata fields

#### Scenario: Doctor returns structured checks
- **WHEN** a caller runs `zpic doctor --json`
- **THEN** the JSON payload includes one structured result per subsystem check with status and message fields

### Requirement: Process behavior is deterministic for callers
Integration-facing commands SHALL use exit code `0` only when every requested operation succeeds, SHALL use non-zero exit codes when any requested operation fails, and SHALL reserve `stderr` for diagnostics rather than payload data.

#### Scenario: Command succeeds
- **WHEN** an integration caller runs a command whose requested work completes successfully
- **THEN** the process exits with code `0`
- **THEN** the requested payload is available on `stdout`

#### Scenario: Command fails
- **WHEN** an integration caller runs a command and any required operation fails
- **THEN** the process exits with a non-zero status
- **THEN** the failure details are emitted as diagnostics without corrupting the expected payload channel
