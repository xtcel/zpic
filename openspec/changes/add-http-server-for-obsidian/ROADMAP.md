# Implementation Roadmap

## Overview

This roadmap outlines the step-by-step implementation plan for adding HTTP server mode to zpic and creating the Obsidian integration plugin.

**Total Estimated Time:** 4-5 weeks
**Status:** Proposed
**Created:** 2026-06-09

---

## Week 1: HTTP Server Foundation

### Days 1-2: Project Setup and Dependencies

**Tasks:**
- [x] Create OpenSpec change structure
- [ ] Add HTTP server dependencies to `zpic-cli/Cargo.toml`
  - axum 0.8
  - tower and tower-http
  - multer for multipart parsing
  - tempfile for temp file handling
- [ ] Create server module structure
  - `src/server/mod.rs`
  - `src/server/routes.rs`
  - `src/server/handlers.rs`
  - `src/server/models.rs`
  - `src/server/middleware.rs`

**Deliverables:**
- Module structure in place
- Dependencies compile successfully

---

### Days 3-4: Core HTTP Endpoints

**Tasks:**
- [ ] Implement request/response data models
  - `UploadRequest` (JSON path list)
  - `UploadResponse` (PicGo-compatible)
  - `HealthResponse`
  - `ConfigResponse`
- [ ] Implement `/upload` endpoint handler
  - JSON request parsing
  - Multipart request parsing
  - Temp file creation and cleanup
  - Integration with existing upload pipeline
- [ ] Implement `/health` endpoint
- [ ] Implement `/config` endpoint
- [ ] Add CORS middleware
- [ ] Add logging middleware with tracing

**Deliverables:**
- All endpoints functional
- Manual testing with curl succeeds

---

### Day 5: CLI Integration and Testing

**Tasks:**
- [ ] Create `src/commands/server.rs` subcommand module
- [ ] Implement `zpic server start` command
  - Parse `--port` and `--host` flags
  - Start HTTP server
  - Graceful shutdown on Ctrl+C
  - Startup banner with URL
- [ ] Write integration tests
  - Test JSON upload request
  - Test multipart upload request
  - Test health endpoint
  - Test error responses
  - Test temp file cleanup

**Deliverables:**
- `zpic server start` works locally
- All tests passing

**Milestone:** ✅ HTTP Server MVP Complete

---

## Week 2: Obsidian Plugin Foundation

### Days 1-2: Plugin Project Setup

**Tasks:**
- [ ] Create `obsidian-zpic-plugin` repository
- [ ] Set up TypeScript project
  - package.json with dependencies
  - tsconfig.json
  - rollup.config.js
  - manifest.json
- [ ] Implement settings system
  - `settings.ts` with data model
  - Settings tab UI
  - Load/save settings
  - Validation

**Deliverables:**
- Plugin loads in Obsidian
- Settings panel works

---

### Days 3-4: Upload Implementation

**Tasks:**
- [ ] Implement HTTP client (`uploader.ts`)
  - JSON upload for file paths
  - Multipart upload for File objects
  - Response parsing
  - Error handling
  - Health check function
- [ ] Implement utility functions (`utils.ts`)
  - Image file detection
  - Placeholder ID generation
  - Markdown formatting
  - Text replacement helpers
- [ ] Write core upload logic in `main.ts`
  - Upload and insert function
  - Placeholder insertion
  - Success/error handling
  - User notifications

**Deliverables:**
- Manual upload via command palette works
- Error messages are clear

---

### Day 5: Event Handlers

**Tasks:**
- [ ] Implement paste event handler
  - Detect clipboard images
  - Check settings flag
  - Call upload function
  - Insert markdown
- [ ] Implement drag-and-drop handler
  - Detect dropped images
  - Skip if Ctrl/Cmd pressed
  - Call upload function
  - Insert markdown
- [ ] Test on desktop Obsidian (macOS)

**Deliverables:**
- Paste upload works
- Drag-and-drop upload works

**Milestone:** ✅ Plugin MVP Complete (Desktop)

---

## Week 3: Cross-Platform and System Services

### Days 1-2: Mobile Platform Testing

**Tasks:**
- [ ] Test plugin on Obsidian Mobile (iOS)
  - Install via manual sideload
  - Test paste behavior
  - Test file selection
  - Fix platform-specific issues
- [ ] Test plugin on Obsidian Mobile (Android)
  - Same testing as iOS
  - Fix Android-specific issues
- [ ] Adjust multipart upload if needed

**Deliverables:**
- Plugin works on iOS
- Plugin works on Android

---

### Days 3-4: System Service Integration

**Tasks:**
- [ ] Design launchd plist template (macOS)
- [ ] Design systemd service template (Linux)
- [ ] Implement `zpic server install` command
  - Detect platform
  - Generate service file with variable substitution
  - Load/enable service
  - Verify service starts
- [ ] Implement `zpic server uninstall` command
  - Stop service
  - Remove service file
  - Unload/disable service
- [ ] Implement `zpic server status` command
  - Check if installed
  - Query running state
  - Display PID, uptime, port

**Deliverables:**
- Auto-start works on macOS
- Auto-start works on Linux

---

### Day 5: Integration Testing

**Tasks:**
- [ ] End-to-end testing scenarios
  - Desktop + zpic server + GitHub uploader
  - Mobile + zpic server + S3 uploader
  - Large images (>10MB)
  - Batch uploads (multiple images)
