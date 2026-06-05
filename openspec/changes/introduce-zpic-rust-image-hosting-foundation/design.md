## Context

The repository currently contains only OpenSpec scaffolding, while the product brief defines a broader Rust-native image-hosting toolkit that should eventually power a CLI, Zed workflows, and MCP tools. The first implementation needs to preserve PicGo migration ergonomics, avoid a Node runtime dependency, and produce machine-readable outputs so future integrations do not have to reimplement upload logic.

## Goals / Non-Goals

**Goals:**

- Define reusable crate boundaries so upload logic, config loading, history, formatting, and diagnostics live outside the CLI shell.
- Ship a CLI-first MVP that covers local files, clipboard upload, formatted output, PicGo compatibility, Markdown migration, and history/doctor workflows.
- Freeze a stable non-interactive command contract that future Zed slash commands and MCP tools can call directly.
- Prefer safe defaults for config import and document rewrite operations.

**Non-Goals:**

- Building a PicGo-style desktop GUI or other rich client UI in this change.
- Supporting arbitrary PicGo Node plugins at runtime.
- Implementing full Zed editor automation such as paste interception or drag-and-drop upload in the first release.
- Shipping remote delete or lifecycle-management features beyond explicit history and migration operations.

## Decisions

### 1. Use a Rust workspace with reusable crates

We will organize the project as a workspace with crates such as `zpic-core`, `zpic-config`, `zpic-cli`, and `zpic-history`, while leaving room for future `zpic-zed` and `zpic-mcp` adapters.

Rationale: shared crates keep upload behavior, result formatting, and config resolution in one place so new surfaces reuse the same rules.

Alternatives considered:
- Single binary crate: rejected because future editor and agent integrations would either shell out to ad hoc code or duplicate core logic.

### 2. Put upload providers behind a common async abstraction

`zpic-core` will expose a shared uploader contract that receives resolved file bytes, metadata, and a target object key, then returns URL/key metadata. The first-party uploader set for the foundation release is `local`, `github`, and `s3-compatible`.

Rationale: the product roadmap depends on one consistent upload pipeline while still supporting multiple backends with different protocols.

Alternatives considered:
- Provider-specific command implementations: rejected because tests, config compatibility, and future integrations become harder to scale.

### 3. Make native TOML canonical and PicGo JSON a compatibility source

`zpic` will use native TOML as the long-term source of truth, but config discovery will fall back to PicGo files when explicit `zpic` config is absent. `config import-picgo` will generate `zpic` TOML without mutating the original PicGo JSON.

Rationale: this preserves a low-friction migration path while allowing `zpic`-specific settings such as history, rename strategies, and future automation safeguards.

Alternatives considered:
- Using PicGo JSON as the primary format: rejected because it hard-couples `zpic` to PicGo naming and plugin assumptions.

### 4. Separate human-facing output from integration-facing output

Core commands will offer concise human-readable output by default and a `--json` mode with deterministic field names for `upload`, `migrate`, and `doctor`. Actionable errors go to `stderr`, and process exit codes signal full success or failure.

Rationale: CLI users want copy-paste-friendly output, while Zed and MCP callers need stable machine-readable payloads.

Alternatives considered:
- Human-readable output only: rejected because downstream callers would need fragile parsing.
- Separate adapter binaries only: rejected because it would duplicate behavior and slow future integration work.

### 5. Parse Markdown structurally before rewriting image links

Markdown migration will detect local image references using a parser/tokenizer approach, upload the referenced assets, and rewrite only the targeted spans. Dry-run and report modes will ship before writeback is considered complete.

Rationale: structural parsing avoids many false positives that regex-only replacement would introduce around titles, nested paths, and non-image links.

Alternatives considered:
- Regex-based rewriting: rejected because it is too brittle for repository documentation workflows.

### 6. Persist upload history in SQLite and expose `doctor`

Successful uploads will be recorded in a local SQLite database, and `zpic doctor` will run subsystem checks for config discovery, uploader credentials, clipboard availability, and history-store writability.

Rationale: SQLite is portable, queryable, and much easier to extend than a flat log file when later search or delete features arrive.

Alternatives considered:
- JSON log files: rejected because filtering, deduplication, and future tooling are harder.

### 7. Defer direct Zed and MCP implementations, but freeze the boundary now

This change defines the non-interactive CLI and JSON contracts that future Zed and MCP integrations will call, but it does not implement those adapters yet.

Rationale: the core workflow needs to be stable before editor and agent surfaces add platform-specific or security-sensitive behavior.

Alternatives considered:
- Shipping Zed and MCP adapters in the same change: rejected because it expands scope and raises avoidable platform and security risk for the initial foundation.

## Risks / Trade-offs

- [Broad initial scope] -> Limit first-party uploaders to local, GitHub, and S3-compatible backends while leaving clear extension points for others.
- [Clipboard behavior varies by platform] -> Treat macOS and Windows as primary targets first, and surface explicit diagnostics when clipboard backends are unavailable.
- [Markdown rewrite edge cases] -> Start with common inline image syntax and expand coverage through tests before widening syntax support.
- [Config compatibility gaps] -> Fail fast on unsupported PicGo plugin uploaders with actionable guidance rather than silently ignoring them.
- [Stable JSON contracts reduce implementation freedom] -> Record those contracts in spec files so breaking changes must be deliberate.

## Migration Plan

1. Bootstrap the Rust workspace and command surface without mutating user-owned files by default.
2. Implement read-only config discovery against PicGo files before enabling import to native `zpic` config.
3. Ship upload, history, and doctor flows on top of native `zpic` config and compatible PicGo discovery.
4. Add Markdown rewrite mode only after dry-run, report generation, and test coverage are in place.
5. Implement Zed and MCP adapters in follow-on changes against the stable CLI contract introduced here.

Rollback strategy:

- Because this is a new project, rollback means withholding release artifacts or disabling incomplete commands before release.
- `config import-picgo` remains safe because it only creates `zpic`-owned files and never edits PicGo-managed files.

## Open Questions

- Which non-GitHub PicGo uploaders must be treated as required for the first public release instead of a follow-on change?
- Should workspace-local `.zpic/config.toml` support ship in the first cut or immediately after a global config baseline is stable?
- Do we want the initial migration release to cover reference-style Markdown images, or should it focus only on inline image syntax first?
