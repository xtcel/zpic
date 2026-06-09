## Phase 1: HTTP Server Foundation

### 1.1 Add HTTP server dependencies
- [ ] Add `axum`, `tower`, `tower-http` to `zpic-cli/Cargo.toml`
- [ ] Add `multer` for multipart form data parsing
- [ ] Add `tempfile` for secure temporary file handling
- [ ] Add `mime` for MIME type detection
- [ ] Verify all dependencies compile without conflicts

### 1.2 Create server module structure
- [ ] Create `crates/zpic-cli/src/server/mod.rs` module
- [ ] Create `crates/zpic-cli/src/server/routes.rs` for route definitions
- [ ] Create `crates/zpic-cli/src/server/handlers.rs` for request handlers
- [ ] Create `crates/zpic-cli/src/server/models.rs` for request/response models
- [ ] Create `crates/zpic-cli/src/server/middleware.rs` for logging and CORS

### 1.3 Implement request/response models
- [ ] Define `UploadRequest` struct for JSON path list
- [ ] Define `UploadResponse` struct with PicGo-compatible fields
- [ ] Define `HealthResponse` struct for health check endpoint
- [ ] Define `ConfigResponse` struct for configuration query
- [ ] Add serialization/deserialization tests

### 1.4 Implement /upload endpoint
- [ ] Add route handler for `POST /upload`
- [ ] Implement JSON request parsing for path list mode
- [ ] Implement multipart request parsing for file upload mode
- [ ] Add temporary file creation with timestamp-based naming
- [ ] Integrate with existing zpic upload pipeline
- [ ] Implement temporary file cleanup on success
- [ ] Implement temporary file cleanup on error
- [ ] Add request validation (file type, size limits)

### 1.5 Implement auxiliary endpoints
- [ ] Add `GET /health` endpoint with uptime tracking
- [ ] Add `GET /config` endpoint (non-sensitive config only)
- [ ] Add structured error responses for all endpoints
- [ ] Add request logging with tracing

### 1.6 Add server CLI subcommand
- [ ] Create `crates/zpic-cli/src/commands/server.rs`
- [ ] Implement `zpic server start` command
- [ ] Add `--port` flag for custom port (default: 36677)
- [ ] Add `--host` flag for custom host (default: 127.0.0.1)
- [ ] Add graceful shutdown on Ctrl+C
- [ ] Add startup banner with server URL

### 1.7 Configure CORS and middleware
- [ ] Add CORS layer with permissive localhost settings
- [ ] Add tracing middleware for request logging
- [ ] Add error handling middleware
- [ ] Add request timeout middleware (default: 60s)

### 1.8 Write server integration tests
- [ ] Test JSON upload request with local file paths
- [ ] Test multipart upload request with binary data
- [ ] Test health endpoint returns valid response
- [ ] Test config endpoint returns non-sensitive data
- [ ] Test error responses for invalid requests
- [ ] Test concurrent upload requests
- [ ] Test temporary file cleanup on success and error

## Phase 2: Obsidian Plugin Implementation

### 2.1 Initialize Obsidian plugin project
- [ ] Create `obsidian-zpic-plugin` repository
- [ ] Set up TypeScript project structure
- [ ] Add `manifest.json` with plugin metadata
- [ ] Configure Rollup for bundling
- [ ] Add `.gitignore` for node_modules and build artifacts

### 2.2 Implement settings system
- [ ] Create `settings.ts` with `ZpicSettings` interface
- [ ] Define default settings (server URL, upload flags, timeout)
- [ ] Implement settings load/save with Obsidian data API
- [ ] Create settings tab UI with text inputs and toggles
- [ ] Add settings validation and error messages

### 2.3 Implement HTTP uploader client
- [ ] Create `uploader.ts` with `ZpicUploader` class
- [ ] Implement JSON upload for file paths
- [ ] Implement multipart upload for File objects
- [ ] Add response parsing and error handling
- [ ] Add timeout handling with configurable duration
- [ ] Add retry logic for transient failures (optional)

### 2.4 Implement paste event handler
- [ ] Register `editor-paste` event listener
- [ ] Detect image files in clipboard data
- [ ] Check settings to enable/disable auto-upload
- [ ] Generate unique placeholder ID
- [ ] Insert temporary markdown: `![Uploading...{id}]()`
- [ ] Call uploader with clipboard files
- [ ] Replace placeholder with final markdown on success
- [ ] Show error notification and update placeholder on failure

### 2.5 Implement drag-and-drop event handler
- [ ] Register `editor-drop` event listener
- [ ] Detect image files in drag data
- [ ] Skip upload if Ctrl/Cmd key is pressed
- [ ] Check settings to enable/disable auto-upload
- [ ] Use same placeholder and replacement flow as paste
- [ ] Extract file paths for desktop Obsidian (Electron API)
- [ ] Handle files as blobs for mobile Obsidian

### 2.6 Implement utility functions
- [ ] Create `utils.ts` with helper functions
- [ ] Add `isImageFile()` for MIME type detection
- [ ] Add `generatePlaceholderId()` for unique IDs
- [ ] Add `replacePlaceholder()` for text replacement
- [ ] Add `formatImageMarkdown()` for markdown generation

