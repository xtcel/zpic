## 1. Data Model

- [x] 1.1 Add `UploaderConfigItem`, `UploaderTypeConfigs`, and `PicBedSection`
      types to `zpic-config`, matching PicGo's `UploaderConfigItem` shape
      (`_id`, `_configName`, `_createdAt`, `_updatedAt`, flattened fields).
- [x] 1.2 Change `ZpicConfigFile.uploaders` from a name-keyed map of
      `UploaderSection` to a type-keyed map of `UploaderTypeConfigs`.
- [x] 1.3 Add `pic_bed: PicBedSection` (with `current`, `uploader`,
      `transformer`, `proxy`, and per-type active mirrors) to `ZpicConfigFile`.
- [x] 1.4 Add a `migrate_legacy(cfg: &mut ZpicConfigFile)` helper that
      promotes the old `default_uploader` + `[uploaders.<name>]` fields to
      the new model.

## 2. UploaderConfigManager

- [x] 2.1 Add `UploaderConfigManager` with `list_types`, `list_configs`,
      `get_active`, `get_by_name`, `use_config`, `create_or_update`,
      `rename`, `copy`, `remove`.
- [x] 2.2 Implement migration inside the manager: when the new model is
      empty but the legacy fields are present, promote them automatically
      on first access.
- [x] 2.3 Keep `picBed.<type>` mirrors in sync when activating or
      updating a config.
- [x] 2.4 Emit a single deprecation warning when the legacy fields are
      auto-migrated, so users can clean up their config.

## 3. CLI: uploader subcommand

- [x] 3.1 Add `zpic uploader list [type]` (text and `--json`).
- [x] 3.2 Add `zpic uploader rename <type> <oldName> <newName>`.
- [x] 3.3 Add `zpic uploader copy <type> <configName> <newConfigName>`.
- [x] 3.4 Add `zpic uploader rm <type> <configName>`.

## 4. CLI: use and set subcommands

- [x] 4.1 Add `zpic use uploader <type> [configName]`.
- [x] 4.2 Add `zpic set uploader <type> <configName>` (non-interactive;
      takes a `--field key=value` flag for each config field, and a
      `--from <name>` flag to copy an existing config's fields as a
      starting point).
- [x] 4.3 Make `upload` accessible via the `u` alias.

## 5. Wiring

- [x] 5.1 Update `resolve_uploader` in `zpic-cli/src/util.rs` to use the
      new `UploaderConfigManager` (with legacy fallback).
- [x] 5.2 Update `zpic config show` to display the new model.
- [x] 5.3 Update `zpic config import-picgo` to write the new model
      directly.
- [x] 5.4 Update `zpic doctor` to inspect the new model.
- [x] 5.5 Add unit tests for the manager and the migration helper.
- [x] 5.6 Add end-to-end CLI tests for `uploader list/rename/copy/rm`,
      `use`, and `set`.

## 6. Documentation

- [x] 6.1 Update `README.md` with the new commands.
- [x] 6.2 Update `docs/cli-contract.md` to document the new JSON payloads
      and exit codes.
- [x] 6.3 Add a "Migrating from PicGo" section explaining how the data
      model change affects existing users.
