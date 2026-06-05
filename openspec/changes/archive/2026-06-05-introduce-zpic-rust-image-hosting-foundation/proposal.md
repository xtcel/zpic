## Why

`zpic` needs a Rust-native foundation that can replace Node/PicGo runtime dependencies without forcing users to rebuild their image-hosting setup from scratch. We need a CLI-first product contract now so upload workflows, PicGo migration, and future Zed/MCP integrations can share one stable implementation.

## What Changes

- Introduce the Rust workspace foundation for `zpic`, centered on reusable upload, config, history, and formatting modules rather than a CLI-only implementation.
- Add CLI upload workflows for local files and clipboard images, including multi-file input, uploader selection, path templating, and human-readable or JSON output.
- Add PicGo configuration compatibility so `zpic` can discover existing PicGo config files, use supported uploader settings directly, and import them into native `zpic` TOML.
- Add Markdown migration commands that scan local image references, upload those assets, and optionally rewrite documents with dry-run and report modes.
- Add history and diagnostics commands so users can inspect prior uploads, verify credentials and clipboard availability, and troubleshoot setup issues quickly.
- Define stable non-interactive command contracts that future Zed and MCP adapters can call without embedding uploader logic.

## Capabilities

### New Capabilities

- `image-upload-cli`: Upload local files or clipboard images through first-party uploaders and return deterministic formatted results.
- `picgo-config-compatibility`: Discover, parse, and import supported PicGo configuration into native `zpic` configuration.
- `upload-history-and-diagnostics`: Persist upload records and expose diagnostic checks for config, credentials, clipboard, and local storage.
- `markdown-image-migration`: Find local Markdown image references, upload them, and provide dry-run, rewrite, and reporting flows.
- `integration-entrypoints`: Expose non-interactive CLI and JSON contracts that editor and agent integrations can call safely.

### Modified Capabilities

- None.

## Impact

- Adds a new Rust workspace with core crates for CLI, config loading, uploader implementations, history storage, and shared data models.
- Introduces user-facing commands, config locations, JSON schemas, and filesystem conventions that future integrations will rely on.
- Adds new dependencies for async runtime, CLI parsing, config serialization, HTTP uploads, clipboard access, and SQLite-backed history.
- Establishes the contract that future `zpic-zed` and `zpic-mcp` changes will build on instead of implementing their own upload logic.