### 2.7 Add user notifications and error handling
- [ ] Show Notice on successful upload
- [ ] Show Notice when server is unreachable
- [ ] Show Notice on upload timeout
- [ ] Show Notice on invalid file type
- [ ] Add actionable error messages with troubleshooting hints

### 2.8 Write plugin documentation
- [ ] Create `README.md` with installation instructions
- [ ] Document plugin settings and their effects
- [ ] Add troubleshooting section for common issues
- [ ] Add screenshots of settings panel and upload flow
- [ ] Document zpic server setup requirements

### 2.9 Test plugin on all platforms
- [ ] Test on Obsidian Desktop (macOS)
- [ ] Test on Obsidian Desktop (Windows)
- [ ] Test on Obsidian Desktop (Linux)
- [ ] Test on Obsidian Mobile (iOS)
- [ ] Test on Obsidian Mobile (Android)
- [ ] Verify paste and drag-and-drop on all platforms

## Phase 3: System Service Integration

### 3.1 Design service templates
- [ ] Create macOS launchd plist template
- [ ] Create Linux systemd service template
- [ ] Add variable substitution for config path, port, and executable path
- [ ] Document service file locations and permissions

### 3.2 Implement `zpic server install` for macOS
- [ ] Generate `~/Library/LaunchAgents/com.zpic.server.plist`
- [ ] Substitute template variables (executable path, port)
- [ ] Load service with `launchctl load`
- [ ] Verify service starts successfully
- [ ] Add error handling for permission issues

### 3.3 Implement `zpic server install` for Linux
- [ ] Generate `~/.config/systemd/user/zpic-server.service`
- [ ] Substitute template variables
- [ ] Reload systemd user daemon
- [ ] Enable and start service with `systemctl --user enable/start`
- [ ] Verify service starts successfully

### 3.4 Implement `zpic server uninstall`
- [ ] Detect platform (macOS vs Linux)
- [ ] Stop running service
- [ ] Remove service file
- [ ] Unload service (macOS) or disable (Linux)
- [ ] Confirm successful removal

### 3.5 Implement `zpic server status`
- [ ] Check if service is installed
- [ ] Query service status (running/stopped)
- [ ] Display PID, uptime, and port
- [ ] Show service auto-start configuration

### 3.6 Test service installation
- [ ] Test install on macOS with clean state
- [ ] Test install on Linux with clean state
- [ ] Verify auto-start after system reboot
- [ ] Test uninstall and verify cleanup
- [ ] Test status command in various states

## Phase 4: Testing and Documentation

### 4.1 Write comprehensive integration tests
- [ ] Test full upload flow: server → uploader → response
- [ ] Test server with multiple concurrent requests
- [ ] Test error scenarios (server down, invalid file, timeout)
- [ ] Test temp file cleanup under various failure modes
- [ ] Test CORS handling from different origins

### 4.2 Create end-to-end test scenarios
- [ ] Test Obsidian plugin + zpic server on desktop
- [ ] Test Obsidian plugin + zpic server on mobile
- [ ] Test with different uploaders (GitHub, S3, local)
- [ ] Test with large images (>10MB)
- [ ] Test with batch uploads (multiple images)

### 4.3 Write user documentation
- [ ] Create `docs/http-server.md` for zpic server
- [ ] Document server configuration options
- [ ] Document API endpoints with examples
- [ ] Create troubleshooting guide for common errors
- [ ] Document security best practices

### 4.4 Write plugin distribution docs
- [ ] Document manual installation from GitHub Releases
- [ ] Create release checklist for plugin versioning
- [ ] Document submission process for Obsidian Community Plugins
- [ ] Add CHANGELOG.md for plugin versions

### 4.5 Create architecture diagrams
- [ ] Draw request/response flow diagram
- [ ] Draw component architecture diagram
- [ ] Draw deployment topology (desktop vs mobile)
- [ ] Add diagrams to documentation

### 4.6 Prepare first release
- [ ] Tag zpic release with server support
- [ ] Build and test release binaries (macOS, Linux, Windows)
- [ ] Create GitHub Release for Obsidian plugin
- [ ] Package plugin files (main.js, manifest.json, styles.css)
- [ ] Write release notes highlighting new features

## Phase 5: Future Enhancements (Optional)

### 5.1 Advanced upload features
- [ ] Add WebSocket support for real-time progress
- [ ] Add batch upload optimization
- [ ] Add image compression before upload
- [ ] Add upload queue with retry mechanism

### 5.2 Security enhancements
- [ ] Add API key authentication for remote access
- [ ] Add TLS/HTTPS support
- [ ] Add rate limiting middleware
- [ ] Add request body size limits

### 5.3 Multi-editor support
- [ ] Create Zed extension using HTTP server
- [ ] Create VS Code extension using HTTP server
- [ ] Create Typora plugin using HTTP server
- [ ] Document generic HTTP API for custom integrations

### 5.4 Developer experience improvements
- [ ] Add OpenAPI/Swagger specification for HTTP API
- [ ] Create Postman collection for API testing
- [ ] Add development mode with CORS relaxation
- [ ] Add server metrics and monitoring endpoint
