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
   cargo package -p zpic-image
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
   cargo publish -p zpic-image
   cargo publish -p zpic-history
   cargo publish -p zpic-uploaders
   cargo publish -p zpic
   ```

6. Wait for crates.io index propagation between publishes if Cargo says a
   dependency version is not visible yet.

## GitHub Releases

For non-Rust users, distribute prebuilt archives through GitHub
Releases. A practical first matrix is:

- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

Recommended artifact naming:

```text
zpic-v0.1.0-x86_64-apple-darwin.tar.gz
zpic-v0.1.0-aarch64-apple-darwin.tar.gz
zpic-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
zpic-v0.1.0-aarch64-unknown-linux-gnu.tar.gz
zpic-v0.1.0-x86_64-pc-windows-msvc.zip
```

Each archive should contain the `zpic` binary plus top-level license
files and, optionally, the README.

## Homebrew Distribution

Homebrew's recommended approach for third-party formulae is a separate
tap repository. For this project, create:

- GitHub repo: `xtcel/homebrew-tap`
- Formula path: `Formula/zpic.rb`
- Install command: `brew install xtcel/tap/zpic`

The formula should download the matching archive from a GitHub Release,
verify its SHA-256, and install the `zpic` binary into `bin`.

## Suggested Release Flow

1. Merge release-ready changes to `main`.
2. Bump the version in the workspace root.
3. Publish the Cargo packages to crates.io.
4. Tag the repo, for example `v0.1.0`.
5. Build and upload platform archives to the GitHub Release for that tag.
6. Update `xtcel/homebrew-tap/Formula/zpic.rb` with the new URLs and SHA-256 values.
7. Verify fresh installs with both `cargo install ...` and `brew install ...`.
