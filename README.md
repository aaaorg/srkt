# srkt

**srkt** is a Wayland-native text expander for Linux written in Rust.

It reads real keyboard events from the **evdev** layer, matches configured triggers, and injects the expansion back through a **uinput virtual keyboard**. In practice that means snippet expansion works across applications without depending on app-specific integrations.

> A slightly over-engineered, deeply practical text expander for people who were told Wayland makes this annoying.

## Why it exists

Typical text expanders often struggle on Wayland because application-level keyboard injection is restricted.

`srkt` takes a lower-level route:

- reads physical keyboard input from `/dev/input/event*`
- decodes keys using the active XKB layout
- detects matching triggers in a rolling buffer
- sends backspaces + expanded text through a virtual keyboard

This makes it work globally, not just inside a single editor.

## Features

- global text expansion on Linux/Wayland
- Rust CLI + daemon in a single binary
- config stored in `~/.config/srkt/expansions.toml`
- live config reload on save
- `srkt reload`, `srkt list`, `srkt add`, `srkt remove`, `srkt status`
- systemd user service installation via `srkt install`
- XKB-aware character decoding for non-US layouts

## How it works

```text
/dev/input/event*  ->  srkt daemon  ->  rolling trigger matcher  ->  /dev/uinput  ->  focused app
                        |                                         
                        +-> XKB keymap from Wayland compositor
```

Wayland is used only to obtain the keyboard layout/keymap. Text injection itself is done through `uinput`, which avoids compositor-specific application APIs.

## Build

```bash
cargo build
cargo build --release
```

## Run

Daemon mode:

```bash
cargo run
# or
./target/release/srkt
```

CLI help:

```bash
srkt --help
```

## Install locally

Build the release binary and copy it somewhere in your PATH:

```bash
cargo build --release
install -Dm755 target/release/srkt ~/.local/bin/srkt
```

Then install the user service:

```bash
srkt install
```

## Configuration

Expansions live in:

```text
~/.config/srkt/expansions.toml
```

Example:

```toml
[expansions]
"/mail" = "user@example.com"
"/sig" = """
Best regards,
Jakub
"""
```

Common commands:

```bash
srkt add /mail "user@example.com"
srkt add /sig "Best regards,\nJakub"
srkt list
srkt reload
srkt remove /mail
srkt status
```

## Important runtime notes

- `srkt` needs access to `/dev/input/event*` and `/dev/uinput`
- if another tool grabs keyboard devices exclusively, `srkt` may receive no events
- the current implementation intentionally avoids Wayland virtual keyboard protocols and uses `uinput` for compatibility with GNOME

## Development

```bash
cargo test
cargo run -- --help
```

For agent instructions and repo workflow notes, see [`AGENTS.md`](./AGENTS.md).

## License

MIT
