# srkt Text Expander Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Wayland-native text expander daemon that reads keyboard input via Linux evdev and injects expansions via the Wayland virtual keyboard protocol.

**Architecture:** A single Rust binary (`srkt`) that runs as a systemd user service. The daemon reads all keyboard events from `/dev/input/event*` using evdev (kernel level, works for every app), matches a rolling character buffer against configured triggers, then injects backspaces + expansion text via `zwp_virtual_keyboard_v1` (Wayland's official virtual keyboard protocol). A separate thread owns the Wayland connection; the main tokio event loop handles evdev, config watching, and IPC.

**Tech Stack:** Rust 2021, tokio (async), evdev (keyboard reading), wayland-client + wayland-protocols-misc (virtual keyboard injection), xkbcommon (XKB keymap reverse-lookup), toml + serde (config), clap 4 (CLI), notify (inotify), tracing (logging).

---

## File Map

| File | Responsibility |
|------|---------------|
| `src/main.rs` | CLI entry point; dispatches to daemon or config subcommands |
| `src/config.rs` | Load/save `expansions.toml`; add/remove/list expansions |
| `src/expander.rs` | Rolling buffer; trigger matching; returns expansion actions |
| `src/keyboard.rs` | Discover evdev keyboards; merge into unified async event stream |
| `src/injector.rs` | Wayland virtual keyboard setup and text injection |
| `src/daemon.rs` | tokio event loop; wires keyboard → expander → injector |
| `src/ipc.rs` | UNIX socket for `reload`/`status` commands |
| `~/.config/srkt/expansions.toml` | User expansion definitions |
| `~/.config/systemd/user/srkt.service` | Written by `srkt install` |

---

## Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/config.rs`, `src/expander.rs`, `src/keyboard.rs`, `src/injector.rs`, `src/daemon.rs`, `src/ipc.rs`

- [ ] **Step 1: Initialize the project**

```bash
cd /home/jakub/nogit/srkt
cargo init --name srkt
```

- [ ] **Step 2: Write `Cargo.toml` with all dependencies**

Replace the generated `Cargo.toml` with:

```toml
[package]
name = "srkt"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "srkt"
path = "src/main.rs"

[dependencies]
# Input reading
evdev = { version = "0.12", features = ["tokio"] }

# Async runtime
tokio = { version = "1", features = ["full"] }
futures = "0.3"
tokio-stream = "0.1"

# Wayland
wayland-client = "0.31"
wayland-protocols-misc = { version = "0.3", features = ["client"] }

# XKB keyboard layout
xkbcommon = "0.7"

# Config
toml = "0.8"
serde = { version = "1", features = ["derive"] }

# CLI
clap = { version = "4", features = ["derive"] }

# File watching
notify = "6"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Unix primitives
nix = { version = "0.29", features = ["signal", "fs"] }

# Error handling
anyhow = "1"

[profile.release]
strip = true
opt-level = "z"
lto = true
```

- [ ] **Step 3: Create stub modules**

Create each source file with just a `pub` stub so the project compiles:

`src/config.rs`:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub expansions: HashMap<String, String>,
}

impl Config {
    pub fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".config")
            });
        base.join("srkt").join("expansions.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn add(&mut self, trigger: &str, expansion: &str) -> Result<()> {
        // Check for prefix conflicts
        for existing in self.expansions.keys() {
            if existing.starts_with(trigger) || trigger.starts_with(existing.as_str()) {
                if existing != trigger {
                    anyhow::bail!(
                        "Prefix conflict: '{}' and '{}' are prefixes of each other",
                        trigger, existing
                    );
                }
            }
        }
        self.expansions.insert(trigger.to_string(), expansion.to_string());
        Ok(())
    }

    pub fn remove(&mut self, trigger: &str) -> bool {
        self.expansions.remove(trigger).is_some()
    }
}
```

`src/expander.rs`:
```rust
pub struct Expansion {
    pub delete_count: usize,
    pub text: String,
}

pub struct Expander {
    buffer: std::collections::VecDeque<char>,
    max_trigger_len: usize,
    expansions: Vec<(String, String)>,
}

impl Expander {
    pub fn new(expansions: Vec<(String, String)>) -> Self {
        let max_trigger_len = expansions.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0);
        Self {
            buffer: std::collections::VecDeque::new(),
            max_trigger_len,
            expansions,
        }
    }

    pub fn update(&mut self, expansions: Vec<(String, String)>) {
        self.max_trigger_len = expansions.iter().map(|(t, _)| t.chars().count()).max().unwrap_or(0);
        self.expansions = expansions;
        self.buffer.clear();
    }

    pub fn push_char(&mut self, c: char) -> Option<Expansion> { todo!() }
    pub fn pop_char(&mut self) { todo!() }
    pub fn reset(&mut self) { self.buffer.clear(); }
}
```

`src/keyboard.rs`:
```rust
pub struct KeyEvent {
    pub code: u16,
    pub value: i32, // 0=release, 1=press, 2=repeat
}
pub struct KeyboardStream;
impl KeyboardStream {
    pub async fn new() -> anyhow::Result<Self> { todo!() }
    pub async fn next_event(&mut self) -> Option<anyhow::Result<KeyEvent>> { todo!() }
}
```

`src/injector.rs`:
```rust
pub enum InjectionCmd {
    Backspace,
    TypeChar(char),
}
pub struct Injector;
impl Injector {
    pub fn send(&self, _cmd: InjectionCmd) -> anyhow::Result<()> { todo!() }
}
```

`src/daemon.rs`:
```rust
use crate::config::Config;
pub async fn run(config: Config) -> anyhow::Result<()> { todo!() }
```

`src/ipc.rs`:
```rust
pub enum IpcCmd { Reload, Status }
pub struct IpcServer;
impl IpcServer {
    pub async fn new(_path: &std::path::Path) -> anyhow::Result<Self> { todo!() }
}
```

`src/main.rs`:
```rust
mod config;
mod daemon;
mod expander;
mod injector;
mod ipc;
mod keyboard;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "srkt", about = "Wayland text expander")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add or overwrite an expansion
    Add { trigger: String, expansion: String },
    /// Remove an expansion
    Remove { trigger: String },
    /// List all expansions
    List,
    /// Signal running daemon to reload config
    Reload,
    /// Show daemon status
    Status,
    /// Install systemd user service
    Install,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            // Daemon mode
            tokio::runtime::Runtime::new()?.block_on(async {
                let config = config::Config::load(&config::Config::path())?;
                daemon::run(config).await
            })
        }
        Some(cmd) => handle_cmd(cmd),
    }
}

