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
zpic u ./cover.png
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

`zpic u` is a direct alias for `zpic upload` and accepts the same flags.

## `zpic uploader`

Manage named uploader configs stored under `uploader.<type>.configList`.

```bash
zpic uploader list
zpic uploader list github
zpic uploader rename github Work Personal
zpic uploader copy github Personal Staging
zpic uploader rm github Staging
```

### `uploader list` JSON payload

```json
{
  "current_uploader": "github",
  "types": [
    {
      "type": "github",
      "is_current": true,
      "default_config": "Personal",
      "configs": [
        {
          "id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
          "name": "Personal",
          "is_default": true,
          "created_at": 1700000000000,
          "updated_at": 1700000000000
        }
      ]
    }
  ]
}
```

Config names are matched case-insensitively. `rename`, `copy`, and `rm`
return exit code `0` on success and `1` on failure.

## `zpic use uploader`

Activate an uploader type and, optionally, a named config inside that
type.

```bash
zpic use uploader github
zpic use uploader github Work
zpic use uploader local Default --json
```

### JSON payload

```json
{
  "action": "use",
  "type": "github",
  "active_config": "Work",
  "saved_to": "/Users/me/.config/zpic/config.toml"
}
```

## `zpic set uploader`

Create or update a named uploader config non-interactively. `--from`
copies fields from an existing config in the same uploader type before
applying any `--field key=value` overrides.

```bash
zpic set uploader
zpic set uploader github Work --field repo=me/picbed --field token=$GITHUB_TOKEN
zpic set uploader github Staging --from Work --field branch=develop
```

When `type`, `configName`, or field values are omitted in text mode,
`zpic set uploader` enters guided setup: it lists the built-in uploader
types, lets the user choose one, then prompts only for the fields that
type requires.

### JSON payload

```json
{
  "action": "set",
  "type": "github",
  "active_config": "Work",
  "inherited_from": null,
  "saved_to": "/Users/me/.config/zpic/config.toml"
}
```

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

## `zpic zed init`

Scaffold project-local Zed tasks and helper scripts into a `.zed/`
directory inside the current project (or an explicit `--project-root`).

```bash
zpic zed init
zpic zed init --project-root ~/notes --zpic-bin /opt/homebrew/bin/zpic
zpic zed init --json
```

Generated files:

- `.zed/tasks.json`
- `.zed/zpic-keymap.json.example`
- `.zed/zpic-README.md`
- platform-specific clipboard upload helper script
- platform-specific current-file migrate helper script

### JSON payload

```json
{
  "action": "init",
  "project_root": "/Users/me/notes",
  "shell": "posix",
  "created": [
    "/Users/me/notes/.zed/tasks.json",
    "/Users/me/notes/.zed/zpic-keymap.json.example",
    "/Users/me/notes/.zed/zpic-README.md",
    "/Users/me/notes/.zed/zpic-upload-from-clipboard.sh",
    "/Users/me/notes/.zed/zpic-migrate-current-file.sh"
  ]
}
```
```

## `zpic config import-picgo`

Convert a PicGo config (default: `~/.picgo/config.json`) into a native
zpic TOML file (default: user config dir). The source PicGo file is
never modified. The generated TOML uses PicGo's uploader-manager shape:
`pic_bed.current`, `pic_bed.<type>`, and `uploader.<type>.configList`.

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

## Migration Notes

Legacy zpic configs that still use `default_uploader` plus
`[uploaders.<name>]` are auto-migrated in memory on load. Saving through
`zpic uploader ...`, `zpic use uploader ...`, or `zpic set uploader ...`
rewrites the file in the PicGo-compatible shape.

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
