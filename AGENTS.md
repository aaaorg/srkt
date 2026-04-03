# AGENTS.md

## Project purpose

`srkt` is a Rust-based, Wayland-native text expander for Linux. It reads physical keyboard events through `evdev`, matches configured triggers, and injects expansions via a `uinput` virtual keyboard. Wayland is used only to fetch the active XKB keymap.

## Repository map

- `src/main.rs` — CLI entry point, subcommands, daemon startup, service installer
- `src/daemon.rs` — main async runtime and event loop
- `src/keyboard.rs` — physical keyboard discovery and evdev event ingestion
- `src/injector.rs` — XKB keymap handling + uinput-based text injection
- `src/expander.rs` — rolling buffer and trigger matching
- `src/config.rs` — `expansions.toml` load/save and trigger validation
- `src/ipc.rs` — UNIX socket IPC used by `reload`/`status`
- `docs/superpowers/plans/` — historical implementation plan notes
- `target/` — build artifacts, generated, never edit manually

## Commands

```bash
cargo build
cargo build --release
cargo test
cargo run -- --help
install -Dm755 target/release/srkt ~/.local/bin/srkt
systemctl --user start srkt
systemctl --user stop srkt
systemctl --user restart srkt
systemctl --user status srkt
journalctl --user -u srkt -f
```

## Runtime/config paths

- config: `~/.config/srkt/expansions.toml`
- IPC socket: `$XDG_RUNTIME_DIR/srkt.sock`
- user service: `~/.config/systemd/user/srkt.service`

## Architecture notes

- Physical keyboard input comes from `/dev/input/event*`.
- The daemon tracks modifier state and decodes keys with the actual XKB keymap.
- `Expander` performs suffix matching against a rolling character buffer.
- Injection sends backspaces first, then expanded text through a virtual keyboard.
- The uinput device name is intentionally `srkt virtual keyboard` so it can be filtered out and not re-read as physical input.

## Important constraints / pitfalls

- GNOME does not expose `zwp_virtual_keyboard_manager_v1`; do not reintroduce protocol-based injection unless compositor support is verified.
- `uinput` timing delays matter. Removing them causes modifier bleed and scrambled output.
- The Wayland keymap payload may include a trailing `\0`; trim it before building XKB structures.
- Config watcher threads must not block runtime shutdown.
- Prefix conflicts between triggers are intentionally rejected.
- Avoid editing generated files under `target/`.
- This repo may contain local, not-yet-pushed work. Check `git status --short` before making broader changes.

## Validation workflow

Before finishing code changes:

1. run `cargo test`
2. run `cargo run -- --help`
3. if service/install logic changed, review `src/main.rs` service template carefully
4. inspect `git diff --stat` and `git status --short`

## Done criteria

- changes are limited to intended files
- docs/README stay aligned with the actual CLI and architecture
- no generated artifacts are committed accidentally
- validation commands pass, unless the user explicitly asks to skip them

## GitHub access

Push access requires the `megastary` GitHub account. Always switch before pushing:

```bash
gh auth switch --user megastary
git push origin <branch>
```

The `jakubsindelar-mountfieldcz` account has read-only access to this repo.

## Branch workflow

All work goes through feature branches — never commit directly to `main`.

```bash
git checkout -b feature/<name>   # branch from main
# ... implement, commit frequently ...
git push -u origin feature/<name>
gh pr create --title "<title>" --body "$(cat <<'EOF'
## Summary
- <bullet>

## Test plan
- [ ] cargo test passes
- [ ] cargo clippy passes
EOF
)"
gh pr checks --watch             # wait for CI
gh pr merge --squash             # merge when green
```

CI on PRs runs: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`.

## Cutting a release

Tag on `main` triggers the full release pipeline (build x86_64 + aarch64, GitHub Release, crates.io publish).

```bash
# Recommended — install once: cargo install cargo-release
git checkout main && git pull
cargo release patch    # or minor / major
git push && git push --tags

# Monitor:
gh run watch
gh release list
```

Manual alternative (without cargo-release):
```bash
# 1. Edit version in Cargo.toml
# 2. git add Cargo.toml && git commit -m "chore: release vX.Y.Z"
# 3. git tag vX.Y.Z && git push && git push --tags
```

Version bump guide: `patch` = bug fix, `minor` = new feature, `major` = breaking change.

**Note:** `CARGO_REGISTRY_TOKEN` secret in GitHub Actions has "publish existing crate" scope only. First-time crate creation must be done locally with `cargo login` + `cargo publish`.
