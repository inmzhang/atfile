# Pinned LSP Binary Download Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Atfile automatically download and run a pinned `atfile-lsp` binary release asset when no user-installed binary is available.

**Architecture:** Move binary installation concerns into a small `src/installation.rs` module, keeping `src/lib.rs` responsible for Zed extension wiring. GitHub Actions will validate Rust code and build release archives named exactly as the extension expects.

**Tech Stack:** Rust 2024, `zed_extension_api 0.7.0`, GitHub Actions, shell/PowerShell archive packaging.

---

## File Map

- Create `src/installation.rs`: platform mapping, release asset naming, download path construction, release lookup, and installation.
- Modify `src/lib.rs`: use `installation::installed_server_binary` and remove dev-path fallback code.
- Modify `README.md`: document automatic download and release/dev workflow.
- Create `.github/workflows/ci.yml`: format, test, clippy, WASI check.
- Create `.github/workflows/release.yml`: build and upload release archives for supported platforms.

## Tasks

### Task 1: Add Installer Module Tests

**Files:**
- Create: `src/installation.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create a test-first skeleton**

Create `src/installation.rs` with data-only helpers and tests for supported platform mapping, using simple internal enums so tests do not need to call Zed host APIs.

- [ ] **Step 2: Wire the module**

Add `mod installation;` to `src/lib.rs`.

- [ ] **Step 3: Run tests and confirm they fail or compile only against stubs**

Run: `rtk cargo test --lib installation`

Expected: fail if helper functions are missing, or pass if the skeleton already includes the helper implementations.

### Task 2: Implement Pinned Download Launcher

**Files:**
- Modify: `src/installation.rs`
- Modify: `src/lib.rs`
- Modify: `extension.toml`

- [ ] **Step 1: Implement install helpers**

In `src/installation.rs`, implement:

- `SERVER_BINARY`
- `EXTENSION_VERSION`
- `RELEASE_TAG`
- `Platform`
- `ArchiveKind`
- `Platform::current()`
- `Platform::asset_name()`
- `Platform::archive_path()`
- `Platform::binary_path()`
- `installed_server_binary()`

Use `zed::github_release_by_tag_name`, `zed::download_file`, and `zed::make_file_executable`.

- [ ] **Step 2: Replace `src/lib.rs` fallback**

Remove `CARGO_MANIFEST_DIR` path lookup. Keep PATH lookup first, then call `installation::installed_server_binary()`.

- [ ] **Step 3: Update capabilities**

Allow the release-backed binary path shape with process execution capability if needed by Zed's capability model.

- [ ] **Step 4: Run Rust checks**

Run:

```bash
rtk cargo fmt --all -- --check
rtk cargo test --all-targets
rtk cargo check -p atfile-extension --target wasm32-wasip2
rtk cargo clippy --all-targets -- -D warnings
```

Expected: all pass.

### Task 3: Add GitHub CI

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Add CI workflow**

Create a workflow on pushes and PRs that installs `wasm32-wasip2`, then runs the same local verification commands.

- [ ] **Step 2: Add release workflow**

Create a tag-triggered workflow that builds `atfile-lsp` and uploads assets:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS aarch64
- Windows x86_64

- [ ] **Step 3: Validate workflow syntax locally by inspection**

Run: `rtk rg -n "atfile-lsp-v|wasm32-wasip2|upload-release-asset|softprops/action-gh-release" .github/workflows`

Expected: release asset names and validation commands are present.

### Task 4: Documentation and Final Verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

Explain automatic release download, local PATH override, and release asset naming.

- [ ] **Step 2: Run final verification**

Run:

```bash
rtk cargo fmt --all -- --check
rtk cargo test --all-targets
rtk cargo check -p atfile-extension --target wasm32-wasip2
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build --bin atfile-lsp
```

Expected: all pass.

- [ ] **Step 3: Commit**

Commit with:

```bash
rtk git add src/lib.rs src/installation.rs extension.toml README.md .github/workflows/ci.yml .github/workflows/release.yml docs/superpowers/plans/2026-05-12-pinned-lsp-binary-download.md
rtk git commit -m "feat: download pinned lsp release binaries"
```
