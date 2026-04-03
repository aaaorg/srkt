# CI / Release Pipeline Design

**Date:** 2026-04-03
**Project:** srkt — Wayland-native text expander
**Repo:** https://github.com/aaaorg/srkt

---

## Summary

Tag-triggered CI/CD for a single-developer (or AI-agent) FOSS Rust project. Feature branches land via PR, releases are cut by pushing a semver tag to `main`. GitHub Actions builds native binaries for x86_64 and aarch64 Linux, publishes them to GitHub Releases with SHA-256 checksums, and publishes the crate to crates.io.

---

## Git Workflow

### Branch model

| Branch | Purpose |
|--------|---------|
| `main` | Always stable. Direct push disabled (branch protection). Only merged via PR. |
| `feature/<name>` | Development. Branched from `main`, deleted after merge. |

No long-lived `develop` branch. No `release/*` branches — releases are tags on `main`.

### Release flow

```
1. Merge feature PR into main
2. Bump version (locally or via agent):
     cargo release minor   # or patch / major
   → bumps Cargo.toml, commits "chore: release vX.Y.Z", creates tag vX.Y.Z
   (Alternative without cargo-release: edit Cargo.toml manually, commit, tag)
3. git push && git push --tags
4. GitHub Actions detects tag → release pipeline runs automatically
```

### Branch protection on `main`

- Require PR before merging
- Require CI (`ci.yml`) to pass before merge
- Disallow force-push
- Disallow direct commits

---

## CI Workflows

### `ci.yml` — PR checks

**Trigger:** `pull_request` targeting `main`; `push` to `feature/**`

**Runs on:** `ubuntu-24.04` (x86_64 only — fast, sufficient for validation)

**Jobs:**

```
check:
  - cargo fmt --check
  - cargo clippy -- -D warnings
  - cargo build
  - cargo test
```

All jobs run in parallel where possible. A failed job blocks the PR merge via branch protection.

---

### `release.yml` — Release pipeline

**Trigger:** `push` of tags matching `v[0-9]+.*`

**Jobs:**

```
build-x86_64
  runs-on: ubuntu-24.04
  steps:
    - Install system deps (libxkbcommon-dev, libwayland-dev)
    - cargo build --release
    - Rename binary: srkt-{tag}-x86_64-linux
    - Compute SHA-256 checksum
    - Upload as workflow artifact

build-aarch64
  runs-on: ubuntu-24.04-arm   ← native ARM runner, no cross-compilation
  steps:
    - Install system deps (same as above)
    - cargo build --release
    - Rename binary: srkt-{tag}-aarch64-linux
    - Compute SHA-256 checksum
    - Upload as workflow artifact

github-release
  needs: [build-x86_64, build-aarch64]
  steps:
    - Download both artifacts
    - Create GitHub Release for the tag
    - Attach all 4 files (2 binaries + 2 checksums)
    - Auto-generate release notes from git log

publish-crates
  needs: [build-x86_64]   ← confirms the build is clean before publishing
  steps:
    - cargo publish
    - Requires CARGO_REGISTRY_TOKEN secret
```

---

## Artifact Naming

```
srkt-v1.1.0-x86_64-linux
srkt-v1.1.0-x86_64-linux.sha256
srkt-v1.1.0-aarch64-linux
srkt-v1.1.0-aarch64-linux.sha256
```

Tag name is embedded in the filename so all release assets are unambiguous.

---

## Cargo.toml Changes

The following fields must be added for a valid `cargo publish`:

```toml
[package]
description = "Wayland-native text expander via evdev + uinput"
license = "MIT"
repository = "https://github.com/aaaorg/srkt"
keywords = ["wayland", "text-expander", "evdev", "uinput", "linux"]
categories = ["command-line-utilities"]
readme = "README.md"
exclude = [".github", "docs"]
```

A minimal `README.md` must exist in the repo root (crates.io displays it on the crate page).

---

## Secrets

| Secret name | Where to set | Purpose |
|-------------|-------------|---------|
| `CARGO_REGISTRY_TOKEN` | GitHub → Settings → Secrets → Actions | `cargo publish` authentication |
| `GITHUB_TOKEN` | Automatic (no setup needed) | Creating GitHub Releases |

---

## Files to Create

```
.github/
  workflows/
    ci.yml          ← PR validation
    release.yml     ← Tag-triggered release pipeline
LICENSE             ← MIT license text
README.md           ← Minimal project description (required for crates.io)
```

`Cargo.toml` will be updated in-place with the missing metadata fields.

---

## Agent / Developer Cheatsheet

```bash
# Daily work
git checkout -b feature/my-feature
# ... make changes ...
git push -u origin feature/my-feature
# Open PR on GitHub → CI runs → merge

# Cut a release
cargo release patch   # or minor / major
git push && git push --tags
# Done — GitHub Actions handles the rest
```
