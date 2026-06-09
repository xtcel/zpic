## Context

`zpic` is designed as a CLI-first tool with rich configuration management and uploader support. To integrate with Obsidian (a cross-platform note-taking app), we need to expose zpic's capabilities through an HTTP API that works on both desktop and mobile devices. This change introduces HTTP server mode while preserving the existing CLI-first architecture and configuration model.

## Goals / Non-Goals

**Goals:**

- Expose zpic upload functionality through PicGo-compatible HTTP endpoints.
- Enable Obsidian users to automatically upload images on paste and drag-and-drop with minimal configuration.
- Support both desktop (spawn/HTTP) and mobile (HTTP-only) Obsidian installations.
- Allow zpic server to run as a system service with auto-start capabilities on macOS and Linux.
- Reuse existing zpic-config, zpic-uploaders, and zpic-history without duplication.
- Provide clear error messages and health check endpoints for troubleshooting.

**Non-Goals:**

- Reimplementing PicGo's plugin system or Node.js runtime compatibility.
- Supporting PicGo's image gallery, settings UI, or desktop application features.
- Building a web-based configuration UI for zpic (configuration stays file-based).
- Changing the core upload pipeline or breaking existing CLI contracts.
- Implementing real-time WebSocket notifications (HTTP polling/long-polling is sufficient).

## Decisions

### 1. Use Axum as the HTTP framework

**Decision:** Adopt `axum` 0.8+ with `tower` and `tower-http` for the HTTP server implementation.

**Rationale:**
- Axum is a mature, performant Rust HTTP framework built on tokio and hyper.
- Provides excellent ergonomics for request/response handling and middleware.
- Tower ecosystem offers battle-tested middleware for CORS, logging, and error handling.
- Minimal overhead compared to alternatives like actix-web while maintaining type safety.

**Alternatives considered:**
- `actix-web`: More features but heavier runtime; overkill for our use case.
- `warp`: Considered, but axum has better ecosystem momentum and clearer composition model.
- `rocket`: Requires nightly Rust; incompatible with our stable Rust requirement.

### 2. Implement PicGo-compatible /upload endpoint with dual input modes

**Decision:** The `/upload` endpoint accepts two request formats:

1. **JSON path list** (for local files already on disk):
   ```json
   POST /upload
   Content-Type: application/json
   
   { "list": ["/path/to/image1.png", "/path/to/image2.jpg"] }
   ```

2. **Multipart form data** (for clipboard/drag-and-drop):
   ```http
   POST /upload
   Content-Type: multipart/form-data; boundary=----...
   
   ------...
   Content-Disposition: form-data; name="list"; filename="timestamp.png"
   Content-Type: image/png
   
   <binary data>
   ```

**Response format** (PicGo-compatible):
```json
{
  "success": true,
  "result": ["https://cdn.example.com/image1.png"],
  "fullResult": [{
    "imgUrl": "https://cdn.example.com/image1.png",
    "delete": "https://cdn.example.com/delete/token123"
  }]
}
```

**Rationale:**
- Desktop Obsidian can access local file paths and send JSON requests (more efficient).
- Mobile Obsidian cannot access arbitrary file paths, requiring multipart upload.
- This dual-mode design matches PicGo and PicList behavior, ensuring plugin compatibility.
- The `fullResult` field enables future delete support if uploaders expose delete tokens.

**Alternatives considered:**
- Multipart-only: Rejected because it's inefficient for desktop use cases where files are already on disk.
- Custom binary protocol: Rejected for complexity and ecosystem incompatibility.

### 3. Handle temporary files for clipboard/drag-and-drop uploads

**Decision:** When receiving multipart uploads (clipboard/drag-and-drop):
1. Save uploaded files to system temp directory with timestamp-based names.
2. Pass temp file paths to the existing zpic upload pipeline.
3. Delete temp files after successful upload or on error.
4. Use `tempfile` crate for secure temp file handling.

**Rationale:**
- Existing zpic upload pipeline expects file paths, not in-memory buffers.
- System temp directory provides cross-platform compatibility and automatic cleanup on reboot.
- Timestamp-based naming avoids collisions while providing traceable file names for debugging.

**Error handling:**
- If upload fails, temp files are deleted to prevent disk space leaks.
- If server crashes mid-upload, OS temp cleanup handles orphaned files.

### 4. Add health check and configuration query endpoints

**Decision:** Implement additional endpoints:

- `GET /health` - Server health check
  ```json
  { "status": "ok", "version": "0.1.0", "uptime": 3600 }
  ```

- `GET /config` - Current configuration overview
  ```json
  {
    "currentUploader": "github",
    "uploaders": ["github", "local", "s3"],
    "version": "0.1.0"
  }
  ```

**Rationale:**
- Obsidian plugin needs to verify server availability before attempting uploads.
- Configuration endpoint helps with troubleshooting and uploader selection in the future.
- Standard health check enables integration with service monitoring tools.

**Security consideration:**
- `/config` endpoint does not expose secrets (tokens, passwords); only metadata and type names.

### 5. Server lifecycle management via CLI subcommands

