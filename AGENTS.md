# AGENTS.md

Instructions for AI coding agents (Codex, Claude Code, and similar tools)
working in this repository or in any project that has `zpic` installed.

## What zpic is

`zpic` is a Rust CLI that uploads images (and other files) to configured
image hosts — local disk, GitHub, S3-compatible storage, Aliyun OSS — and
renders the result as Markdown, a bare URL, HTML, or JSX. It is PicGo-config
compatible. Full docs: [`README.md`](README.md).

## Preferred integration paths, in order

1. **MCP server (best for agents that support MCP).** Build and run
   `crates/zpic-mcp` (binary `zpic-mcp`). It exposes `upload_image`,
   `upload_clipboard_image`, `migrate_markdown_images`,
   `list_upload_history`, `list_uploaders`, and `doctor` as MCP tools over
   stdio. See [`crates/zpic-mcp/README.md`](crates/zpic-mcp/README.md) for
   the tool schemas, the security config (`workspace_roots`,
   `allow_clipboard`, `allow_migrate_write`, `max_file_size_mb`,
   `allowed_extensions`), and how to register it with an MCP-aware client.
2. **Direct CLI + `--json` (works everywhere, no MCP required).** Every
   state-changing command accepts `--json` and prints a single JSON object
   or array to `stdout`; diagnostics go to `stderr`. This is the frozen
   contract documented in [`docs/cli-contract.md`](docs/cli-contract.md).
   Prefer this when the agent can only shell out to a subprocess.

## Quick CLI reference (see docs/cli-contract.md for full payloads)

```bash
zpic upload <file...> --json          # upload file(s), print {success, items[]}
zpic upload --clipboard --json        # upload the clipboard image
zpic migrate <file-or-dir> --dry-run --json   # rewrite local image refs to remote URLs
zpic uploader list --json             # active uploader + per-type named configs (no secrets)
zpic history list --json              # past uploads
zpic doctor --json                    # config/credential/clipboard/history health checks
```

Exit code `0` means every requested operation succeeded; `1` means at
least one failed. The JSON payload is emitted on both exit codes when
`--json` is passed — check the payload's `success` field for granular
per-item results (e.g. one file failing out of a multi-file upload).

## Safety notes for agents

- Never invent or guess credential values. If `zpic doctor --json` reports
  a missing credential, surface it to the user instead of writing one into
  a config file.
- `zpic set uploader` / `zpic use uploader` mutate the user's config file
  on disk. Only run these when the user explicitly asked to add or switch
  an uploader.
- `zpic migrate` rewrites Markdown files in place unless `--dry-run` is
  passed. Default to `--dry-run` first and show the user the diff/report
  before rerunning without it.
- The MCP server enforces `workspace_roots` and file-size/extension limits
  by default (see its README). Do not try to work around those limits from
  the CLI on the agent's behalf.
