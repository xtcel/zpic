## 1. Workspace Foundation

- [x] 1.1 Create the Rust workspace and initial crate layout for `zpic-core`, `zpic-config`, `zpic-cli`, and `zpic-history`.
- [x] 1.2 Add shared dependencies, linting, and test scaffolding for async uploads, serialization, clipboard access, and SQLite storage.
- [x] 1.3 Define shared error types, result models, and config-discovery utilities that all commands can reuse.

## 2. Upload and Output Flow

- [x] 2.1 Implement `zpic upload` argument parsing for file paths, clipboard mode, uploader overrides, config overrides, and output selection.
- [x] 2.2 Implement the shared uploader abstraction plus first-party `local`, `github`, and `s3-compatible` uploader backends.
- [x] 2.3 Implement path-template rendering, metadata extraction, and deterministic human-readable and JSON upload results.
- [x] 2.4 Add clipboard upload support with platform-aware diagnostics when image data cannot be read.

## 3. PicGo Compatibility

- [x] 3.1 Implement config discovery precedence across explicit `zpic` config sources and PicGo fallback files.
- [x] 3.2 Implement PicGo config parsing for supported built-in uploaders and active `picBed` resolution.
- [x] 3.3 Implement `zpic config import-picgo` to write native TOML without mutating the source PicGo file.
- [x] 3.4 Add actionable validation errors for unsupported PicGo plugin uploaders and invalid config fields.

## 4. History, Diagnostics, and Migration

- [x] 4.1 Persist successful uploads in SQLite and add `zpic history list` with uploader-based filtering.
- [x] 4.2 Implement `zpic doctor` checks for config presence, uploader credentials, clipboard availability, and history-store writability.
- [x] 4.3 Implement Markdown image discovery, dry-run summaries, report output, and safe rewrite mode for local image references.
- [x] 4.4 Add structured JSON output for `migrate` and `doctor` so integrations can consume results without parsing text.

## 5. Verification and Integration Contract

- [x] 5.1 Document the stable non-interactive CLI contract and JSON payloads that future Zed and MCP adapters will call.
- [x] 5.2 Add end-to-end tests that cover upload success and failure, config fallback, PicGo import, migration dry-run, and doctor output.
- [x] 5.3 Prepare follow-on adapter hooks or command examples for Zed slash commands and MCP tool invocation without implementing those adapters yet.