**Decision:** Add `zpic server` subcommand with these operations:

```bash
# Start server in foreground (default)
zpic server start [--port <PORT>] [--host <HOST>]

# Start server in background (daemon mode, optional future work)
zpic server start --daemon

# Stop running server (optional future work)
zpic server stop

# Show server status
zpic server status
```

**Default configuration:**
- Port: `36677` (matches PicGo default)
- Host: `127.0.0.1` (localhost only for security)
- Config: Reads from standard zpic config locations

**Rationale:**
- Explicit subcommand keeps CLI surface clean and extensible.
- Default port matches PicGo ecosystem expectations for compatibility.
- Localhost-only binding prevents accidental exposure on untrusted networks.

**Alternatives considered:**
- Always-running daemon: Rejected for Phase 1; users prefer explicit control.
- Auto-start on CLI commands: Rejected; mixing modes creates confusion.

### 6. System service integration for macOS and Linux

**Decision:** Provide installation scripts and service templates:

**macOS (launchd)**:
- Template: `~/Library/LaunchAgents/com.zpic.server.plist`
- Install command: `zpic server install` (generates and loads plist)
- Uninstall command: `zpic server uninstall`

**Linux (systemd)**:
- Template: `~/.config/systemd/user/zpic-server.service`
- Install command: `zpic server install`
- Uninstall command: `zpic server uninstall`

**Rationale:**
- User-level services (not system-level) for security and permission simplicity.
- Template-based approach allows customization of port and config paths.
- Auto-start on login improves UX for Obsidian users who want "always available" uploads.

**Alternatives considered:**
- System-level services: Rejected for requiring sudo/admin privileges.
- Manual service file creation: Rejected for poor UX; auto-generation is table stakes.

### 7. Obsidian Plugin Architecture

**Component structure:**

```
obsidian-zpic-plugin/
├── main.ts          # Plugin entry, event registration
├── uploader.ts      # HTTP client for zpic server
├── settings.ts      # Settings panel and data model
├── utils.ts         # Image detection, text replacement helpers
└── types.ts         # TypeScript type definitions
```

**Key behaviors:**

1. **Paste handling:**
   - Register `editor-paste` event listener.
   - Check if clipboard contains image files.
   - Insert temporary placeholder: `![Uploading...abc123]()`.
   - Upload via HTTP POST to zpic server.
   - Replace placeholder with final markdown link.

2. **Drag-and-drop handling:**
   - Register `editor-drop` event listener.
   - Skip if Ctrl/Cmd key is pressed (preserve local file behavior).
   - Extract files from `DataTransfer`.
   - Same upload flow as paste.

3. **Error handling:**
   - Connection refused → Notify user to start zpic server.
   - Upload failed → Replace placeholder with error message, allow retry.
   - Timeout → Configurable timeout (default 30s), show progress indicator.

**Settings:**
```typescript
interface ZpicSettings {
  serverUrl: string;           // Default: http://127.0.0.1:36677
  uploadOnPaste: boolean;      // Default: true
  uploadOnDrop: boolean;       // Default: true
  imageDesc: "origin" | "none"; // Default: "origin"
  deleteLocalAfterUpload: boolean; // Default: false
  timeout: number;             // Default: 30000ms
}
```

**Decision rationale:**
- Simple architecture focuses on the core upload flow without over-engineering.
- Settings are minimal but cover essential configuration needs.
- Error handling is explicit and actionable for end users.

### 8. Request/Response flow for clipboard uploads

**Detailed flow:**

```
┌─────────────────────────────────────────────────────────────┐
│ Obsidian Plugin (Client)                                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│ 1. User pastes image                                         │
│    ↓                                                         │
│ 2. Extract File from clipboard                              │
│    ↓                                                         │
│ 3. Generate temp placeholder: ![Uploading...abc123]()       │
│    ↓                                                         │
│ 4. Create FormData with file                                │
│    ↓                                                         │
│ 5. POST to http://127.0.0.1:36677/upload                    │
│    Content-Type: multipart/form-data                         │
│    Body: file as 'list' field                               │
│    ↓                                                         │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP POST
                       ↓
┌─────────────────────────────────────────────────────────────┐
│ zpic HTTP Server                                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│ 6. Receive multipart request                                │
│    ↓                                                         │
│ 7. Parse files from multipart body                          │
│    ↓                                                         │
│ 8. Save each file to temp dir: /tmp/zpic-{timestamp}.png    │
│    ↓                                                         │
│ 9. Call existing upload pipeline with temp paths            │
│    ↓                                                         │
│ 10. Get upload result URLs from uploader                    │
│    ↓                                                         │
│ 11. Delete temp files                                       │
│    ↓                                                         │
│ 12. Return JSON response:                                   │
│     { "success": true, "result": ["https://..."] }          │
│    ↓                                                         │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP Response
                       ↓
┌─────────────────────────────────────────────────────────────┐
│ Obsidian Plugin (Client)                                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│ 13. Parse response JSON                                     │
│    ↓                                                         │
│ 14. Replace placeholder with final markdown:                │
│     ![image.png](https://cdn.example.com/image.png)         │
│    ↓                                                         │
│ 15. Show success notice to user                             │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### 9. CORS and security configuration

**Decision:** Enable CORS with permissive settings for localhost development:

```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([Method::GET, Method::POST])
    .allow_headers(Any);
