# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release

# Test (all)
cargo test

# Test (single, with output)
cargo test test_match_on_complete_trigger -- --nocapture

# Install binary
install -Dm755 target/release/srkt ~/.local/bin/srkt

# Service management
systemctl --user start|stop|restart|status srkt
journalctl --user -u srkt -f
```

## Architecture

srkt reads physical keyboard input at the kernel layer (evdev) and injects text via a uinput virtual keyboard — bypassing Wayland's application-layer restrictions entirely. Wayland is used only to obtain the XKB keymap.

```
/dev/input/event*  ──evdev──▶  daemon (tokio)  ──uinput──▶  /dev/uinput  ──▶  all apps
  (all keyboards)               buffer+match                 (kernel vkbd)
       ▲                              │
       │ XKB keymap only              ▼
  Wayland compositor            ~/.config/srkt/expansions.toml
```

**Data flow** (in `daemon.rs` `tokio::select!` loop):
1. `keyboard.rs` streams `KeyEvent { code, value }` from all physical keyboards merged into one channel.
2. `daemon.rs` tracks `shift_held`/`altgr_held` from modifier key events, then calls `injector.keymap().from_evdev(code, shift, altgr)` to decode each keypress to a `char` using the actual XKB layout.
3. `expander.rs` maintains a `VecDeque<char>` buffer capped at the longest trigger length. Returns `Expansion { delete_count, text }` on suffix match.
4. `injector.rs` sends `delete_count` backspaces then the expansion text as uinput events.

**Two-thread injection** (`injector.rs`):
- `srkt-keymap` thread: connects to Wayland, calls `wl_seat.get_keyboard()` which causes the compositor to immediately send `wl_keyboard.keymap`, builds `KeymapLookup`, sends it over a channel, then exits.
- `srkt-uinput` thread: long-lived; owns the `VirtualDevice`, receives `InjectionCmd` over a sync channel, emits events with per-event timing delays.

The uinput device is named `"srkt virtual keyboard"` — this string causes `keyboard.rs`'s `is_virtual()` filter to exclude it from evdev reading, preventing injection loops.

## Non-obvious constraints

**GNOME does not expose `zwp_virtual_keyboard_manager_v1`** — that protocol is absent from GNOME's Wayland globals. Injection must use `/dev/uinput`, not the virtual keyboard protocol.

**uinput timing is mandatory**: without delays between injected events, modifier state (Shift, AltGr) bleeds into adjacent keys at the compositor layer, producing scrambled output. Current delays: 5ms after press, 12ms after regular release, 20ms after Shift/AltGr release.

**Wayland keymap null terminator**: the compositor sends the keymap fd with a `\0` terminator. xkbcommon uses `CString::new()` internally which rejects embedded nulls — always `trim_end_matches('\0')` before passing to `KeymapLookup::build()`.

**Config watcher must use `std::thread::spawn`**, not `tokio::task::spawn_blocking`. Tokio waits for spawn_blocking tasks on runtime drop; the file watcher loops forever, causing SIGTERM to hang.

**Espanso conflict**: Espanso uses `EVIOCGRAB` (exclusive grab) on keyboard devices. If Espanso is running, srkt receives zero events despite successfully opening the device. Disable espanso.service before running srkt.

**Prefix conflicts**: `Config::add` rejects triggers where one is a prefix of another (e.g. `/m` and `/mail`). `Expander` uses suffix matching — it fires on the last character of the trigger with no terminator key.

## Config

`~/.config/srkt/expansions.toml`:
```toml
[expansions]
"/mail" = "user@example.com"
"/sig" = """
Best regards,
Jakub"""
```

Auto-reloaded on file save (inotify watch in daemon) and on `srkt reload` (IPC over `$XDG_RUNTIME_DIR/srkt.sock`).
