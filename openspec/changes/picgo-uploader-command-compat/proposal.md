## Why

PicGo-Core's `uploader` subcommand is how users manage *multiple named
configurations per uploader type* (for example, two GitHub repos for
"personal" and "work"). zpic currently uses a simpler one-config-per-name
model that doesn't match PicGo's storage and can't express that use case.

To be a true drop-in PicGo replacement, zpic needs to:

1. Adopt PicGo's data model (`picBed.current` + `uploader.<type>.configList`
   with a `defaultId`), so existing PicGo configs work without manual
   conversion.
2. Ship PicGo-compatible `uploader`, `use`, and `set` subcommands so the
   workflows that depend on those commands keep working through zpic.

## What Changes

- Refactor the zpic config data model to mirror PicGo's `UploaderConfigManager`
  shape: `picBed.current` (active type), `uploader.<type>.configList` (named
  configs), `uploader.<type>.defaultId` (active config), plus per-type
  mirrors under `picBed.<type>`.
- Auto-migrate existing `default_uploader` + `[uploaders.<name>]` configs on
  load so users don't have to reconfigure.
- Add `zpic uploader {list,rename,copy,rm}` matching PicGo's exact flag
  shape and exit codes.
- Add `zpic use uploader <type> [configName]` and
  `zpic set uploader <type> [configName]` for activating and creating
  configs non-interactively (PicGo supports interactive prompts; zpic
  keeps the non-interactive form for the foundation).
- Add the `u` alias for `upload` to match PicGo.
- Document the migration in `docs/cli-contract.md` and `README.md`.

## Capabilities

### New Capabilities

- `picgo-uploader-manager`: First-class `uploader` subcommand with
  `list`/`rename`/`copy`/`rm` operations matching PicGo's CLI.
- `picgo-use-set-commands`: `use` and `set` subcommands for activating
  and creating uploader configs.

### Modified Capabilities

- `image-upload-cli`: The active uploader is now resolved from
  `picBed.current` + `uploader.<type>.configList[defaultId]` (or the
  legacy fallback) instead of `default_uploader` + `uploaders[name]`.
- `picgo-config-compatibility`: The native TOML is written in PicGo's
  shape. `config import-picgo` now produces the same shape directly.
- `upload-history-and-diagnostics`: `zpic doctor` inspects the new model
  and surfaces actionable fix messages.

## Impact

- Touches `zpic-config` (data model + manager), `zpic-cli` (new commands +
  updated subcommand plumbing), and the migration path.
- New file: `crates/zpic-config/src/manager.rs` and three new files under
  `crates/zpic-cli/src/commands/`.
- Backward compatible: existing single-config zpic TOML files migrate
  transparently on first read.
- No public Rust API breakage — `UploaderSection` is still the in-memory
  shape consumed by the existing uploaders.
