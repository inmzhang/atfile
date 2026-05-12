# Pinned LSP Binary Download Design

## Goal

Make the Atfile Zed extension install and run its native `atfile-lsp` server automatically, without requiring users to build the binary or add it to `PATH`.

## Release Model

The extension downloads a binary from the GitHub release that matches the extension version. Extension version `0.1.0` maps to GitHub tag `v0.1.0`.

This keeps installs reproducible: a given extension version always runs the matching LSP binary version. Publishing a new extension version requires a matching GitHub release tag and uploaded binary assets.

## Runtime Launcher

`language_server_command` keeps a local override path first:

1. If `worktree.which("atfile-lsp")` returns a binary, launch it with `--stdio`.
2. Otherwise, resolve the current OS and architecture with `zed::current_platform()`.
3. Find or download the matching release asset.
4. Make the binary executable on Unix platforms.
5. Launch the downloaded binary with `--stdio`.

The production launcher will not use `CARGO_MANIFEST_DIR` or `env::current_dir()` as a binary fallback. Those paths are useful during local experiments but do not represent Zed's installed extension work directory.

## Asset Names

Release assets use predictable names:

- `atfile-lsp-v0.1.0-linux-x86_64.tar.gz`
- `atfile-lsp-v0.1.0-linux-aarch64.tar.gz`
- `atfile-lsp-v0.1.0-macos-x86_64.tar.gz`
- `atfile-lsp-v0.1.0-macos-aarch64.tar.gz`
- `atfile-lsp-v0.1.0-windows-x86_64.zip`

Downloaded assets are extracted under:

- `atfile-lsp-v0.1.0/<platform>/atfile-lsp`
- `atfile-lsp-v0.1.0/<platform>/atfile-lsp.exe` on Windows

Unsupported platforms return a clear error that includes the OS and architecture.

## CI

CI is split into two workflows:

- `ci.yml`: run formatting, tests, clippy with warnings denied, and the `wasm32-wasip2` extension build check.
- `release.yml`: on tags matching `v*`, build `atfile-lsp` on Ubuntu, macOS, and Windows, package the binary using the asset names above, and upload the archives to the GitHub release.

The release workflow builds:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

## Cleanup

The extension glue should stay small and focused. It should contain platform selection, release lookup, download, and command construction. The LSP implementation modules stay as they are unless a test or clippy warning requires a narrow cleanup.

Redundant tests around temporary development launch paths should be removed. Tests should cover:

- platform-to-asset mapping,
- binary path construction,
- user-installed binary precedence,
- clear unsupported-platform errors.

## Documentation

The README should describe two paths:

- Normal use: the extension downloads the pinned release binary automatically.
- Local development: either put `atfile-lsp` on `PATH`, or create a matching release asset/tag for the extension version.