- [ ] Test error scenarios
  - Server not running
  - Invalid credentials
  - Network timeout
  - File too large
- [ ] Performance testing
  - Concurrent uploads
  - Memory usage
  - Temp file cleanup

**Deliverables:**
- All platforms tested
- Performance acceptable

**Milestone:** ✅ Full Platform Support Complete

---

## Week 4: Documentation and Release Prep

### Days 1-2: User Documentation

**Tasks:**
- [ ] Write `docs/http-server.md`
  - Server configuration
  - API endpoints
  - Security best practices
- [ ] Write plugin README
  - Installation instructions
  - Configuration guide
  - Usage examples
  - Troubleshooting
- [ ] Write API specification
  - Endpoint reference
  - Request/response formats
  - Error codes
- [ ] Create quick start guide
  - Step-by-step setup
  - Common workflows

**Deliverables:**
- Complete documentation set

---

### Days 3-4: Testing and Polish

**Tasks:**
- [ ] Code review and refactoring
- [ ] Add unit tests for edge cases
- [ ] Fix any remaining bugs
- [ ] Performance optimization
- [ ] Update CHANGELOG
- [ ] Create architecture diagrams

**Deliverables:**
- Code quality high
- All tests passing

---

### Day 5: Release Preparation

**Tasks:**
- [ ] Build release binaries
  - macOS (Intel + ARM)
  - Linux (x86_64)
  - Windows (x86_64)
- [ ] Package Obsidian plugin
  - Build production bundle
  - Create zip file
  - Verify contents
- [ ] Create GitHub Releases
  - zpic with server support
  - Obsidian plugin v0.1.0
- [ ] Write release notes
- [ ] Tag versions

**Deliverables:**
- zpic release with HTTP server
- Obsidian plugin release for manual installation

**Milestone:** ✅ First Release Complete

---

## Week 5: Community Testing and Iteration

### Days 1-3: Beta Testing

**Tasks:**
- [ ] Announce beta release
- [ ] Collect user feedback
- [ ] Monitor GitHub issues
- [ ] Fix critical bugs
- [ ] Update documentation based on feedback

---

### Days 4-5: Obsidian Plugin Submission

**Tasks:**
- [ ] Prepare community plugin submission
  - Fork obsidian-releases repo
  - Add plugin to community-plugins.json
  - Provide required metadata
  - Submit PR
- [ ] Address review feedback
- [ ] Final polish for official release

**Deliverables:**
- Plugin submitted to Obsidian Community Plugins
- Ready for official approval

**Milestone:** ✅ Production Ready

---

## Future Enhancements (Post-v1.0)

### Phase 2: Advanced Features
- [ ] WebSocket support for real-time progress
- [ ] API key authentication for remote deployment
- [ ] Batch upload optimization
- [ ] Image compression before upload
- [ ] Upload queue with retry mechanism

### Phase 3: Multi-Editor Support
- [ ] Zed extension
- [ ] VS Code extension
- [ ] Typora plugin
- [ ] Generic HTTP API documentation

### Phase 4: Enhanced UX
- [ ] Web-based configuration UI
- [ ] Upload history viewer in plugin
- [ ] Drag-to-reorder uploaded images
- [ ] Image thumbnail preview

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Port conflict | Medium | High | Allow custom port via CLI flag |
| Mobile CORS issues | Medium | High | Early testing, adjust CORS config |
| Temp file leaks | Low | Medium | Use tempfile crate with drop guarantees |
| Service install fails | Low | Medium | Provide manual installation docs |
| Obsidian API changes | Low | High | Pin Obsidian API version, monitor updates |

---

## Success Metrics

### Week 1
- ✅ zpic server starts without errors
- ✅ curl upload requests work
- ✅ All integration tests pass

### Week 2
- ✅ Plugin uploads images on paste
- ✅ Plugin uploads images on drag-and-drop
- ✅ Error messages are clear

### Week 3
- ✅ Plugin works on iOS and Android
- ✅ Auto-start works on macOS and Linux
- ✅ End-to-end tests pass

### Week 4
- ✅ Documentation complete
- ✅ Release packages ready
- ✅ GitHub releases published

### Week 5
- ✅ Beta feedback addressed
- ✅ Plugin submitted to community
- ✅ No critical bugs

---

## Dependencies

### Development Dependencies
- Rust 1.70+ (for zpic server)
- Node.js 18+ (for Obsidian plugin)
- Obsidian 0.15.0+ (for plugin testing)
- macOS, Linux, or Windows (for platform testing)

### Runtime Dependencies
- zpic CLI installed
- zpic server running
- Network connectivity (localhost or LAN)

---

## Communication Plan

### Weekly Updates
- Progress summary
- Blockers and risks
- Next week's goals

### Milestones
- Week 1: HTTP Server MVP
- Week 2: Plugin MVP
- Week 3: Cross-Platform Support
- Week 4: First Release
- Week 5: Community Ready

---

## Rollback Plan

If critical issues are found:

1. **Revert HTTP server changes** if they break existing CLI functionality
2. **Disable plugin** if it causes Obsidian crashes
3. **Postpone release** until issues are resolved
4. **Communicate clearly** with users about known issues

---

## Next Steps

1. **Review this roadmap** and adjust timeline if needed
2. **Set up development environment** with all dependencies
3. **Create project tracking board** (GitHub Projects)
4. **Start Week 1 tasks** beginning with dependencies setup

---

**Last Updated:** 2026-06-09
**Status:** Ready to begin implementation
