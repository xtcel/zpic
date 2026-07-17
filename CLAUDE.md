# CLAUDE.md

@AGENTS.md

The imported file above documents how to *call* `zpic` (CLI JSON contract,
MCP server, safety rules). The notes below are specific to *developing*
this repository.

## Build & test

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Workspace map

- `crates/zpic-core` — data models, error types, uploader trait, formatters
- `crates/zpic-config` — TOML config + PicGo compatibility layer
- `crates/zpic-media` — MIME/dimension/hash + path template rendering
- `crates/zpic-history` — SQLite-backed upload history
- `crates/zpic-plugins` — plugin manifests, discovery, WASM runtime
- `crates/zpic-uploaders` — local, GitHub, S3-compatible, Aliyun OSS uploaders
- `crates/zpic-cli` — the `zpic` binary (commands, HTTP server, pipeline)
- `crates/zpic-mcp` — the `zpic-mcp` binary (MCP server for AI agents)

The CLI's non-interactive `--json` surface is a frozen contract — see
[`docs/cli-contract.md`](docs/cli-contract.md) before changing any
command's JSON shape, and update that doc in the same change.

This project uses [OpenSpec](openspec/) to track specs and in-flight
changes; check `openspec/specs/` and `openspec/changes/` for the current
contract before proposing a design that conflicts with it.
