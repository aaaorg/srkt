# CI / Release Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Set up GitHub Actions CI for PR validation and a tag-triggered release pipeline that publishes native x86_64 and aarch64 Linux binaries to GitHub Releases and the crate to crates.io.

**Architecture:** Two workflow files — `ci.yml` (PR checks: fmt, clippy, build, test) and `release.yml` (tag `v*` → build both arches natively, create GitHub Release with checksums, publish to crates.io). Supporting files: `LICENSE`, `README.md`, updated `Cargo.toml` metadata.

**Tech Stack:** GitHub Actions, `dtolnay/rust-toolchain`, `actions/cache`, `actions/upload-artifact`, `actions/download-artifact`, `softprops/action-gh-release`, `cargo publish`.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modify | Add crates.io required metadata fields |
| `LICENSE` | Create | MIT license text |
| `README.md` | Create | Minimal project description for crates.io |
| `.github/workflows/ci.yml` | Create | PR validation (fmt + clippy + build + test) |
| `.github/workflows/release.yml` | Create | Tag-triggered build + GitHub Release + crates.io publish |

---

## Task 1: Update Cargo.toml metadata

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add metadata fields to `[package]`**

Open `Cargo.toml`. The current `[package]` section is:

```toml
[package]
name = "srkt"
version = "0.1.0"
edition = "2021"
```

Replace it with:

```toml
[package]
name = "srkt"
version = "0.1.0"
edition = "2021"
description = "Wayland-native text expander via evdev + uinput"
license = "MIT"
repository = "https://github.com/aaaorg/srkt"
keywords = ["wayland", "text-expander", "evdev", "uinput", "linux"]
categories = ["command-line-utilities"]
readme = "README.md"
exclude = [".github", "docs"]
```

- [ ] **Step 2: Verify cargo accepts the metadata**

```bash
cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json; p=json.load(sys.stdin)['packages'][0]; print(p['name'], p['version'], p['license'])"
```

Expected output:
```
srkt 0.1.0 MIT
```

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add crates.io metadata to Cargo.toml"
```

---

## Task 2: Create LICENSE

**Files:**
- Create: `LICENSE`

- [ ] **Step 1: Write MIT license**

Create `LICENSE` with this exact content (update year/name if needed):

```
MIT License

Copyright (c) 2026 Jakub Šindelář

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 2: Commit**

```bash
git add LICENSE
git commit -m "chore: add MIT license"
```

---

## Task 3: Create README.md

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write minimal README**

Create `README.md`:

```markdown
# srkt

Wayland-native text expander. Reads keyboard input at the kernel layer (evdev)
and injects expansions via a uinput virtual keyboard — works in every Wayland
app without application-level integration.

## Requirements

- Linux kernel with `/dev/uinput` support
- Wayland compositor (GNOME, KDE, Sway, …)
- System libraries: `libxkbcommon`, `libwayland-client`
- User must be in the `input` group (or have read access to `/dev/input/event*`)

## Install

**Pre-built binary** (recommended):

Download from [GitHub Releases](https://github.com/aaaorg/srkt/releases) and
place in `~/.local/bin/`.

**Via cargo:**

```bash
cargo install srkt
```

Requires `libxkbcommon-dev` and `libwayland-dev` (or distro equivalents).

## Usage

```toml
# ~/.config/srkt/expansions.toml
[expansions]
"/mail" = "user@example.com"
"/sig" = """
Best regards,
Jakub"""
```

```bash
srkt daemon     # start the daemon (usually via systemd user service)
srkt reload     # hot-reload config without restart
srkt status     # check if daemon is running
```

See `srkt --help` for all commands.

## License

MIT
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README"
```

---

## Task 4: Create CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create directory**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: Write `ci.yml`**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: ['feature/**']

jobs:
  check:
    name: Check
    runs-on: ubuntu-24.04

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Install system dependencies
        run: |
          sudo apt-get update -q
          sudo apt-get install -y libxkbcommon-dev libwayland-dev

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Build
        run: cargo build

      - name: Test
        run: cargo test
```

- [ ] **Step 3: Verify YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "YAML OK"
```

Expected output:
```
YAML OK
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add PR validation workflow"
```

---

## Task 5: Create release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write `release.yml`**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v[0-9]*'

