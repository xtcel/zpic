# Changelog

All notable changes to the **zpic** workspace will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> The Obsidian community plugin that ships in `extensions/obsidian/` has its
> own version line and its own changelog at
> [`extensions/obsidian/.obsidian/plugins/zpic/CHANGELOG.md`](extensions/obsidian/.obsidian/plugins/zpic/CHANGELOG.md).
> The notes below cover the published Rust crates only.

## [Unreleased]

### Changed

- **Crate renamed: `zpic-image` → `zpic-media`.** The crate's responsibility
  has always been media-agnostic (MIME detection, content hashing, path
  templating); the new name reflects that. The directory under `crates/`,
  the `Cargo.toml` workspace member, and every internal `use` statement
  are updated. **Breaking** for downstream consumers depending on the
  Rust crate by name.
- **`zpic` server now accepts audio and video uploads.** New MIME
  detection entries cover `mp3`, `flac`, `wav`, `ogg`/`oga`, `m4a`,
  `3gp`, `mp4`, `webm`, and `ogv` (in addition to the existing image
  formats). The `MEDIA_EXTENSIONS` allow-list replaces the previous
  image-only constant; the JSON path under `/upload` enforces it.
- **HTML and JSX output formats now pick the right tag per media kind.**
  Images render as `<img>` / `<Image>` (unchanged), audio as
  `<audio controls>`, video as `<video controls>`. Markdown output
  remains format-agnostic — `![alt](url)` works for everything; users
  who want media-specific Markdown can pass a `--format` custom
  template.
- **Removed unused `mime_guess` dependency.** The crate was declared in
  the workspace `Cargo.toml` but never used — MIME detection is handled
  by `infer` plus a small extension table in `zpic-media`. Removing it
  drops a handful of transitive crates from the build.

### Obsidian plugin

- The plugin's `MEDIA_EXTENSIONS` allow-list (renamed from
  `IMAGE_EXTENSIONS`) now includes audio and video formats, mirroring
  the Rust server. The `guessMimeType` helper covers the same set of
  extensions.

## [0.1.2] - 2026-06-10

### Fixed

- **`zpic-uploaders` (s3 + oss): `canonicalize_headers` now lowercases header
  names per the AWS SigV4 spec.** Previously the helper emitted the keys
  verbatim, so a caller that handed in mixed-case keys (e.g. `Content-Type`
  from an upstream HTTP library) would produce a canonical request that did
  not match what the server computes, silently yielding a `SignatureDoesNotMatch`
  at the bucket. The OSS uploader's `canonicalize_headers_sorts_by_lower_key`
  test caught the bug; the S3 test used already-lowercase keys and therefore
  passed despite the same defect. Both call sites are now fixed.
- **`zpic-uploaders` (oss): `percent_encode_path` test no longer contradicts
  itself.** The first assertion expected `*` to be left unencoded, the next
  asserted the opposite. The implementation is correct (it encodes `*` per
  the OSS / S3 sub-delims rule), so the test was the thing that was wrong.

### Changed

- **`zpic-core`: added `UploaderKind::AliyunOss` variant.** The OSS uploader
  was already shipping in 0.1.0; the new variant is the source-of-truth enum
  entry that the loader, CLI flags, and PicGo compatibility layer now use to
  route configuration.
- **`zpic-plugins`: `Uploader::name` trait method now returns `&str` instead of
  `&'static str`.** Strictly speaking this is a **breaking change** for any
  external implementor of the `Uploader` trait, but the in-tree WASM plugin
  runtime was the only consumer and it was already returning borrowed strings.
  Bumping the workspace minor version was acceptable here because every
  `0.1.x` is still pre-`1.0` and may introduce breaking changes per semver.
- **Workspace version bumped from `0.1.0` to `0.1.2`.** Version `0.1.1` was
  intentionally skipped at the Rust-crate level: the 0.1.1 release line
  existed only for the Obsidian plugin (`extensions/obsidian/`), which
  shipped a client-side fix for the HTTP 400 reported on mobile and clipboard
  uploads. The server-side `multipart` parser was already correct; the
  defect was on the client and was fixed by passing the body as an
  `ArrayBuffer` instead of casting a `Uint8Array` to `string`. The corresponding
  Obsidian plugin notes are in its own changelog.
- **Added `categories` and `keywords` metadata to every published crate.**

  | Crate | Categories |
  |-------|-----------|
  | `zpic-core` | `data-structures` |
  | `zpic-config` | `config`, `parser-implementations` |
  | `zpic-image` | `multimedia`, `api-bindings` |
  | `zpic-history` | `database` |
  | `zpic-plugins` | `wasm`, `development-tools` |
  | `zpic-uploaders` | `api-bindings`, `network-programming` |
  | `zpic` (CLI) | `command-line-utilities`, `api-bindings` |

  Every crate shares the keywords `image-hosting`, `picgo`, and `zpic`, plus
  one or two crate-specific tags (`s3`, `github`, `wasm`, `sqlite`, `cli`, ...).

- **Test surface: `zpic-uploaders` 33/33, full workspace 110/110 passing.**

## [0.1.0] - 2026-06-09

### Added

- Initial public release of the **zpic** workspace. Seven crates, published
  to crates.io together.
- **`zpic-core`** — core data models, error types, the `Uploader` trait, and
  Markdown / URL formatters.
- **`zpic-config`** — TOML config loader, PicGo `picBed` compatibility layer
  (auto-detects both `~/.picgo/config.json` and the GUI `data.json` shape),
  and a guided `zpic config init` wizard.
- **`zpic-image`** — MIME detection, image dimension probing, BLAKE3-based
  content hashing, and `{{var}}` path-template rendering.
- **`zpic-history`** — SQLite-backed upload history (opt-in via
  `history_enabled = true`) with deduplication against the on-disk
  `rename.path` template.
- **`zpic-plugins`** — plugin manifest, discovery (vault + project-local
  `.zpic/plugins`), registry, and a `wasmtime`-based runtime for loading
  user-supplied uploaders as WASM modules.
- **`zpic-uploaders`** — built-in uploaders:
  - `local` — writes to a configurable target directory and returns a
    `public_base_url` URL.
  - `github` — uploads via the GitHub Contents API, with optional CDN
    rewriting (`customUrl` / `public_base_url`).
  - `s3` — direct REST / SigV4 implementation, no AWS SDK; works with AWS
    S3, Cloudflare R2, MinIO, and Backblaze B2.
  - `oss` — Aliyun OSS V4 signing (separate from S3 to match the OSS
    endpoint shape and header rules).
- **`zpic` (CLI binary)** — the `zpic` command. Subcommands: `upload`,
  `config`, `history`, `migrate` (Markdown image rewriting), `doctor`, and
  `server start` for the PicGo-compatible HTTP server.
- **PicGo-compatible HTTP server** — `POST /upload` (multipart and JSON
  path-list modes), `GET /health`, `GET /config`. CORS-open by design
  because it binds to loopback by default.
- **Obsidian community plugin** — auto-upload on paste and drag-and-drop,
  ribbon icon, command palette entries, per-note opt-out via the
  `zpic-upload: false` YAML key. Lives in `extensions/obsidian/` and
  releases independently of the Rust crates.

[Unreleased]: https://github.com/xtcel/zpic/compare/0.1.2...HEAD
[0.1.2]: https://github.com/xtcel/zpic/compare/d38d129...16b4781
[0.1.0]: https://github.com/xtcel/zpic/compare/fcea8d7...d38d129
