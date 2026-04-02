use anyhow::Result;
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
        let mut watcher = match RecommendedWatcher::new(tx, NConfig::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Config watcher failed to start: {}", e);
                return;
            }
        };
        if let Err(e) = watcher.watch(&config_path, RecursiveMode::NonRecursive) {
            tracing::error!("Failed to watch config file: {}", e);
            return;
        }
        loop {
            match rx.recv() {
                Ok(_) => { let _ = watch_tx2.send(()); }
                Err(_) => break, // channel closed, exit loop cleanly
            }
        }
    });

    // Signals
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;
    let mut sig_usr1 = signal(SignalKind::user_defined1())?;

    // Keep watch_tx alive so the channel stays open
    let _watch_tx = watch_tx;

    tracing::info!("srkt daemon ready");

    loop {
        tokio::select! {
            event = kb_stream.next_event() => {
                match event {
                    Some(ev) if ev.value == 0 => {
                        // Key release — ignore
                    }
                    Some(ev) if ev.value == 1 => {
                        // Key press only (not repeat — repeat would flood the buffer)
                        handle_key_event(&ev, &mut expander, &injector);
                    }
                    Some(_) => {}
                    None => {
                        tracing::warn!("Keyboard stream ended");
                        break;
                    }
                }
            }

            Some(_) = watch_rx.recv() => {
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

    if let Some(ch) = evdev_key_to_char(ev.code) {
        if let Some(expansion) = expander.push_char(ch) {
            tracing::debug!("Expanding trigger ({} chars)", expansion.delete_count);
            expander.reset();
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
/// Only needs to cover characters that appear in trigger strings.
fn evdev_key_to_char(code: u16) -> Option<char> {
    match code {
        16 => Some('q'), 17 => Some('w'), 18 => Some('e'), 19 => Some('r'),
        20 => Some('t'), 21 => Some('y'), 22 => Some('u'), 23 => Some('i'),
        24 => Some('o'), 25 => Some('p'), 30 => Some('a'), 31 => Some('s'),
        32 => Some('d'), 33 => Some('f'), 34 => Some('g'), 35 => Some('h'),
        36 => Some('j'), 37 => Some('k'), 38 => Some('l'), 44 => Some('z'),
        45 => Some('x'), 46 => Some('c'), 47 => Some('v'), 48 => Some('b'),
        49 => Some('n'), 50 => Some('m'),
        2 => Some('1'), 3 => Some('2'), 4 => Some('3'), 5 => Some('4'),
        6 => Some('5'), 7 => Some('6'), 8 => Some('7'), 9 => Some('8'),
        10 => Some('9'), 11 => Some('0'),
        52 => Some('.'), 53 => Some('/'), 51 => Some(','), 39 => Some(';'),
        40 => Some('\''), 26 => Some('['), 27 => Some(']'), 43 => Some('\\'),
        12 => Some('-'), 13 => Some('='), 57 => Some(' '),
        _ => None,
    }
}
