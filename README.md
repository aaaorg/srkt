<div align="center">
  <img src="icon.svg" width="80" alt="srkt logo">
  <h1>srkt</h1>

  [![crates.io](https://img.shields.io/crates/v/srkt)](https://crates.io/crates/srkt)
  [![CI](https://github.com/aaaorg/srkt/actions/workflows/ci.yml/badge.svg)](https://github.com/aaaorg/srkt/actions/workflows/ci.yml)
  [![Release](https://github.com/aaaorg/srkt/actions/workflows/release.yml/badge.svg)](https://github.com/aaaorg/srkt/actions/workflows/release.yml)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
</div>

> **Disclaimer:** This is a 100% vibecoded project. It exists to solve one specific problem: text expansion on **Linux with GNOME and Wayland**, where [Espanso](https://espanso.org) doesn't work because GNOME doesn't expose the `zwp_virtual_keyboard_manager_v1` protocol. If you're not in that exact situation, **use Espanso** — it's actively maintained, cross-platform, and far more capable.

## Espanso vs srkt

| Feature | [Espanso](https://espanso.org) | srkt |
|---|---|---|
| Platform support | Linux, macOS, Windows | Linux only |
| Wayland + GNOME | ✗ (missing protocol) | ✓ (evdev + uinput) |
| Rich expansions (scripts, images, forms) | ✓ | ✗ |
| Trigger terminator (space/enter) | ✓ | ✗ (instant suffix match) |
| Config format | YAML | TOML |
| Active community & docs | ✓ | ✗ |

---

Wayland-native text expander. Reads keyboard input at the kernel layer (evdev)
and injects expansions via a uinput virtual keyboard — works in every Wayland
app without application-level integration.

## Requirements

- Linux kernel with `/dev/uinput` support
- Wayland compositor with GNOME Shell
- System libraries: `libxkbcommon`, `libwayland-client`
- User must be in the `input` group (or have read access to `/dev/input/event*`)

## Install

**Pre-built binary** (recommended):

Download from [GitHub Releases](https://github.com/aaaorg/srkt/releases) and place in `~/.local/bin/`.

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