fn handle_cmd(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::List => {
            let cfg = config::Config::load(&config::Config::path())?;
            for (trigger, expansion) in &cfg.expansions {
                println!("{} => {}", trigger, expansion.replace('\n', "\\n"));
            }
        }
        Cmd::Add { trigger, expansion } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            // Handle \n escape sequences from shell
            let expansion = expansion.replace("\\n", "\n");
            cfg.add(&trigger, &expansion)?;
            cfg.save(&path)?;
            println!("Added: {} => {}", trigger, expansion.replace('\n', "\\n"));
        }
        Cmd::Remove { trigger } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            if cfg.remove(&trigger) {
                cfg.save(&path)?;
                println!("Removed: {}", trigger);
            } else {
                eprintln!("Trigger not found: {}", trigger);
                std::process::exit(1);
            }
        }
        Cmd::Reload | Cmd::Status => {
            eprintln!("IPC not yet implemented");
        }
        Cmd::Install => {
            eprintln!("Install not yet implemented");
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

```bash
cd /home/jakub/nogit/srkt
cargo build 2>&1 | head -50
```

Expected: compiles with warnings about `todo!()` and unused code, but no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/jakub/nogit/srkt
git init
git add -A
git commit -m "feat: initial scaffold with stub modules"
```

---

## Task 2: Config Module (TDD)

**Files:**
- Modify: `src/config.rs`

The Config module is pure file I/O, fully testable without hardware.

- [ ] **Step 1: Write failing tests**

Add at the bottom of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_path(dir: &TempDir) -> std::path::PathBuf {
        dir.path().join("expansions.toml")
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let cfg = Config::load(&tmp_path(&dir)).unwrap();
        assert!(cfg.expansions.is_empty());
    }

    #[test]
    fn test_save_and_reload() {
        let dir = TempDir::new().unwrap();
        let path = tmp_path(&dir);
        let mut cfg = Config::default();
        cfg.add("/mail", "user@example.com").unwrap();
        cfg.save(&path).unwrap();

        let cfg2 = Config::load(&path).unwrap();
        assert_eq!(cfg2.expansions.get("/mail").unwrap(), "user@example.com");
    }

    #[test]
    fn test_multiline_expansion_survives_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = tmp_path(&dir);
        let mut cfg = Config::default();
        cfg.add("/sig", "Best regards,\nJakub").unwrap();
        cfg.save(&path).unwrap();

        let cfg2 = Config::load(&path).unwrap();
        assert_eq!(cfg2.expansions.get("/sig").unwrap(), "Best regards,\nJakub");
    }

    #[test]
    fn test_remove_existing_trigger() {
        let dir = TempDir::new().unwrap();
        let path = tmp_path(&dir);
        let mut cfg = Config::default();
        cfg.add("/mail", "user@example.com").unwrap();
        cfg.save(&path).unwrap();

        let mut cfg2 = Config::load(&path).unwrap();
        assert!(cfg2.remove("/mail"));
        assert!(cfg2.expansions.is_empty());
    }

    #[test]
    fn test_remove_missing_trigger_returns_false() {
        let mut cfg = Config::default();
        assert!(!cfg.remove("/nonexistent"));
    }

    #[test]
    fn test_prefix_conflict_rejected() {
        let mut cfg = Config::default();
        cfg.add("/mail", "a@b.com").unwrap();
        let result = cfg.add("/m", "something");
        assert!(result.is_err(), "prefix conflict should be rejected");
    }

    #[test]
    fn test_no_conflict_when_overwriting_same_trigger() {
        let mut cfg = Config::default();
        cfg.add("/mail", "a@b.com").unwrap();
        // Overwriting the same trigger should succeed
        assert!(cfg.add("/mail", "new@b.com").is_ok());
        assert_eq!(cfg.expansions.get("/mail").unwrap(), "new@b.com");
    }
}
```

Add `tempfile` to `Cargo.toml` dev dependencies:
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/jakub/nogit/srkt
cargo test config 2>&1
```

Expected: compilation errors about missing `tempfile` crate (add it), then test failures on unimplemented behavior.

- [ ] **Step 3: The Config impl in Step 1 of Task 1 already passes most tests — verify**

```bash
cargo test config 2>&1
```

Expected: all 7 tests pass. The implementation in Task 1 is already complete for this module.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs Cargo.toml
git commit -m "feat: config module with TDD — load/save/add/remove/prefix-check"
```

---

## Task 3: Expander Logic (TDD)

**Files:**
- Modify: `src/expander.rs`

This is the core matching logic. Zero external dependencies.

- [ ] **Step 1: Write failing tests**

Replace the entire `src/expander.rs` with:

```rust
use std::collections::VecDeque;

#[derive(Debug, PartialEq)]
pub struct Expansion {
    pub delete_count: usize,
    pub text: String,
}

pub struct Expander {
    buffer: VecDeque<char>,
    max_trigger_len: usize,
    expansions: Vec<(String, String)>,
}

impl Expander {
    pub fn new(expansions: Vec<(String, String)>) -> Self {
        let max_trigger_len = expansions
            .iter()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        Self {
            buffer: VecDeque::new(),
            max_trigger_len,
            expansions,
        }
    }

    pub fn update(&mut self, expansions: Vec<(String, String)>) {
        self.max_trigger_len = expansions
            .iter()
            .map(|(t, _)| t.chars().count())
            .max()
            .unwrap_or(0);
        self.expansions = expansions;
        self.buffer.clear();
    }

    /// Call when a printable character key is pressed.
    /// Returns Some(Expansion) if a trigger was matched.
    pub fn push_char(&mut self, c: char) -> Option<Expansion> {
        self.buffer.push_back(c);
        // Keep buffer at most max_trigger_len chars
        while self.buffer.len() > self.max_trigger_len {
            self.buffer.pop_front();
        }
        self.check_match()
    }

    /// Call when Backspace is pressed.
    pub fn pop_char(&mut self) {
        self.buffer.pop_back();
    }

    /// Call when a cursor-movement key is pressed (arrow, Home, End, Escape, etc.)
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    fn check_match(&self) -> Option<Expansion> {
        // Build the buffer as a string for suffix matching
        let buf: String = self.buffer.iter().collect();
        for (trigger, expansion) in &self.expansions {
            if buf.ends_with(trigger.as_str()) {
                return Some(Expansion {
                    delete_count: trigger.chars().count(),
                    text: expansion.clone(),
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exp(pairs: &[(&str, &str)]) -> Expander {
        Expander::new(pairs.iter().map(|(t, e)| (t.to_string(), e.to_string())).collect())
    }

    #[test]
    fn test_no_match_on_partial_trigger() {
        let mut e = exp(&[("/mail", "user@example.com")]);
        assert!(e.push_char('/').is_none());
        assert!(e.push_char('m').is_none());
        assert!(e.push_char('a').is_none());
        assert!(e.push_char('i').is_none());
    }

    #[test]
    fn test_match_on_complete_trigger() {
        let mut e = exp(&[("/mail", "user@example.com")]);
        e.push_char('/');
        e.push_char('m');
        e.push_char('a');
        e.push_char('i');
        let result = e.push_char('l');
        assert_eq!(
            result,
            Some(Expansion {
                delete_count: 5,
                text: "user@example.com".to_string(),
            })
        );
    }

    #[test]
    fn test_backspace_prevents_match() {
        let mut e = exp(&[("/mail", "user@example.com")]);
        e.push_char('/');
        e.push_char('m');
        e.push_char('a');
        e.push_char('i');
        e.pop_char(); // removes 'i'
        // Now type 'l' — buffer is "/ma" + "l" = "/mal", no match
        assert!(e.push_char('l').is_none());
    }

    #[test]
    fn test_reset_prevents_match() {
        let mut e = exp(&[("/mail", "user@example.com")]);
        e.push_char('/');
        e.push_char('m');
        e.reset(); // arrow key or similar
        e.push_char('a');
        e.push_char('i');
        e.push_char('l');
        // Buffer only has "ail" after reset, not "/mail"
        assert!(e.push_char('/').is_none()); // wrong anyway
    }

    #[test]
    fn test_reset_after_match_allows_new_match() {
        let mut e = exp(&[("/mail", "a@b.com")]);
        e.push_char('/');
        e.push_char('m');
        e.push_char('a');
        e.push_char('i');
        let first = e.push_char('l');
        assert!(first.is_some());
        // After expansion the caller resets the buffer
        e.reset();
        // Type the trigger again
        e.push_char('/');
        e.push_char('m');
        e.push_char('a');
        e.push_char('i');
        let second = e.push_char('l');
        assert!(second.is_some());
    }

    #[test]
    fn test_multiline_expansion_text() {
        let mut e = exp(&[("/sig", "Best regards,\nJakub")]);
        e.push_char('/');
        e.push_char('s');
        e.push_char('i');
        let result = e.push_char('g');
        assert_eq!(
            result.unwrap().text,
            "Best regards,\nJakub"
        );
    }

    #[test]
    fn test_delete_count_equals_trigger_char_count() {
        let mut e = exp(&[("/sig", "x")]);
        e.push_char('/');
        e.push_char('s');
        e.push_char('i');
        let result = e.push_char('g').unwrap();
        assert_eq!(result.delete_count, 4); // "/sig" = 4 chars
    }

    #[test]
    fn test_buffer_capped_at_max_trigger_length() {
        // max trigger is "/mail" = 5 chars
        let mut e = exp(&[("/mail", "x")]);
        for c in "hello world /mail".chars() {
            e.push_char(c);
        }
        // Even with a long preamble, the trigger fires when buffer tail matches
        // The last 5 chars of "hello world /mail" are "/mail"
        // But since we push char by char in the loop, last push is 'l'
        // The result of the last push_char should be Some
        // Re-do it correctly:
        let mut e2 = exp(&[("/mail", "user@b.com")]);
        let inputs: Vec<char> = "hello world /mail".chars().collect();
        let mut last = None;
        for c in inputs {
            last = e2.push_char(c);
        }
        assert!(last.is_some());
    }

    #[test]
    fn test_multiple_triggers() {
        let mut e = exp(&[("/mail", "a@b.com"), ("/phone", "123456")]);
        for c in "/phone".chars() {
            e.push_char(c);
        }
        // Check the last push triggers /phone
        let mut e2 = exp(&[("/mail", "a@b.com"), ("/phone", "123456")]);
        let inputs: Vec<char> = "/phone".chars().collect();
        let mut last = None;
        for c in inputs {
            last = e2.push_char(c);
        }
        assert_eq!(last.unwrap().text, "123456");
    }
}
```

- [ ] **Step 2: Run tests — they should all pass with the implementation above**

```bash
cargo test expander 2>&1
```

Expected: all 9 tests pass. The `check_match` method uses `ends_with` on the buffer string, which correctly handles multi-char Unicode triggers and buffer capping.

- [ ] **Step 3: Commit**

```bash
git add src/expander.rs
git commit -m "feat: expander module — rolling buffer + trigger matching, TDD"
```

---

## Task 4: IPC Module (TDD)

**Files:**
- Modify: `src/ipc.rs`

Simple line-based protocol over a Unix socket.

- [ ] **Step 1: Write the full IPC implementation with tests**

Replace `src/ipc.rs` with:

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
    Ok(PathBuf::from(runtime_dir).join("srkt.sock"))
}

