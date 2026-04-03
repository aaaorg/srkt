<div align="center">
  <img src="icon.svg" width="150" alt="srkt logo">

[![crates.io](https://img.shields.io/crates/v/srkt)](https://crates.io/crates/srkt)
[![CI](https://github.com/aaaorg/srkt/actions/workflows/ci.yml/badge.svg)](https://github.com/aaaorg/srkt/actions/workflows/ci.yml)
[![Release](https://github.com/aaaorg/srkt/actions/workflows/release.yml/badge.svg)](https://github.com/aaaorg/srkt/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

</div>

> **Disclaimer:** This is a 100% vibecoded project. It exists to solve one specific problem: text expansion on **Linux with GNOME and Wayland**, where [Espanso](https://espanso.org) doesn't work reliably as it stops working all of a sudden quite often. If you're not in that exact situation, **use Espanso** — it's actively maintained, cross-platform, and far more capable.

## Espanso vs srkt

| Feature                                  | [Espanso](https://espanso.org) | srkt                     |
| ---------------------------------------- | ------------------------------ | ------------------------ |
| Platform support                         | Linux, macOS, Windows          | Linux only               |
| Wayland + GNOME                          | ✗ (missing protocol)           | ✓ (evdev + uinput)       |
| Rich expansions (scripts, images, forms) | ✓                              | ✗                        |
| Trigger terminator (space/enter)         | ✓                              | ✗ (instant suffix match) |
| Config format                            | YAML                           | TOML                     |
| Active community & docs                  | ✓                              | ✗                        |

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

Download the latest release from [GitHub Releases](https://github.com/aaaorg/srkt/releases),
then place it in your `PATH`:

```bash
# Example for x86_64
curl -L https://github.com/aaaorg/srkt/releases/latest/download/srkt-x86_64-linux \
  -o ~/.local/bin/srkt
chmod +x ~/.local/bin/srkt
```

**Via cargo:**

```bash
cargo install srkt
```

Requires `libxkbcommon-dev` and `libwayland-dev` (or distro equivalents).

## Setup

After installing the binary, set up the systemd user service and join the `input` group:

```bash
srkt install                    # writes .service file, enables and starts the daemon
sudo usermod -a -G input $USER  # grant access to /dev/input/event*
# Log out and back in for the group change to take effect
```

To verify the daemon is running:

```bash
srkt status
journalctl --user -u srkt -f    # live logs
```

## Usage

### Config file

```toml
# ~/.config/srkt/expansions.toml
[expansions]
"/mail" = "user@example.com"
"/sig" = """
Best regards,
Jakub"""
```

The config is hot-reloaded on file save and after any `srkt add` / `srkt remove`.

### CLI reference

```bash
srkt list                        # show all triggers and expansions
srkt add '/trigger' 'text'       # add or overwrite an expansion
srkt add '/sig' 'Line1\nLine2'   # multiline: use \n in the argument
srkt remove '/trigger'           # remove an expansion
srkt reload                      # signal the daemon to reload config
srkt status                      # check if the daemon is running
srkt install                     # install and enable the systemd user service
```

> **Note:** `srkt add` and `srkt remove` automatically signal the daemon to reload.
> Use `srkt reload` only when you edit the config file manually.

### Prefix conflicts

Triggers must not be prefixes of each other — `srkt add` will reject the pair `/m`
and `/mail` because `/m` fires instantly and `/mail` would never be reachable.

## AI agent skill

If you use an AI coding assistant (Claude Code, Copilot, etc.) you can give it a
skill so it knows how to manage your srkt shortcuts via the CLI.

See [`skills/srkt-shortcuts.md`](skills/srkt-shortcuts.md) for the ready-to-use
skill file and installation instructions.

## License

MIT
