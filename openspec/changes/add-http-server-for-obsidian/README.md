# Add HTTP Server for Obsidian Integration

## Status: Proposed

This OpenSpec change introduces HTTP server mode to zpic and creates a dedicated Obsidian plugin for seamless image upload integration.

## Quick Links

- **[Proposal](./proposal.md)** - High-level overview and goals
- **[Design](./design.md)** - Detailed technical design and decisions
- **[Tasks](./tasks.md)** - Implementation task breakdown (4 phases, 60+ tasks)
- **[Roadmap](./ROADMAP.md)** - 5-week implementation timeline

### Specifications
- **[API Specification](./specs/api-specification.md)** - HTTP endpoint documentation

### Obsidian Plugin
- **[Quick Start Guide](./obsidian-plugin/QUICKSTART.md)** - User installation and usage
- **[Implementation Reference](./obsidian-plugin/IMPLEMENTATION.md)** - Developer code guide

## What's Changing

### zpic (Rust)
- ➕ Add HTTP server mode with PicGo-compatible API
- ➕ Add `zpic server start/stop/status/install/uninstall` subcommands
- ➕ Add system service support (macOS launchd, Linux systemd)
- ➕ Add `/upload`, `/health`, `/config` endpoints
- ✅ Preserve existing CLI functionality

### New: Obsidian Plugin (TypeScript)
- ✨ Auto-upload on paste (clipboard images)
- ✨ Auto-upload on drag-and-drop (local files)
- ✨ Cross-platform (Desktop: macOS/Windows/Linux, Mobile: iOS/Android)
- ✨ Settings panel for configuration
- ✨ Error handling with actionable messages

## Architecture

```
┌────────────────────────────────────────────────┐
│           Obsidian Plugin (Client)              │
│  • Paste/Drop event handlers                   │
│  • HTTP client (requestUrl)                    │
│  • Markdown insertion                          │
└──────────────────┬─────────────────────────────┘
                   │ HTTP POST
                   ▼
┌────────────────────────────────────────────────┐
│         zpic HTTP Server (Rust/Axum)           │
│  • POST /upload (JSON or multipart)            │
│  • GET /health                                 │
│  • GET /config                                 │
└──────────────────┬─────────────────────────────┘
                   │
                   ▼
┌────────────────────────────────────────────────┐
│        Existing zpic Upload Pipeline           │
│  • zpic-config  (read config)                  │
│  • zpic-uploaders (execute upload)             │
│  • zpic-history (record history)               │
└────────────────────────────────────────────────┘
```

## Timeline

| Week | Focus | Deliverable |
|------|-------|-------------|
| 1 | HTTP Server Foundation | `zpic server start` works |
| 2 | Obsidian Plugin MVP | Paste/drop upload works on desktop |
| 3 | Cross-Platform + Services | Mobile support + auto-start |
| 4 | Documentation + Release | First GitHub release |
| 5 | Community Testing | Submit to Obsidian marketplace |

## Key Features

### HTTP Server
- PicGo-compatible `/upload` endpoint
- Support both JSON (file paths) and multipart (binary) uploads
- Temporary file handling for clipboard/mobile uploads
- CORS-enabled for localhost clients
- Health check and config query endpoints
- System service integration for auto-start

### Obsidian Plugin
- Zero-configuration for basic use (just start zpic server)
- Automatic upload on paste and drag-and-drop
- Clear error messages with troubleshooting hints
- Configurable timeout, image description, and server URL
- Works on desktop (Electron) and mobile (iOS/Android)

## Configuration Example

### 1. Set up zpic (one-time)
```bash
# Configure uploader
zpic config init
zpic set uploader github MyBlog \
  --field repo=user/images \
  --field branch=main \
  --field token=$GITHUB_TOKEN

# Install auto-start service
zpic server install
```

### 2. Install Obsidian plugin
- Download from GitHub Releases
- Copy to `.obsidian/plugins/zpic/`
- Enable in Settings → Community plugins

### 3. Use in Obsidian
- Paste image → Auto-uploaded → Markdown inserted ✅
- Drag image → Auto-uploaded → Markdown inserted ✅

## Dependencies Added

### zpic-cli/Cargo.toml
```toml
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
multer = "3.0"
tempfile = "3.0"
```

### obsidian-zpic-plugin/package.json
```json
{
  "devDependencies": {
    "@rollup/plugin-typescript": "^11.0.0",
    "obsidian": "^1.4.0",
    "typescript": "^5.0.0"
  }
}
```

## Testing Strategy

- **Unit Tests**: Request parsing, temp file handling, response formatting
- **Integration Tests**: Full upload flow with mock uploader
- **Manual Tests**: All platforms (macOS, Windows, Linux, iOS, Android)
- **Performance Tests**: Concurrent uploads, large files, memory usage

## Success Criteria

- ✅ `zpic server start` launches HTTP server on port 36677
- ✅ Obsidian plugin uploads pasted images successfully
- ✅ Plugin works on both desktop and mobile Obsidian
- ✅ Auto-start works on macOS and Linux
- ✅ All existing CLI functionality still works
- ✅ Documentation is clear and complete

## Risks

| Risk | Mitigation |
|------|------------|
| Port 36677 conflict | Allow custom `--port` flag |
| Mobile CORS issues | Early mobile testing, adjust CORS |
| Temp file leaks | Use `tempfile` crate with drop guarantees |
| Service install fails | Provide manual installation docs |

## Future Enhancements

- WebSocket support for real-time progress
- API key authentication for remote deployment
- Support for other editors (Zed, VS Code)
- Batch upload optimization
- Image compression before upload

## Get Started

1. **Review the [Design](./design.md)** for technical details
2. **Check the [Tasks](./tasks.md)** for implementation checklist
3. **Follow the [Roadmap](./ROADMAP.md)** for timeline
4. **Use [API Specification](./specs/api-specification.md)** as reference

## Questions?

See [Design Decisions](./design.md#decisions) for rationale behind key choices.

---

**Created:** 2026-06-09  
**Status:** Proposed  
**Estimated Effort:** 4-5 weeks  
**Impact:** Enables Obsidian integration and future editor integrations
