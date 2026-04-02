use std::collections::HashSet;
use std::path::PathBuf;

use evdev::{Device, EventType, Key};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// A single key event forwarded from the kernel via evdev.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: u16,   // Linux evdev keycode
    pub value: i32,  // 0=release, 1=press, 2=repeat
}

/// Merges key events from all physical keyboards into a single async stream.
pub struct KeyboardStream {
    rx: mpsc::UnboundedReceiver<KeyEvent>,
}

impl KeyboardStream {
    /// Discover keyboards, start per-device reader tasks, and start the hotplug watcher.
    pub async fn new() -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel::<KeyEvent>();

        // Track which paths we have already opened so the hotplug watcher doesn't
        // re-open them.
        let mut known: HashSet<PathBuf> = HashSet::new();

        for path in discover_keyboards() {
            known.insert(path.clone());
            let tx2 = tx.clone();
            tokio::spawn(async move {
                if let Err(e) = run_device_reader(path.clone(), tx2).await {
                    warn!("device reader for {:?} exited: {}", path, e);
                }
            });
        }

        // Hotplug: watch /dev/input/ for new event files.
        // notify callbacks must be Send + 'static, so we bridge through a std channel.
        let (notify_tx, notify_rx) = std::sync::mpsc::channel::<PathBuf>();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Create(_)) {
                        for path in event.paths {
                            let _ = notify_tx.send(path);
                        }
                    }
                }
                Err(e) => warn!("inotify error: {}", e),
            }
        })?;

        watcher.watch(std::path::Path::new("/dev/input"), RecursiveMode::NonRecursive)?;

        // Bridge the blocking std::sync::mpsc into an async tokio channel so that
        // the watcher future can drive new-device spawns.
        let (hotplug_tx, mut hotplug_rx) = mpsc::unbounded_channel::<PathBuf>();
        let hotplug_tx2 = hotplug_tx.clone();

        // Keep watcher alive by moving it into the blocking thread.
        tokio::task::spawn_blocking(move || {
            // The watcher must stay alive for the duration of this thread.
            let _watcher = watcher;
            loop {
                match notify_rx.recv() {
                    Ok(path) => {
                        if hotplug_tx2.send(path).is_err() {
                            break; // async side dropped — time to stop
                        }
                    }
                    Err(_) => break, // sender (watcher) dropped
                }
            }
        });

        // Async task that handles new paths arriving from the hotplug channel.
        tokio::spawn(async move {
            while let Some(path) = hotplug_rx.recv().await {
                if !is_event_device(&path) || known.contains(&path) {
                    continue;
                }
                match Device::open(&path) {
                    Ok(dev) => {
                        if !is_keyboard(&dev) || is_virtual(&dev) {
                            continue;
                        }
                        info!("hotplug: opened keyboard {:?}", path);
                        known.insert(path.clone());
                        let tx3 = tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = run_device_reader(path.clone(), tx3).await {
                                warn!("hotplug device reader for {:?} exited: {}", path, e);
                            }
                        });
                    }
                    Err(e) => warn!("hotplug: could not open {:?}: {}", path, e),
                }
            }
        });

        Ok(Self { rx })
    }

    /// Returns the next key event, or `None` if all senders have been dropped.
    pub async fn next_event(&mut self) -> Option<KeyEvent> {
        self.rx.recv().await
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Scan `/dev/input/event*` and return paths of confirmed keyboard devices.
fn discover_keyboards() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let read_dir = match std::fs::read_dir("/dev/input") {
        Ok(rd) => rd,
        Err(e) => {
            warn!("cannot read /dev/input: {}", e);
            return paths;
        }
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !is_event_device(&path) {
            continue;
        }
        match Device::open(&path) {
            Ok(dev) => {
                if is_keyboard(&dev) && !is_virtual(&dev) {
                    info!("found keyboard: {:?} (name={:?})", path, dev.name());
                    paths.push(path);
                }
            }
            Err(e) => warn!("cannot open {:?}: {}", path, e),
        }
    }
    paths
}

/// Returns `true` if `path` looks like `/dev/input/event<N>`.
fn is_event_device(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("event"))
        .unwrap_or(false)
}

/// Returns `true` if `device` supports EV_KEY and has KEY_A, KEY_Z, and KEY_SPACE.
fn is_keyboard(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    device.supported_events().contains(EventType::KEY)
        && keys.contains(Key::KEY_A)
        && keys.contains(Key::KEY_Z)
        && keys.contains(Key::KEY_SPACE)
}

/// Returns `true` if the device name contains "virtual" (case-insensitive).
fn is_virtual(device: &Device) -> bool {
    device
        .name()
        .map(|n| n.to_ascii_lowercase().contains("virtual"))
        .unwrap_or(false)
}

/// Runs the event loop for a single keyboard device.
///
/// Opens an async `EventStream`, reads key events, and forwards them to `tx`.
/// Returns when the device is removed or `tx` is closed.
async fn run_device_reader(
    path: PathBuf,
    tx: mpsc::UnboundedSender<KeyEvent>,
) -> anyhow::Result<()> {
    let device = Device::open(&path)?;
    info!("reading events from {:?} (name={:?})", path, device.name());
    let mut stream = device.into_event_stream()?;
    loop {
        let ev = stream.next_event().await?;
        if ev.event_type() == EventType::KEY {
            let key_event = KeyEvent {
                code: ev.code(),
                value: ev.value(),
            };
            if tx.send(key_event).is_err() {
                break; // receiver dropped — shutdown
            }
        }
    }
    Ok(())
}
