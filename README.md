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
