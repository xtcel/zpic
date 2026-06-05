# zpic CLI & JSON Contract

This document freezes the non-interactive command surface that future
Zed and MCP adapters will call. Anything documented here is part of the
public contract; breaking changes must be deliberate and announced.

## Exit codes

- `0`: every requested operation completed successfully.
- `1`: one or more requested operations failed (partial uploads, missing
  credentials, config errors, ...).

Errors are always reported on `stderr`. The JSON payload, when
requested, is always printed to `stdout`.

## `zpic upload`

Upload one or more local image files (or the contents of the clipboard)
to the active uploader. Supports the following output formats:
`markdown` (default), `url`, `html`, `jsx`, and `json`.

```bash
zpic upload ./cover.png
zpic upload ./a.png ./b.jpg
zpic upload --clipboard
zpic upload ./cover.png --uploader github --format markdown --copy
zpic upload ./cover.png --dry-run
zpic upload ./cover.png --json
```

### JSON payload

```json
{
  "success": true,
  "items": [
    {
      "source": "/abs/path/to/cover.png",
      "url": "https://cdn.example.com/cover.png",
      "key": "images/2026/06/04/cover.png",
      "markdown": "![cover](https://cdn.example.com/cover.png)",
      "mime": "image/png",
      "size": 238912,
      "width": 1200,
      "height": 800,
      "uploader": "github",
      "error": null
    }
  ]
}
```

When a single file fails, its `error` field is set and the other fields
may be `null`. The `success` flag at the top level is `true` only when
every item succeeded.

## `zpic migrate`

Scan Markdown files for local image references, upload them through the
active uploader, and rewrite the documents with the new remote URLs.

```bash
zpic migrate README.md
zpic migrate ./docs --recursive
zpic migrate README.md --dry-run
zpic migrate ./docs --report migration-report.json
```

### JSON payload

```json
{
  "scanned_files": 1,
  "found": 1,
  "uploaded": 1,
  "rewritten_files": 1,
  "changes": [
    {
      "file": "/abs/path/README.md",
      "from": "./assets/logo.png",
      "to": "https://cdn.example.com/logo.png",
      "markdown": "![logo](https://cdn.example.com/logo.png)"
    }
  ],
  "items": [
    {
      "source": "/abs/path/assets/logo.png",
      "url": "https://cdn.example.com/logo.png",
      "key": "images/2026/06/04/logo.png",
      "markdown": "![logo](https://cdn.example.com/logo.png)",
      "mime": "image/png",
      "size": 12345,
      "width": 800,
      "height": 600,
      "uploader": "github",
      "error": null
    }
  ]
}
```

`migrate` exits with code `1` if any referenced file failed to upload,
even when dry-run / report mode is used.

## `zpic doctor`

Run local diagnostic checks for config discovery, PicGo fallback, the
active uploader's credentials, clipboard availability, and the
history-store writability.

```bash
zpic doctor
zpic doctor --json
```

### JSON payload

```json
{
  "checks": [
    {
      "name": "config (user)",
      "status": "pass",
      "message": "path: /Users/me/.config/zpic/config.toml"
    },
    {
      "name": "uploader (r2) credentials",
      "status": "fail",
      "message": "missing credential: s3 uploader requires `access_key_id` and `secret_access_key`",
      "fix": "set `access_key_id` and `secret_access_key` env vars or config keys"
    }
  ]
}
```

`status` is one of `pass`, `warn`, or `fail`. Each check may include a
`fix` field with a short remediation hint.

## `zpic history list`

List previously recorded uploads. Supports filtering by uploader name.

```bash
zpic history list
zpic history list --uploader github --limit 20
zpic history list --json
```

### JSON payload

```json
[
  {
    "id": "1b9d6bcd-1bf2-4a5e-9c0c-3a2b1c2d3e4f",
    "created_at": "2026-06-04T10:23:45Z",
    "source_path": "/abs/path/cover.png",
    "uploader": "github",
    "key": "images/2026/06/04/cover.png",
    "url": "https://cdn.example.com/cover.png",
    "markdown": "![cover](https://cdn.example.com/cover.png)",
    "mime": "image/png",
    "size": 238912,
    "width": 1200,
    "height": 800,
    "status": "ok"
  }
]
```

## `zpic config import-picgo`

Convert a PicGo config (default: `~/.picgo/config.json`) into a native
zpic TOML file (default: user config dir). The source PicGo file is
never modified.

```bash
zpic config import-picgo
zpic config import-picgo --from /path/to/picgo.json --to /path/to/zpic.toml
```

Returns exit code `0` on success. Errors include:

- `ConfigNotFound` — no PicGo file at the resolved path.
- `UploaderUnsupported` — the active PicGo uploader is provided by a
  Node plugin with no native `zpic` implementation.
- `ConfigInvalid` — the destination already exists or the source is
  unreadable.

## Zed adapter hooks

A Zed extension can call zpic as a child process and parse the JSON
payload:

```rust
let output = Command::new("zpic")
    .args(&["upload", path, "--uploader", "r2", "--format", "markdown", "--json"])
    .output()?;
let payload: UploadPayload = serde_json::from_slice(&output.stdout)?;
```

Slash commands can be implemented in the Zed extension to expose
`/zpic-upload <path>` and `/zpic-url <path>` on top of this contract.

## MCP adapter hooks

A future `zpic-mcp` server will expose the same operations as MCP tools:

- `upload_image(path, uploader?, format?)` → `UploadPayload`
- `migrate_markdown_images(path, dry_run?)` → `MigrateReport`
- `list_upload_history(uploader?, limit?)` → `[HistoryEntry]`
- `doctor()` → `DoctorReport`

MCP defaults must continue to be safe (no destructive operations
without explicit flags, workspace-root restrictions, etc.). The JSON
contract above is the wire format.
