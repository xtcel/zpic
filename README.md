# zpic

A Rust-native image hosting CLI compatible with PicGo configuration. zpic
ships a single binary, `zpic`, that uploads images to local filesystems,
GitHub repositories, and S3-compatible object storage (Cloudflare R2, AWS
S3, MinIO, Backblaze B2, ...), with optional Markdown migration, upload
history, and `doctor` diagnostics.

## Workspace layout

```text
crates/
├── zpic-core/        # data models, error types, uploader trait, formatters
├── zpic-config/      # zpic TOML config + PicGo compatibility layer
├── zpic-image/       # MIME/dimension/hash + path template rendering
├── zpic-history/     # SQLite-backed upload history
├── zpic-plugins/     # plugin manifests, discovery, registry, and WASM runtime
├── zpic-uploaders/   # local, GitHub, and S3-compatible uploaders
└── zpic-cli/         # the `zpic` binary
```

## Installation

```bash
# From a local checkout
cargo install --path crates/zpic-cli

# From the GitHub repository
cargo install --git https://github.com/xtcel/zpic zpic --bin zpic

# From crates.io, after the package is published
cargo install zpic --bin zpic

# From Homebrew, after the tap is published
brew install xtcel/tap/zpic
```

See [`docs/distribution.md`](docs/distribution.md) for the release and
distribution checklist.

## Quick start

```bash
# Build
cargo build --release

# Create a starter config
./target/release/zpic config init

# Show the resolved config (secrets are redacted)
./target/release/zpic config show

# Run the diagnostic check
./target/release/zpic doctor

# Upload a local file
./target/release/zpic upload ./cover.png

# PicGo-compatible alias for `upload`
./target/release/zpic u ./cover.png

# Scaffold project-local Zed tasks and helper scripts
./target/release/zpic zed init

# Upload with a custom output format and copy the result to the clipboard
./target/release/zpic upload ./cover.png --format markdown --copy

# Inspect and switch named uploader configs
./target/release/zpic uploader list
./target/release/zpic use uploader github Work

# Create or update a named uploader config non-interactively
./target/release/zpic set uploader github Work \
  --field repo=me/picbed \
  --field branch=main \
  --field token=$GITHUB_TOKEN

# Or run guided setup: pick a type, then fill the fields it needs
./target/release/zpic set uploader

# Migrate a markdown file in dry-run mode
./target/release/zpic migrate README.md --dry-run

# Rewrite local image references in a markdown file
./target/release/zpic migrate README.md
```

## Config

The canonical configuration is a TOML file. By default zpic looks in:

1. `--config <path>` (command line)
2. `ZPIC_CONFIG` (environment)
3. `<cwd>/.zpic/config.toml` (project)
4. `~/.config/zpic/config.toml` (user; platform-aware via `directories`)
5. `~/.picgo/config.json` (PicGo core fallback)
6. PicGo GUI data file (per-OS fallback)

`zpic config import-picgo` converts a PicGo config into a native zpic
TOML file at the user-global path. The original PicGo file is never
modified.

The native TOML mirrors PicGo's uploader manager model:

- `pic_bed.current` / `pic_bed.uploader` select the active uploader type
- `uploader.<type>.configList` stores named configs per uploader type
- `uploader.<type>.defaultId` points at the active config for that type
- `pic_bed.<type>` mirrors the active config fields for that type

See [`examples/local/config.toml`](examples/local/config.toml) for a
minimal PicGo-compatible native config.

## Plugins

`zpic` supports uploader plugins through a `zpic`-native WASM plugin
system. Plugins are discovered from:

1. `ZPIC_PLUGIN_DIRS` (path-separated list, highest priority)
2. `<cwd>/.zpic/plugins`
3. the user-global plugin directory resolved by `directories`

Each plugin lives in its own directory and includes:

- `plugin.toml` — plugin metadata and uploader field schema
- `plugin.wasm` — the WASM module executed by `zpic`

Plugin uploader configs use the same native config shape as built-in
uploaders: `uploader.<type>.configList`.

## Compatibility

zpic understands PicGo's `picBed` plus `uploader.<type>.configList`
layout and supports the following built-in uploaders out of the box:

- `local` — copy to a local directory
- `github` — upload to a GitHub repo via the contents API
- `s3` — upload to any S3-compatible endpoint (R2, MinIO, B2, S3)

PicGo compatibility is limited to configuration compatibility. `zpic`
does **not** run PicGo Node plugins or emulate PicGo's plugin commands.
If a PicGo config references a plugin uploader, `zpic` can use it only
when a corresponding `zpic` uploader plugin is installed locally.

Existing legacy zpic configs that still use `default_uploader` and
`[uploaders.<name>]` are auto-migrated in memory on load. The next save
through `zpic set uploader`, `zpic use uploader`, or `zpic uploader ...`
rewrites them in the PicGo-compatible shape.

## Integration contract

The CLI and JSON contracts are designed to be safe to call from editor
and agent integrations (Zed slash commands, MCP tools). Concretely:

- `zpic upload <files> --json` — single object with `success` and `items`
- `zpic uploader list [type] --json` — current uploader plus per-type config summaries
- `zpic use uploader <type> [configName] --json` — active uploader selection result
- `zpic set uploader <type> <configName> --json` — create/update result
- `zpic migrate <path> --json` — object with `found`, `uploaded`, and
  `changes`
- `zpic doctor --json` — object with one entry per subsystem check
- `zpic history list --json` — array of history entries
- `zpic zed init --json` — created `.zed` task and helper file paths

Exit codes: `0` on success, non-zero on any failure. Diagnostic messages
go to `stderr`; the JSON payload stays on `stdout`.

## Zed integration

`zpic` ships a task-first Zed workflow for day-to-day editing, plus a thin
dev extension for Assistant slash commands:

1. Install `zpic` locally.
2. In any writing project, run `zpic zed init`.
3. Open that project in Zed and use the generated `.zed/tasks.json` entries
   such as `zpic: upload clipboard as markdown`.
4. Optionally merge `.zed/zpic-keymap.json.example` into your global Zed
   `keymap.json` for shortcuts.
5. If you also want Assistant slash commands, install the dev extension from
   [`extensions/zed`](extensions/zed).

See [`docs/zed-integration.md`](docs/zed-integration.md) for details.

## Status

This is the v0.1 foundation release with PicGo-compatible uploader
multi-config management plus the first cut of `zpic`-native WASM
uploader plugins. See `openspec/specs/` for the current tracked
contract.