```

**Rationale:**
- Obsidian's internal HTTP client originates from `app://obsidian.md` or `file://` origin.
- Localhost binding (`127.0.0.1`) already limits exposure to local machine.
- CORS needs to be permissive for both desktop and mobile Obsidian.

**Security considerations:**
- Server binds to `127.0.0.1` by default, not `0.0.0.0`.
- No authentication required for localhost-only deployment.
- Future enhancement: Add API key authentication for remote deployment scenarios.

### 10. Logging and observability

**Decision:** Use `tracing` for structured logging:

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(body))]
async fn upload_handler(body: Multipart) -> Result<Json<UploadResponse>> {
    info!("Received upload request");
    // ...
    info!(file_count = files.len(), "Processing upload");
    // ...
}
```

**Log levels:**
- `INFO`: Server start/stop, request handling, upload success
- `WARN`: Configuration issues, upload warnings
- `ERROR`: Upload failures, connection errors

**Rationale:**
- Consistent with existing zpic CLI logging.
- Structured logs enable easier troubleshooting.
- Instrument macro provides automatic span tracking for requests.

### 11. Error response format

**Decision:** Standardized error response:

```json
{
  "success": false,
  "msg": "Human-readable error message",
  "code": "ERROR_CODE"  // Optional, for programmatic handling
}
```

**Common error codes:**
- `INVALID_FILE_TYPE`: Unsupported image format
- `UPLOAD_FAILED`: Uploader returned error
- `CONFIG_ERROR`: Configuration issue
- `SERVER_ERROR`: Internal server error

**Rationale:**
- Matches PicGo error response format for compatibility.
- `msg` field provides actionable feedback for users.
- Optional `code` field enables future client-side error handling logic.

## Implementation Phases

### Phase 1: HTTP Server Foundation (Week 1-2)

**Deliverables:**
- Add axum, tower dependencies to zpic-cli.
- Implement `/upload` endpoint with both JSON and multipart support.
- Implement `/health` endpoint.
- Add `zpic server start` subcommand.
- Write integration tests for server endpoints.

**Acceptance criteria:**
- `zpic server start` launches HTTP server on port 36677.
- `curl -X POST http://127.0.0.1:36677/upload -d '{"list":["test.png"]}'` returns valid response.
- Multipart upload works with real image files.

### Phase 2: Obsidian Plugin Implementation (Week 2-3)

**Deliverables:**
- Create obsidian-zpic-plugin repository.
- Implement paste and drag-and-drop event handlers.
- Build settings panel with server URL configuration.
- Add error handling and user notifications.
- Write plugin README with installation instructions.

**Acceptance criteria:**
- Pasting image in Obsidian triggers upload to zpic server.
- Dragging image file into editor triggers upload.
- Error messages display when server is not running.
- Plugin settings can configure server URL and behavior.

### Phase 3: System Service Integration (Week 3-4)

**Deliverables:**
- Implement `zpic server install` for macOS (launchd).
- Implement `zpic server install` for Linux (systemd).
- Add `zpic server uninstall` and `zpic server status`.
- Document service installation process.

**Acceptance criteria:**
- `zpic server install` creates and loads service file.
- zpic server auto-starts on system login.
- `zpic server status` reports running/stopped state.
- `zpic server uninstall` cleanly removes service.

### Phase 4: Testing and Documentation (Week 4-5)

**Deliverables:**
- Comprehensive integration tests for server + plugin.
- User documentation for zpic server and Obsidian plugin.
- API documentation for HTTP endpoints.
- Troubleshooting guide for common issues.
- GitHub Release preparation for plugin distribution.

**Acceptance criteria:**
- All tests pass on macOS, Linux, Windows (server).
- Plugin tested on desktop and mobile Obsidian.
- Documentation covers installation, configuration, and troubleshooting.

## Testing Strategy

### Unit Tests
- Request parsing (JSON and multipart).
- Response formatting.
- Temp file handling and cleanup.

### Integration Tests
- Full upload flow with mock uploader.
- Error handling scenarios.
- Concurrent request handling.

### Manual Testing
- Desktop Obsidian (macOS, Windows, Linux).
- Mobile Obsidian (iOS, Android).
- System service installation and auto-start.
- Various image formats and sizes.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Port 36677 already in use | High | Allow custom port via `--port` flag |
| Mobile Obsidian CORS issues | High | Test early on iOS/Android, adjust CORS config |
| Temp file cleanup failure | Medium | Use `tempfile` crate with drop guarantees |
| System service installation failure | Medium | Provide manual installation fallback docs |
| Server crash during upload | Medium | Use structured error handling, log crashes |

## Future Enhancements (Out of Scope)

- WebSocket support for real-time upload progress notifications.
- API key authentication for remote server deployment.
- Web-based configuration UI.
- Support for other editors (Zed, VS Code, Typora).
- Batch upload optimization for large image sets.
