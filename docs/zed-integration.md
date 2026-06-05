# Zed Integration

`zpic` integrates with Zed in two layers:

1. Project-local tasks for the real editing workflow
2. A thin dev extension for Assistant slash commands

## 1. Project tasks

Inside any content project, run:

```bash
zpic zed init
```

This creates a `.zed/` directory with:

- `tasks.json`
- `zpic-keymap.json.example`
- `zpic-README.md`
- platform-specific helper scripts for clipboard upload and current-file migrate

The generated tasks are:

- `zpic: upload clipboard as markdown`
- `zpic: upload clipboard as url`
- `zpic: migrate current markdown file`
- `zpic: doctor`
- `zpic: uploader list`

The upload tasks pass the current Zed selection through to `zpic` as:

- `--alt` using normalized selection text
- `--name` using a sanitized, slug-like variant of that selection text

That gives a VS PicGo-style flow without reimplementing upload logic inside the
editor.

### Optional shortcuts

Open Zed's global keymap file with `zed: open keymap file` and merge in
`.zed/zpic-keymap.json.example`.

The generated example binds:

- `secondary-alt-u` to clipboard upload as Markdown
- `secondary-alt-shift-u` to clipboard upload as URL
- `secondary-alt-m` to migrate the current file
- `secondary-alt-d` to run `zpic doctor`

`secondary` maps to `cmd` on macOS and `ctrl` on Windows/Linux.

## 2. Assistant dev extension

This repository also includes a thin Zed extension in [`extensions/zed`](../extensions/zed)
that shells out to the local `zpic` binary.

Current slash commands:

- `/zpic-upload`
- `/zpic-doctor`
- `/zpic-history`
- `/zpic-uploader-list`

Install it locally:

1. Make sure `zpic` is on your shell `PATH`
2. In Zed, run `zed: install dev extension`
3. Select `extensions/zed`
4. Open the Assistant and try `/zpic-doctor`

## Scope

This integration intentionally keeps the editor-facing layer thin:

- `zpic` remains the only upload implementation
- Zed tasks handle the main edit-time workflow
- The extension only forwards Assistant commands to the local CLI

That keeps behavior aligned across editors and avoids splitting config or
uploader logic into a second runtime.
