# Distribution Guide

This repository is structured as a Cargo workspace. The end-user command
is the `zpic` binary, which lives in the `zpic` Cargo package.

## Install Commands

Use these commands in user-facing docs:

```bash
# Local checkout
cargo install --path crates/zpic-cli

# Install directly from GitHub
cargo install --git https://github.com/xtcel/zpic zpic --bin zpic

# Install from crates.io after publishing
cargo install zpic --bin zpic

# Install from Homebrew after publishing the tap
brew install xtcel/tap/zpic
```

## crates.io Release Checklist

Cargo does not allow published packages to depend on local `path`
dependencies alone. Each internal workspace dependency must also carry a
version, which this repository now does through `workspace.dependencies`.

Before a release:

1. Update `workspace.package.version` in the root `Cargo.toml`.
2. Run `cargo test`.
3. Run package checks:

   ```bash
   cargo package -p zpic-core
   cargo package -p zpic-config
   cargo package -p zpic-media
   cargo package -p zpic-history
   cargo package -p zpic-uploaders
   cargo package -p zpic
   ```

4. Authenticate once on the release machine:

   ```bash
   cargo login
   ```

5. Publish in dependency order:

   ```bash
   cargo publish -p zpic-core
   cargo publish -p zpic-config
   cargo publish -p zpic-media
   cargo publish -p zpic-history
   cargo publish -p zpic-uploaders
   cargo publish -p zpic
   ```

6. Wait for crates.io index propagation between publishes if Cargo says a
   dependency version is not visible yet.

## GitHub Releases

For non-Rust users, distribute prebuilt archives through GitHub
Releases. This repository now includes
[`release.yml`](../.github/workflows/release.yml), which triggers on a
tag like `v0.1.1` and currently builds:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

Recommended artifact naming:

```text
zpic-v0.1.0-x86_64-apple-darwin.tar.gz
zpic-v0.1.0-aarch64-apple-darwin.tar.gz
zpic-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
zpic-v0.1.0-x86_64-pc-windows-msvc.zip
zpic-v0.1.0-source.tar.gz
```

Each archive should contain the `zpic` binary plus top-level license
files and, optionally, the README. The workflow also uploads:

- `checksums.txt` — SHA-256 manifest for all release archives
- `zpic.rb` — rendered Homebrew formula ready to copy into the tap repo

## Homebrew Distribution

Homebrew's recommended approach for third-party formulae is a separate
tap repository. For this project, create:

- GitHub repo: `xtcel/homebrew-tap`
- Formula path: `Formula/zpic.rb`
- Install command: `brew install xtcel/tap/zpic`

The checked-in [`Formula/zpic.rb`](../Formula/zpic.rb) file is the
template source-of-truth. Each tagged release renders a concrete
`zpic.rb` formula that points at the versioned `zpic-v<version>-source.tar.gz`
release asset and includes the matching SHA-256.

That generated formula should be copied into `xtcel/homebrew-tap` and
committed there. The formula builds from source with `cargo install`,
which is a better fit for Homebrew than shipping a binary-only formula.

## Suggested Release Flow

1. Merge release-ready changes to `main`.
2. Bump the version in the workspace root.
3. Publish the Cargo packages to crates.io.
4. Push a tag like `v0.1.1` to trigger `.github/workflows/release.yml`.
5. Copy the generated `zpic.rb` release asset into `xtcel/homebrew-tap/Formula/zpic.rb`.
6. Verify fresh installs with both `cargo install ...` and `brew install ...`.