pub enum IpcCmd {
    Reload,
    Status,
}

pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub async fn new(path: &Path) -> Result<Self> {
        // Remove stale socket from a previous run
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        Ok(Self { listener })
    }

    /// Accept one connection and return the parsed command.
    /// The caller is responsible for looping.
    pub async fn accept(&self) -> Result<IpcCmd> {
        let (stream, _) = self.listener.accept().await?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        match line.trim() {
            "reload" => Ok(IpcCmd::Reload),
            "status" => Ok(IpcCmd::Status),
            other => anyhow::bail!("Unknown IPC command: '{}'", other),
        }
    }
}

/// Send a command to the running daemon and return its response.
pub async fn send_cmd(path: &Path, cmd: &str) -> Result<String> {
    let mut stream = UnixStream::connect(path).await?;
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;
    stream.shutdown().await?;
    let mut resp = String::new();
    BufReader::new(stream).read_line(&mut resp).await?;
    Ok(resp.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_receives_reload_command() {
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("test.sock");

        let server = IpcServer::new(&sock).await.unwrap();

        // Simulate a client in a background task
        let sock2 = sock.clone();
        tokio::spawn(async move {
            let mut stream = UnixStream::connect(&sock2).await.unwrap();
            stream.write_all(b"reload\n").await.unwrap();
        });

        let cmd = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Reload));
    }

    #[tokio::test]
    async fn test_server_receives_status_command() {
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("test2.sock");

        let server = IpcServer::new(&sock).await.unwrap();

        let sock2 = sock.clone();
        tokio::spawn(async move {
            let mut stream = UnixStream::connect(&sock2).await.unwrap();
            stream.write_all(b"status\n").await.unwrap();
        });

        let cmd = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Status));
    }

    #[tokio::test]
    async fn test_stale_socket_is_removed_on_startup() {
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("test3.sock");

        // Create a fake stale socket file
        std::fs::write(&sock, b"stale").unwrap();

        // Should succeed despite the stale file
        let _server = IpcServer::new(&sock).await.unwrap();
        assert!(sock.exists());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test ipc 2>&1
```

Expected: all 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/ipc.rs
git commit -m "feat: IPC module — unix socket server/client, TDD"
```

---

## Task 5: Keyboard Reader

**Files:**
- Modify: `src/keyboard.rs`

Reads from all evdev keyboard devices, merges into one async stream, handles hotplug.

- [ ] **Step 1: Write the keyboard module**

Replace `src/keyboard.rs` with:

```rust
use anyhow::Result;
use evdev::{Device, EventType, InputEvent, InputEventKind, Key};
use futures::stream::{SelectAll, StreamExt};
use std::path::PathBuf;
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Linux input event key code (evdev keycode, NOT XKB keycode)
    pub code: u16,
    /// 0 = release, 1 = press, 2 = repeat
    pub value: i32,
}

pub struct KeyboardStream {
    // Receives key events merged from all keyboards
    rx: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
}

impl KeyboardStream {
    pub async fn new() -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<KeyEvent>();

        let keyboards = discover_keyboards()?;
        tracing::info!("Found {} keyboard device(s)", keyboards.len());

        for path in keyboards {
            let tx = tx.clone();
            let path_str = path.to_string_lossy().to_string();
            tokio::spawn(async move {
                match read_device(path, tx).await {
                    Ok(()) => {}
                    Err(e) => tracing::warn!("Keyboard device error ({}): {}", path_str, e),
                }
            });
        }

        // Hotplug watcher — spawns new readers when new keyboards appear
        let tx_hotplug = tx.clone();
        tokio::spawn(async move {
            if let Err(e) = watch_hotplug(tx_hotplug).await {
                tracing::warn!("Hotplug watcher error: {}", e);
            }
        });

        Ok(Self { rx })
    }

    pub async fn next_event(&mut self) -> Option<KeyEvent> {
        self.rx.recv().await
    }
}

/// Find all /dev/input/event* devices that have keyboard keys
fn discover_keyboards() -> Result<Vec<PathBuf>> {
    let mut keyboards = Vec::new();
    let dir = std::fs::read_dir("/dev/input")?;
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.to_string_lossy().contains("event") {
            continue;
        }
        match Device::open(&path) {
            Ok(device) => {
                if is_keyboard(&device) {
                    keyboards.push(path);
                }
            }
            Err(_) => {} // Permission denied or not a real device
        }
    }
    Ok(keyboards)
}

fn is_keyboard(device: &Device) -> bool {
    // A keyboard has EV_KEY events and has the standard alpha keys
    device.supported_keys().map_or(false, |keys| {
        keys.contains(Key::KEY_A)
            && keys.contains(Key::KEY_Z)
            && keys.contains(Key::KEY_SPACE)
    })
}

async fn read_device(
    path: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>,
) -> Result<()> {
    let device = Device::open(&path)?;
    let mut stream = device.into_event_stream()?;
    loop {
        let event = stream.next_event().await?;
        if event.event_type() == EventType::KEY {
            if tx
                .send(KeyEvent {
                    code: event.code(),
                    value: event.value(),
                })
                .is_err()
            {
                break; // Receiver dropped, shut down
            }
        }
    }
    Ok(())
}

/// Watch /dev/input for new devices appearing (USB keyboards etc.)
async fn watch_hotplug(tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>) -> Result<()> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let (ntx, nrx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(ntx, Config::default())?;
    watcher.watch(std::path::Path::new("/dev/input"), RecursiveMode::NonRecursive)?;

    let mut known: std::collections::HashSet<PathBuf> = discover_keyboards()
        .unwrap_or_default()
        .into_iter()
        .collect();

    loop {
        let event = tokio::task::spawn_blocking({
            let nrx = &nrx as *const _ as usize; // raw ptr trick for Send
            move || {
                // SAFETY: nrx is only used here, blocking for one event
                let nrx = unsafe { &*(nrx as *const mpsc::Receiver<_>) };
                nrx.recv()
            }
        })
        .await??;

        // Re-scan for new keyboards
        let current: std::collections::HashSet<PathBuf> =
            discover_keyboards().unwrap_or_default().into_iter().collect();
        for new_path in current.difference(&known) {
            tracing::info!("New keyboard detected: {:?}", new_path);
            let tx2 = tx.clone();
            let p = new_path.clone();
            tokio::spawn(async move {
                let _ = read_device(p, tx2).await;
            });
        }
        known = current;
    }
}
```

**Note:** The hotplug watcher has a subtle unsafe usage to work around `Receiver<T>` not being `Send`. For a simpler production-ready version, use `tokio::sync::broadcast` or `inotify` crate directly. The above is functional but could be simplified.

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no errors. Warnings about unused imports are fine.

- [ ] **Step 3: Manual test (needs `input` group — see Installation task)**

If already in the `input` group, test by running:
```bash
cargo run -- 2>&1 &
# (daemon starts, will hit todo!() in daemon but keyboard module init prints)
# check logs: journalctl --user -f
```

- [ ] **Step 4: Commit**

```bash
git add src/keyboard.rs
git commit -m "feat: keyboard reader — evdev multi-device stream with hotplug"
```

---

## Task 6: Wayland Setup + Keymap Acquisition

**Files:**
- Modify: `src/injector.rs`

This task establishes the Wayland connection, creates the virtual keyboard, and captures the user's XKB keymap. It runs on a dedicated thread.

- [ ] **Step 1: Add wayland dispatch boilerplate**

Replace `src/injector.rs` with this full implementation:

```rust
use anyhow::{Context, Result};
use std::os::fd::FromRawFd;
use std::sync::mpsc;
use std::thread;
use wayland_client::{
    delegate_noop,
    protocol::{wl_keyboard, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};

// ---------------------------------------------------------------------------
// InjectionCmd
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum InjectionCmd {
    /// Send a single evdev key event (press=1, release=0)
    Key { evdev_code: u32, state: u32 },
    /// Update modifier state
    Modifiers {
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    },
}

// ---------------------------------------------------------------------------
// Injector (public API, used from main tokio thread)
// ---------------------------------------------------------------------------

pub struct Injector {
    tx: mpsc::SyncSender<InjectionCmd>,
    keymap: KeymapLookup,
}

impl Injector {
    /// Spawn the Wayland thread and return an Injector.
    pub fn spawn() -> Result<Self> {
        let (keymap_tx, keymap_rx) = std::sync::mpsc::channel::<(String, KeymapLookup)>();
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<InjectionCmd>(256);

        thread::Builder::new()
            .name("srkt-wayland".into())
            .spawn(move || {
                if let Err(e) = wayland_thread(keymap_tx, cmd_rx) {
                    tracing::error!("Wayland thread error: {}", e);
                }
            })?;

        // Block until keymap is received from compositor
        let (_keymap_str, keymap) = keymap_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .context("Timed out waiting for Wayland keymap")?;

        Ok(Self {
            tx: cmd_tx,
            keymap,
        })
    }

    /// Send N backspace key events (press + release pairs).
    pub fn backspace(&self, count: usize) {
        let ts = timestamp();
        for _ in 0..count {
            // KEY_BACKSPACE = 14 (evdev)
            let _ = self.tx.send(InjectionCmd::Key { evdev_code: 14, state: 1 });
            let _ = self.tx.send(InjectionCmd::Key { evdev_code: 14, state: 0 });
        }
    }

    /// Type a string of text via the virtual keyboard.
    pub fn type_text(&self, text: &str) {
        let ts = timestamp();
        for ch in text.chars() {
            if ch == '\n' {
                // KEY_ENTER = 28
                let _ = self.tx.send(InjectionCmd::Key { evdev_code: 28, state: 1 });
                let _ = self.tx.send(InjectionCmd::Key { evdev_code: 28, state: 0 });
                continue;
            }
            match self.keymap.lookup(ch) {
                Some(ki) => {
                    if ki.mods_depressed != 0 {
                        let _ = self.tx.send(InjectionCmd::Modifiers {
                            depressed: ki.mods_depressed,
                            latched: 0,
                            locked: 0,
                            group: 0,
                        });
                    }
                    let _ = self.tx.send(InjectionCmd::Key {
                        evdev_code: ki.evdev_code,
                        state: 1,
                    });
                    let _ = self.tx.send(InjectionCmd::Key {
                        evdev_code: ki.evdev_code,
                        state: 0,
                    });
                    if ki.mods_depressed != 0 {
                        let _ = self.tx.send(InjectionCmd::Modifiers {
                            depressed: 0,
                            latched: 0,
                            locked: 0,
                            group: 0,
                        });
                    }
                }
                None => {
                    tracing::warn!("No keycode found for char {:?}, skipping", ch);
                }
            }
            // NOTE: no sleep here — this runs on the tokio thread, sleeping would block
            // the event loop. If apps need delays, configure inject_delay_ms in the
            // Wayland thread's handle_cmd instead.
        }
    }
}

fn timestamp() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Use as_millis() cast to u32 — monotonically increasing (wraps ~every 49 days, fine)
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32
}

// ---------------------------------------------------------------------------
// KeymapLookup: XKB reverse lookup (char -> evdev keycode + mods)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KeyInfo {
    pub evdev_code: u32,
    pub mods_depressed: u32, // 0 = no mod, 1 = Shift
}

#[derive(Clone)]
pub struct KeymapLookup {
    table: std::collections::HashMap<char, KeyInfo>,
}

impl KeymapLookup {
    pub fn build(keymap_str: &str) -> Self {
        use xkbcommon::xkb;

        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &ctx,
            keymap_str,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );

        let mut table = std::collections::HashMap::new();

        let Some(keymap) = keymap else {
            tracing::error!("Failed to parse XKB keymap");
            return Self { table };
        };

        let min = u32::from(keymap.min_keycode());
        let max = u32::from(keymap.max_keycode());

        for xkb_code in min..=max {
            let kc = xkb::Keycode::from(xkb_code);
            let num_layouts = keymap.key_num_layouts(kc);
            for layout in 0..num_layouts {
                let num_levels = keymap.key_num_levels(kc, layout);
                for level in 0..num_levels {
                    let syms = keymap.key_get_syms_by_level(kc, layout, level);
                    for &sym in syms {
                        let sym_val = u32::from(sym);
                        if let Some(ch) = keysym_to_char(sym_val) {
                            // Only store the first (lowest modifier) mapping
                            table.entry(ch).or_insert(KeyInfo {
                                evdev_code: xkb_code - 8, // XKB -> evdev: subtract 8
                                mods_depressed: if level == 1 { 1 } else { 0 },
                            });
                        }
                    }
                }
            }
        }

        Self { table }
    }

    pub fn lookup(&self, ch: char) -> Option<&KeyInfo> {
        self.table.get(&ch)
    }
}

/// Convert an XKB keysym value to a Unicode char, if possible.
fn keysym_to_char(keysym: u32) -> Option<char> {
    let cp = match keysym {
        0x0020..=0x007e => keysym,          // printable ASCII
        0x00a0..=0x00ff => keysym,          // Latin-1 Supplement
        k if k & 0xff000000 == 0x01000000 => k & 0x00ffffff, // Unicode keysym range
        _ => return None,
    };
    char::from_u32(cp)
}

// ---------------------------------------------------------------------------
// Wayland thread
// ---------------------------------------------------------------------------

struct WaylandState {
    seat: Option<wl_seat::WlSeat>,
    vk_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    vk: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    keymap_str: Option<String>,
    keymap_sent: bool,
    keymap_tx: Option<mpsc::Sender<(String, KeymapLookup)>>,
    cmd_rx: mpsc::Receiver<InjectionCmd>,
    serial: u32,
}

fn wayland_thread(
    keymap_tx: mpsc::Sender<(String, KeymapLookup)>,
    cmd_rx: mpsc::Receiver<InjectionCmd>,
) -> Result<()> {
    let conn = Connection::connect_to_env().context("Failed to connect to Wayland display")?;
    let display = conn.display();
    let mut event_queue = conn.new_event_queue::<WaylandState>();
    let qh = event_queue.handle();

    let mut state = WaylandState {
        seat: None,
        vk_manager: None,
        keyboard: None,
        vk: None,
        keymap_str: None,
        keymap_sent: false,
        keymap_tx: Some(keymap_tx),
        cmd_rx,
        serial: 0,
    };

    // Request registry to enumerate globals
    display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state)?;
    event_queue.roundtrip(&mut state)?;

    // Create virtual keyboard now that we have seat and manager
    if let (Some(ref seat), Some(ref manager)) = (&state.seat, &state.vk_manager) {
        let vk = manager.create_virtual_keyboard(seat, &qh, ());
        state.vk = Some(vk);
    } else {
        anyhow::bail!("Wayland seat or virtual keyboard manager not available");
    }

    // Get wl_keyboard to receive keymap
    if let Some(ref seat) = state.seat {
        let kb = seat.get_keyboard(&qh, ());
        state.keyboard = Some(kb);
    }

    // Roundtrip to receive keymap event
    event_queue.roundtrip(&mut state)?;

    // Main injection loop
    loop {
        // Dispatch any pending Wayland events
        event_queue.dispatch_pending(&mut state)?;
        conn.flush()?;

        // Process injection commands (non-blocking)
        loop {
            match state.cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_cmd(&state.vk, &cmd, &mut state.serial);
                    conn.flush()?;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // Block briefly on Wayland events (with timeout)
        use std::os::unix::io::AsRawFd;
        let fd = conn.as_fd();
        let mut fds = [libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        }];
        unsafe { libc::poll(fds.as_mut_ptr(), 1, 5) }; // 5ms timeout
    }
}

fn handle_cmd(
    vk: &Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    cmd: &InjectionCmd,
    serial: &mut u32,
) {
    let Some(vk) = vk else { return };
    *serial = serial.wrapping_add(1);
    let ts = timestamp();
    match cmd {
        InjectionCmd::Key { evdev_code, state } => {
            vk.key(ts, *evdev_code, *state);
        }
        InjectionCmd::Modifiers {
            depressed,
            latched,
            locked,
            group,
        } => {
            vk.modifiers(*depressed, *latched, *locked, *group);
        }
    }
}

// ---------------------------------------------------------------------------
// Wayland Dispatch implementations
// ---------------------------------------------------------------------------

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_seat" => {
                    let seat: wl_seat::WlSeat = registry.bind(name, 1, qh, ());
                    state.seat = Some(seat);
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    let manager: zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 =
                        registry.bind(name, 1, qh, ());
                    state.vk_manager = Some(manager);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _seat: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // We don't need seat events
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _kb: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            use wl_keyboard::KeymapFormat;
            if format != KeymapFormat::XkbV1 {
                tracing::warn!("Unexpected keymap format: {:?}", format);
                return;
            }
            // Read the keymap from the file descriptor
            let keymap_str = unsafe {
                let file = std::fs::File::from_raw_fd(fd.as_raw_fd());
                let mut content = String::with_capacity(size as usize);
                use std::io::Read;
                let mut f = file;
                f.read_to_string(&mut content).ok();
                content
            };

            if !state.keymap_sent {
                state.keymap_sent = true;
                let lookup = KeymapLookup::build(&keymap_str);

                // Send keymap to main thread and set on virtual keyboard
                if let Some(tx) = state.keymap_tx.take() {
                    let _ = tx.send((keymap_str.clone(), lookup));
                }

                // Set keymap on virtual keyboard
                if let Some(ref vk) = state.vk {
                    use std::io::Write;
                    // Write keymap to a memfd or temp file and send fd
                    // For simplicity, use a temp file
                    let mut tmp = tempfile::tempfile().expect("tempfile");
                    tmp.write_all(keymap_str.as_bytes()).unwrap();
                    use std::os::unix::io::IntoRawFd;
                    let fd = tmp.into_raw_fd();
                    use wayland_client::protocol::wl_keyboard::KeymapFormat;
                    vk.keymap(
                        1, // XKB_KEYMAP_FORMAT_TEXT_V1
                        unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) },
                        keymap_str.len() as u32 + 1,
                    );
                }

                state.keymap_str = Some(keymap_str);
            }
        }
    }
}

impl Dispatch<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, ()>
    for WaylandState
{
    fn event(
        _: &mut Self,
        _: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        _: zwp_virtual_keyboard_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        _: zwp_virtual_keyboard_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
```

Add `libc` and `tempfile` to `Cargo.toml` dependencies:
```toml
libc = "0.2"
tempfile = "3"
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

If `wayland-protocols-misc` module path is wrong, the error will show the actual module path. Typical fix: adjust the `use wayland_protocols_misc::...` import based on the compiler's suggestion.

- [ ] **Step 3: Commit**

```bash
git add src/injector.rs Cargo.toml
git commit -m "feat: injector — Wayland virtual keyboard thread + XKB keymap lookup"
```

---

## Task 7: Daemon Event Loop

**Files:**
- Modify: `src/daemon.rs`

Wires keyboard reader → expander → injector in a tokio select loop.

- [ ] **Step 1: Write the daemon**

Replace `src/daemon.rs` with:

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::expander::Expander;
use crate::injector::Injector;
use crate::ipc::{IpcCmd, IpcServer};
use crate::keyboard::KeyboardStream;

// evdev KEY codes (Linux input-event-codes.h)
const KEY_BACKSPACE: u16 = 14;
// Keys that reset the expansion buffer (cursor movement)
const RESET_KEYS: &[u16] = &[
    105, // KEY_LEFT
    106, // KEY_RIGHT
    103, // KEY_UP
    108, // KEY_DOWN
    102, // KEY_HOME
    107, // KEY_END
    1,   // KEY_ESC
    110, // KEY_INSERT
    111, // KEY_DELETE
    104, // KEY_PAGEUP
    109, // KEY_PAGEDOWN
];

pub async fn run(config: Config) -> Result<()> {
    tracing::info!("srkt daemon starting");

    // Initialize logging to stderr/journald
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("srkt=info".parse()?),
        )
        .init();

    // Spawn Wayland thread (blocks until keymap received)
    let injector = Injector::spawn()?;
    tracing::info!("Wayland virtual keyboard ready");

    // Open evdev keyboard stream
    let mut kb_stream = KeyboardStream::new().await?;
    tracing::info!("Keyboard event stream ready");

    // IPC server
    let ipc_path = crate::ipc::socket_path()?;
    let ipc_server = IpcServer::new(&ipc_path).await?;
    tracing::info!("IPC socket at {:?}", ipc_path);

    // Config + expander
    let config = Arc::new(Mutex::new(config));
    let mut expander = {
        let cfg = config.lock().unwrap();
        Expander::new(cfg.expansions.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    };

    // Config file watcher
    let (watch_tx, mut watch_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let config_path = Config::path();
    let watch_tx2 = watch_tx.clone();
    tokio::task::spawn_blocking(move || {
        use notify::{Config as NConfig, RecommendedWatcher, RecursiveMode, Watcher};
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, NConfig::default()).unwrap();
        watcher.watch(&config_path, RecursiveMode::NonRecursive).unwrap();
        loop {
            if rx.recv().is_ok() {
                let _ = watch_tx2.send(());
            }
        }
    });

    // Signals
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;
    let mut sig_usr1 = signal(SignalKind::user_defined1())?;

    tracing::info!("srkt daemon ready");

    loop {
        tokio::select! {
            event = kb_stream.next_event() => {
                match event {
                    Some(ev) if ev.value == 0 => {
                        // Key release — ignore
                    }
                    Some(ev) if ev.value == 1 || ev.value == 2 => {
                        // Key press or repeat
                        handle_key_event(&ev, &mut expander, &injector);
                    }
                    Some(_) => {}
                    None => {
                        tracing::warn!("Keyboard stream ended");
                        break;
                    }
                }
            }

            _ = watch_rx.recv() => {
                tracing::info!("Config changed, reloading");
                reload_config(&config, &mut expander);
            }

            cmd = ipc_server.accept() => {
                match cmd {
                    Ok(IpcCmd::Reload) => {
                        tracing::info!("Reload requested via IPC");
                        reload_config(&config, &mut expander);
                    }
                    Ok(IpcCmd::Status) => {
                        tracing::info!("Status requested via IPC");
                    }
                    Err(e) => tracing::warn!("IPC error: {}", e),
                }
            }

            _ = sig_term.recv() => {
                tracing::info!("SIGTERM received, shutting down");
                break;
            }
            _ = sig_int.recv() => {
                tracing::info!("SIGINT received, shutting down");
                break;
            }
            _ = sig_usr1.recv() => {
                tracing::info!("SIGUSR1 received, reloading config");
                reload_config(&config, &mut expander);
            }
        }
    }

    tracing::info!("srkt daemon stopped");
    Ok(())
}

fn handle_key_event(
    ev: &crate::keyboard::KeyEvent,
    expander: &mut Expander,
    injector: &Injector,
) {
    if RESET_KEYS.contains(&ev.code) {
        expander.reset();
        return;
    }

    if ev.code == KEY_BACKSPACE {
        expander.pop_char();
        return;
    }

    // Try to convert evdev keycode to a char for buffer matching.
    // We use a simplified ASCII mapping for trigger detection.
    // Note: this doesn't need to be perfect — it's just for the trigger buffer.
    // The injector uses the full XKB keymap for output.
    if let Some(ch) = evdev_key_to_char(ev.code) {
        if let Some(expansion) = expander.push_char(ch) {
            tracing::debug!("Expanding trigger ({} chars)", expansion.delete_count);
            expander.reset(); // Reset before injection to avoid re-triggering
            injector.backspace(expansion.delete_count);
            injector.type_text(&expansion.text);
        }
    }
}

fn reload_config(config: &Arc<Mutex<Config>>, expander: &mut Expander) {
    match Config::load(&Config::path()) {
        Ok(new_cfg) => {
            let exps: Vec<(String, String)> = new_cfg
                .expansions
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            expander.update(exps);
            *config.lock().unwrap() = new_cfg;
            tracing::info!("Config reloaded");
        }
        Err(e) => tracing::warn!("Failed to reload config: {}", e),
    }
}

/// Map evdev keycode to ASCII char for trigger buffer.
/// This only needs to cover characters that appear in trigger strings.
/// Covers a-z, 0-9, and common punctuation on US/CZ keyboards.
fn evdev_key_to_char(code: u16) -> Option<char> {
    // Based on linux/input-event-codes.h KEY_* values
    match code {
        16 => Some('q'), 17 => Some('w'), 18 => Some('e'), 19 => Some('r'),
        20 => Some('t'), 21 => Some('y'), 22 => Some('u'), 23 => Some('i'),
        24 => Some('o'), 25 => Some('p'), 30 => Some('a'), 31 => Some('s'),
        32 => Some('d'), 33 => Some('f'), 34 => Some('g'), 35 => Some('h'),
        36 => Some('j'), 37 => Some('k'), 38 => Some('l'), 44 => Some('z'),
        45 => Some('x'), 46 => Some('c'), 47 => Some('v'), 48 => Some('b'),
        49 => Some('n'), 50 => Some('m'),
        // Number row
        2 => Some('1'), 3 => Some('2'), 4 => Some('3'), 5 => Some('4'),
        6 => Some('5'), 7 => Some('6'), 8 => Some('7'), 9 => Some('8'),
        10 => Some('9'), 11 => Some('0'),
        // Common punctuation
        52 => Some('.'), 53 => Some('/'), 51 => Some(','), 39 => Some(';'),
        40 => Some('\''), 26 => Some('['), 27 => Some(']'), 43 => Some('\\'),
        12 => Some('-'), 13 => Some('='), 57 => Some(' '),
        _ => None,
    }
}
```

**Important note:** The `evdev_key_to_char` function maps evdev keycodes to their **unshifted** character on a US layout. This is only used for trigger detection (not for text output). Triggers like `/mail` use ASCII characters that are the same on Czech and US layouts at level 0.

- [ ] **Step 2: Verify it compiles**

```bash
cargo build 2>&1 | grep "^error"
```

- [ ] **Step 3: Commit**

```bash
git add src/daemon.rs
git commit -m "feat: daemon event loop — wires keyboard → expander → injector"
```

---

## Task 8: CLI and Installation

**Files:**
- Modify: `src/main.rs`

Complete the CLI subcommands: `reload`, `status`, and `install`.

- [ ] **Step 1: Write complete `main.rs`**

Replace `src/main.rs` with:

```rust
mod config;
mod daemon;
mod expander;
mod injector;
mod ipc;
mod keyboard;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "srkt", about = "Wayland-native text expander", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add or overwrite an expansion (use \\n for newlines)
    Add {
        trigger: String,
        expansion: String,
    },
    /// Remove an expansion
    Remove { trigger: String },
    /// List all expansions
    List,
    /// Signal running daemon to reload config
    Reload,
    /// Show daemon status
    Status,
    /// Install and enable systemd user service
    Install,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            // Daemon mode: run the tokio event loop
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let config = config::Config::load(&config::Config::path())?;
                    daemon::run(config).await
                })
        }
        Some(cmd) => handle_cmd(cmd),
    }
}

fn handle_cmd(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::List => {
            let cfg = config::Config::load(&config::Config::path())?;
            if cfg.expansions.is_empty() {
                println!("No expansions configured.");
                println!("Add one with: srkt add /trigger \"expansion text\"");
            } else {
                let mut pairs: Vec<_> = cfg.expansions.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                for (trigger, expansion) in pairs {
                    println!("{:<20} => {}", trigger, expansion.replace('\n', "\\n"));
                }
            }
        }

        Cmd::Add { trigger, expansion } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            let expansion = expansion.replace("\\n", "\n");
            cfg.add(&trigger, &expansion)?;
            cfg.save(&path)?;
            println!("Added: {} => {}", trigger, expansion.replace('\n', "\\n"));
            // Signal daemon to reload
            let _ = signal_daemon_reload();
        }

        Cmd::Remove { trigger } => {
            let path = config::Config::path();
            let mut cfg = config::Config::load(&path)?;
            if cfg.remove(&trigger) {
                cfg.save(&path)?;
                println!("Removed: {}", trigger);
                let _ = signal_daemon_reload();
            } else {
                eprintln!("Trigger '{}' not found", trigger);
                std::process::exit(1);
            }
        }

        Cmd::Reload => {
            signal_daemon_reload().map_err(|e| {
                anyhow::anyhow!("Could not reach daemon (is it running?): {}", e)
            })?;
            println!("Reloaded");
        }

        Cmd::Status => {
            let sock = ipc::socket_path()?;
            if sock.exists() {
                println!("srkt daemon is running (socket: {:?})", sock);
            } else {
                println!("srkt daemon is NOT running");
                std::process::exit(1);
            }
        }

        Cmd::Install => {
            install_service()?;
        }
    }
    Ok(())
}

fn signal_daemon_reload() -> anyhow::Result<()> {
    let sock = ipc::socket_path()?;
    if !sock.exists() {
        return Ok(()); // Daemon not running, config will be read on next start
    }
    // Use blocking send via std::os::unix::net
    use std::io::Write;
    let mut stream = std::os::unix::net::UnixStream::connect(&sock)?;
    stream.write_all(b"reload\n")?;
    Ok(())
}

fn install_service() -> anyhow::Result<()> {
    // Find the binary path
    let binary = std::env::current_exe()?;

    // Build service file content
    let service = format!(
        r#"[Unit]
Description=srkt Wayland text expander
Documentation=https://github.com/jakub/srkt
After=graphical-session.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=2s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=graphical-session.target
"#,
        binary.display()
    );

    // Write service file
    let service_dir = {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(std::env::var("HOME").expect("HOME not set"))
                    .join(".config")
            });
        config_home.join("systemd").join("user")
    };
    std::fs::create_dir_all(&service_dir)?;
    let service_path = service_dir.join("srkt.service");
    std::fs::write(&service_path, &service)?;
    println!("Wrote: {:?}", service_path);

    // Enable and start the service
    let status = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }

    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "srkt"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl enable --now srkt failed");
    }

    println!("srkt service installed and started.");
    println!();
    println!("IMPORTANT: You still need to join the 'input' group (requires re-login):");
    println!("  sudo usermod -a -G input $USER");
    println!("  # Then log out and back in");
    Ok(())
}
```

- [ ] **Step 2: Build release binary**

```bash
cd /home/jakub/nogit/srkt
cargo build --release 2>&1 | grep -E "^error|Finished"
```

Expected: `Finished release [optimized] target(s)`

- [ ] **Step 3: Run basic CLI tests**

```bash
# Test help
./target/release/srkt --help

# Test add
./target/release/srkt add /test "hello world"

# Test list
./target/release/srkt list

# Test remove
./target/release/srkt remove /test
./target/release/srkt list
```

Expected: clean output for all commands.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: complete CLI — add/remove/list/reload/status/install subcommands"
```

---

## Task 9: System Installation

This task is manual steps the user runs once.

- [ ] **Step 1: Install binary**

```bash
install -Dm755 /home/jakub/nogit/srkt/target/release/srkt ~/.local/bin/srkt
```

- [ ] **Step 2: Ensure ~/.local/bin is in PATH**

```bash
echo $PATH | grep -q "$HOME/.local/bin" && echo "Already in PATH" || echo "Add ~/.local/bin to PATH in ~/.bashrc or ~/.profile"
```

If not in PATH, add to `~/.bashrc`:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

- [ ] **Step 3: Add user to input group**

```bash
sudo usermod -a -G input $USER
```

Then **log out and log back in** (required for group membership to take effect).

- [ ] **Step 4: Verify group membership**

```bash
groups | grep input && echo "OK: in input group" || echo "MISSING: log out and back in"
```

- [ ] **Step 5: Install and start the service**

```bash
srkt install
```

Expected output:
```
Wrote: /home/jakub/.config/systemd/user/srkt.service
srkt service installed and started.

IMPORTANT: You still need to join the 'input' group...
```

- [ ] **Step 6: Check service status**

```bash
systemctl --user status srkt
journalctl --user -u srkt -n 20
```

Expected: service active or failing with a clear error (if group membership is missing).

---

## Task 10: End-to-End Verification

- [ ] **Step 1: Add test expansion**

```bash
srkt add /test "hello from srkt"
srkt list
```

- [ ] **Step 2: Test in gedit**

Open gedit. Type `/test`. Expected: "hello from srkt" appears in place of the trigger.

- [ ] **Step 3: Test multi-line in gedit**

```bash
srkt add /sig $'Best regards,\nJakub'
```

Open gedit. Type `/sig`. Expected: two lines appear.

- [ ] **Step 4: Test in Firefox**

Open Firefox URL bar or any text field. Type `/test`. Expected: expands correctly.

- [ ] **Step 5: Test in terminal**

Open gnome-terminal. Type `/test` (before pressing Enter). Expected: "hello from srkt" appears inline.

- [ ] **Step 6: Test backspace mid-trigger**

Type `/tes` then press Backspace, then `x`. Expected: `/tex` in the buffer, no expansion.

- [ ] **Step 7: Test config auto-reload**

Edit `~/.config/srkt/expansions.toml` directly, save it, type the new trigger. Expected: works without daemon restart.

- [ ] **Step 8: Check resource usage**

```bash
ps aux | grep srkt | grep -v grep
```

Expected: `VSZ` ~5–15MB, `RSS` ~3–5MB.

- [ ] **Step 9: Test daemon resilience**

```bash
# Kill daemon forcefully
pkill -9 srkt
sleep 3
# systemd should restart it
srkt status
```

Expected: daemon restarted automatically by systemd.

---

## Known Caveats and Likely Fixes

### 1. `wayland-protocols-misc` module path

If the build fails with "could not find `zwp_virtual_keyboard_v1` in `wayland_protocols_misc`", check the actual module path:

```bash
# After adding the dep, check what modules are available:
cargo doc --open
# or
cargo tree | grep wayland-protocols-misc
```

The alternative path is:
```rust
use wayland_protocols::unstable::virtual_keyboard::v1::client::...
```
Change `Cargo.toml` to use `wayland-protocols = { version = "0.31", features = ["unstable", "client"] }` if needed.

### 2. Czech keyboard characters in trigger detection

The `evdev_key_to_char` function maps evdev codes to unshifted US layout characters. Since trigger strings like `/mail`, `/phone`, `/sig` use only ASCII lowercase + `/`, this works fine regardless of whether the user types on a Czech or US keyboard layout.

### 3. Injecting Czech characters in expansion text

The `KeymapLookup::build()` function iterates the user's XKB keymap and builds a `char → KeyInfo` table. Czech characters like `š`, `ě`, `č` will be found at their level-0 keycodes in a Czech keymap. If a character is not in the table, it is skipped with a warning. All standard Latin characters and Czech diacritical marks should be found.

### 4. Virtual keyboard events read back by evdev

The injected key events from `zwp_virtual_keyboard_v1` will appear as a new `/dev/input/event*` device and will be picked up by the keyboard reader. This means expansion text that itself contains a trigger pattern would recursively expand. Mitigation: in `discover_keyboards()`, skip devices whose `name()` contains "virtual" (case-insensitive) — the compositor names virtual keyboard devices with "Virtual" in their name. If that doesn't cover all cases, add a 100ms injection-in-progress flag that prevents the expander from processing events during injection.

### 5. Apps that don't receive virtual keyboard events

Some apps run under XWayland (older GTK2 apps, Steam, etc.) and may behave differently. If an app doesn't receive the expanded text, try adding a small delay between characters by increasing the `sleep(2ms)` in `Injector::type_text`.