jobs:
  build-x86_64:
    name: Build (x86_64-linux)
    runs-on: ubuntu-24.04

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-release-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-release-

      - name: Install system dependencies
        run: |
          sudo apt-get update -q
          sudo apt-get install -y libxkbcommon-dev libwayland-dev

      - name: Build release binary
        run: cargo build --release

      - name: Package artifact
        run: |
          TAG="${{ github.ref_name }}"
          cp target/release/srkt "srkt-${TAG}-x86_64-linux"
          sha256sum "srkt-${TAG}-x86_64-linux" > "srkt-${TAG}-x86_64-linux.sha256"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: x86_64-linux
          path: |
            srkt-*-x86_64-linux
            srkt-*-x86_64-linux.sha256

  build-aarch64:
    name: Build (aarch64-linux)
    runs-on: ubuntu-24.04-arm

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-release-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-release-

      - name: Install system dependencies
        run: |
          sudo apt-get update -q
          sudo apt-get install -y libxkbcommon-dev libwayland-dev

      - name: Build release binary
        run: cargo build --release

      - name: Package artifact
        run: |
          TAG="${{ github.ref_name }}"
          cp target/release/srkt "srkt-${TAG}-aarch64-linux"
          sha256sum "srkt-${TAG}-aarch64-linux" > "srkt-${TAG}-aarch64-linux.sha256"

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: aarch64-linux
          path: |
            srkt-*-aarch64-linux
            srkt-*-aarch64-linux.sha256

  github-release:
    name: GitHub Release
    needs: [build-x86_64, build-aarch64]
    runs-on: ubuntu-24.04
    permissions:
      contents: write

    steps:
      - name: Download x86_64 artifact
        uses: actions/download-artifact@v4
        with:
          name: x86_64-linux

      - name: Download aarch64 artifact
        uses: actions/download-artifact@v4
        with:
          name: aarch64-linux

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: |
            srkt-*-x86_64-linux
            srkt-*-x86_64-linux.sha256
            srkt-*-aarch64-linux
            srkt-*-aarch64-linux.sha256

  publish-crates:
    name: Publish to crates.io
    needs: [build-x86_64, build-aarch64]
    runs-on: ubuntu-24.04

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install system dependencies
        run: |
          sudo apt-get update -q
          sudo apt-get install -y libxkbcommon-dev libwayland-dev

      - name: Publish
        run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

- [ ] **Step 2: Verify YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo "YAML OK"
```

Expected output:
```
YAML OK
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add tag-triggered release pipeline"
```

---

## Task 6: Verify crates.io readiness

**Files:** none

- [ ] **Step 1: Run publish dry-run**

```bash
cargo publish --dry-run --allow-dirty 2>&1
```

Expected: output ends with `Uploading srkt v0.1.0` and no errors. Warnings about unpublished dependencies are OK if there are none.

If you see `error[E0...]: ...` — that is a compile error unrelated to publish config, fix it first.

If you see `error: manifest path ... does not exist` for `readme` — verify `README.md` exists in the repo root.

- [ ] **Step 2: Verify all required metadata is present**

```bash
cargo metadata --no-deps --format-version 1 | python3 -m json.tool | grep -E '"(name|version|description|license|repository|keywords|categories)"'
```

Expected output contains all six fields with non-empty values.

---

## Task 7: Branch protection setup (manual — GitHub UI)

This task cannot be automated without a GitHub token with admin scope. Do it once after pushing the workflows.

- [ ] **Step 1: Go to repository settings**

Navigate to: `https://github.com/aaaorg/srkt/settings/branches`

- [ ] **Step 2: Add branch protection rule for `main`**

Click **Add rule**, set branch name pattern to `main`, then enable:

- [x] **Require a pull request before merging**
  - Required approvals: 0 (solo project — CI is the gatekeeper, not approvals)
- [x] **Require status checks to pass before merging**
  - Add status check: `Check` (this is the job name from `ci.yml`)
- [x] **Require branches to be up to date before merging**
- [x] **Do not allow bypassing the above settings** ← uncheck this if you want admin override ability

Click **Save changes**.

---

## Cheatsheet: Cutting a release

After everything is set up, the release flow is:

```bash
# Option A — with cargo-release (install once: cargo install cargo-release)
cargo release patch   # or minor / major
git push && git push --tags

# Option B — manual
# 1. Edit Cargo.toml: bump version field
# 2. git add Cargo.toml && git commit -m "chore: release v1.1.0"
# 3. git tag v1.1.0
# 4. git push && git push --tags
```

GitHub Actions will detect the `v*` tag and run the full release pipeline automatically.
