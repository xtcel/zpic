# markdown-image-migration Specification

## Purpose
TBD - created by archiving change introduce-zpic-rust-image-hosting-foundation. Update Purpose after archive.
## Requirements
### Requirement: Migration scans Markdown for local image references
The `zpic migrate` command SHALL scan a Markdown file or directory tree for local image references and identify which references are eligible for upload.

#### Scenario: Scan a single Markdown file
- **WHEN** the user runs `zpic migrate README.md`
- **THEN** the command finds local image references inside `README.md`
- **THEN** the command excludes non-image links from the upload candidate list

#### Scenario: Scan a directory recursively
- **WHEN** the user runs `zpic migrate ./docs --recursive`
- **THEN** the command scans Markdown files within that directory tree
- **THEN** the command aggregates upload candidates across the matching files

### Requirement: Migration supports dry-run summaries
The migration command SHALL support a dry-run mode that reports discovered changes without rewriting any Markdown files.

#### Scenario: Preview migration changes
- **WHEN** the user runs `zpic migrate README.md --dry-run`
- **THEN** the command reports how many local image references were found
- **THEN** the command does not modify `README.md`

### Requirement: Migration can upload and rewrite local image references
The migration command SHALL upload discovered local image assets and rewrite the corresponding Markdown references when rewrite mode is requested.

#### Scenario: Rewrite local image references
- **WHEN** the user runs migration in write mode for a Markdown file containing local image paths
- **THEN** the command uploads those local image files
- **THEN** the command replaces each migrated local path with the uploaded remote URL

### Requirement: Migration preserves remote image references
The migration command SHALL leave already remote image references unchanged while still allowing them to appear in reports when requested.

#### Scenario: Encounter a remote image URL
- **WHEN** a Markdown file contains an image reference that already points to `http` or `https`
- **THEN** the migration command does not upload that image
- **THEN** the original Markdown reference remains unchanged

### Requirement: Migration can emit a machine-readable report
The migration command SHALL support writing a structured report of scanned files, uploaded assets, and rewritten references.

#### Scenario: Write a migration report
- **WHEN** the user runs `zpic migrate ./docs --report migration-report.json`
- **THEN** the command writes a structured report file at the requested path
- **THEN** the report includes counts plus the before-and-after value for each rewritten reference

