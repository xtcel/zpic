## Why

`zpic` currently operates as a standalone CLI tool, which limits its integration with editors and note-taking applications. To enable seamless image upload workflows in Obsidian (and future editor integrations), we need an HTTP server mode that exposes zpic's upload capabilities through a network API while maintaining compatibility with the PicGo ecosystem.

## What Changes

- Add an HTTP server mode to `zpic` with PicGo-compatible API endpoints for image uploads.
- Implement a dedicated Obsidian plugin that communicates with the zpic server for automatic image uploads on paste and drag-and-drop operations.
- Support both desktop (macOS, Linux, Windows) and mobile (iOS, Android) platforms through HTTP-based communication.
- Add system service integration for macOS and Linux to enable zpic server auto-start on system boot.
- Preserve all existing CLI functionality while adding server mode as a new operational mode.

## Capabilities

### New Capabilities
- `http-server-mode`: Start zpic as an HTTP server exposing upload endpoints compatible with PicGo's API contract.
- `obsidian-integration`: Provide a first-class Obsidian plugin that leverages zpic server for image uploads.
- `system-service-integration`: Enable zpic server to run as a system service with auto-start support.

### Modified Capabilities
- `image-upload-cli`: Existing CLI upload flows remain unchanged; HTTP server is an additive operational mode.
- `picgo-config-compatibility`: Configuration continues to be shared; HTTP server reads the same config as CLI mode.
- `upload-history-and-diagnostics`: History and diagnostics work identically in both CLI and server modes.

## Impact

- Adds HTTP server dependencies (axum, tower, multer) to zpic-cli.
- Introduces a new `zpic server` subcommand with start/stop/status operations.
- Creates a separate Obsidian plugin repository/package for distribution.
- Requires documentation for server deployment, API endpoints, and Obsidian plugin installation.
- Enables future integrations with other editors (Zed, VS Code) using the same HTTP API.

## Success Criteria

- Users can start `zpic server` and upload images via HTTP POST to `/upload`.
- Obsidian plugin successfully uploads pasted and drag-dropped images to configured uploader.
- Plugin works on both desktop and mobile Obsidian installations.
- zpic server can be configured to auto-start on macOS and Linux system boot.
- All existing CLI functionality continues to work without regression.
