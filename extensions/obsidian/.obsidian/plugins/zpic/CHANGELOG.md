# Changelog

All notable changes to the **zpic Image Upload** Obsidian plugin are
documented here. The format follows [Keep a Changelog][keep-a-changelog]
and the project adheres to [Semantic Versioning][semver].

[keep-a-changelog]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html

## [0.1.1] — 2026-06-09

### Fixed

- **Drag-and-drop uploads were rejected with `unsupported content type`**
  on Obsidian desktop and mobile. Obsidian's `requestUrl` does not
  reliably set the `Content-Type: multipart/form-data; boundary=…`
  header when handed a `FormData` body, so the server had no way to
  tell which parser to dispatch to.

  The plugin now serialises the multipart envelope by hand
  (`buildMultipartBody` in `src/uploader.ts`) and sets the
  `Content-Type` header explicitly. As a defence-in-depth measure,
  the Rust server has also been taught to **sniff the body** when the
  `Content-Type` header is missing and dispatch to multipart or JSON
  based on the leading bytes — see the server `CHANGELOG.md` for the
  matching change.

## [0.1.0] — 2026-06-09

### Added

- Initial release.
- Auto-upload on **paste** from the clipboard.
- Auto-upload on **drag-and-drop** of image files.
- Manual upload via the ribbon icon and the command palette
  (`Upload image from clipboard`, `Upload current image attachment`).
- Settings tab with **Server URL**, **Request timeout**,
  **Upload on paste**, **Upload on drop**, **Image description**,
  and **Delete local after upload** controls.
- **Check server** button in the settings tab — verifies reachability
  and reports the active uploader.
- Per-note opt-out via `zpic-upload: false` frontmatter.
- Server URL validation in the settings tab.
- Multipart upload path used by mobile Obsidian and clipboard blobs.
- JSON path-list upload path used by desktop Obsidian with native
  files.
- Health-check + config-probe helpers used by the settings tab and
  before every upload.
- PicGo-compatible request / response model matching the spec in
  `openspec/changes/add-http-server-for-obsidian/specs/api-specification.md`.
- `styles.css` with the status block styling for the settings tab.
