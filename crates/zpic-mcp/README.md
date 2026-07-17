# zpic-mcp

An [MCP](https://modelcontextprotocol.io) server that exposes zpic's
upload/migrate/history/doctor operations as tools an AI agent (Claude
Code, Codex, or any other MCP-aware client) can call directly, instead of
shelling out to the `zpic` CLI itself.

It communicates over stdio and internally shells out to the `zpic`
binary with `--json` for each call — see [`docs/cli-contract.md`](../../docs/cli-contract.md)
for the exact JSON payloads each tool returns.

## Install & register

```bash
cargo install --path crates/zpic-mcp
```

Then register it with your MCP client. For Claude Code:

```bash
claude mcp add zpic -- zpic-mcp
```

Codex CLI and other MCP-aware tools have an equivalent "add an MCP server
by command" step; point it at the `zpic-mcp` binary the same way.

## Tools

| Tool | Description |
| :- | :- |
| `upload_image` | Upload a local image file through zpic's active uploader. |
| `upload_clipboard_image` | Upload the current clipboard image. **Disabled by default.** |
| `migrate_markdown_images` | Scan a Markdown file/dir and rewrite local image refs to remote URLs. Dry-run unless write mode is enabled. |
| `list_upload_history` | List past uploads from zpic's local history store. |
| `list_uploaders` | List configured uploader types/configs and which is active. Never returns credentials. |
| `doctor` | Run zpic's local diagnostic checks. |

Every tool result is the raw JSON `zpic` prints to stdout. Check the
top-level `success` field — a non-zero exit still returns a payload
describing which items failed (e.g. one file out of a multi-file upload).

## Security config

By default the server is safe to point at a project with zero setup: it
only reads files under its own working directory, caps file size at
20 MiB, and refuses anything but common image extensions. Clipboard
access and Markdown-rewriting are off until explicitly enabled.

Override the defaults with a TOML file, found via `ZPIC_MCP_CONFIG` or
`<user config dir>/zpic/mcp.toml`:

```toml
# workspace_roots defaults to the server's cwd if omitted
workspace_roots = ["/Users/me/projects"]
allow_clipboard = false
allow_migrate_write = false
max_file_size_mb = 20
allowed_extensions = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"]
# zpic_bin defaults to resolving `zpic` from PATH
zpic_bin = "zpic"
```

Every invocation of the underlying `zpic` binary (command + args + exit
status) is logged to stderr, since stdout is reserved for the MCP
JSON-RPC stream.

## Status

v1: process-per-call over stdio, path/extension/size validation, no
audit-log file (stderr only). Not yet implemented: per-tool
confirmation prompts and a persistent audit log file, both listed in
`PRD_v0.1.md` §16.3.
