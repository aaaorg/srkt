use std::collections::HashSet;
use std::path::PathBuf;

use evdev::{Device, EventType, Key};

/// A single key event forwarded from the kernel via evdev.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: u16,   // Linux evdev keycode
    pub value: i32,  // 0=release, 1=press, 2=repeat
}

/// Merges key events from all physical keyboards into a single async stream.
pub struct KeyboardStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
}

impl KeyboardStream {
    /// Discover keyboards, start per-device reader tasks, and start the hotplug watcher.
    pub async fn new() -> anyhow::Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<KeyEvent>();
        let (done_tx, done_rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

        // Start hotplug actor (registers watch BEFORE we scan, closing the race window)
        let tx_clone = tx.clone();
        let done_tx_clone = done_tx.clone();
        tokio::spawn(hotplug_actor(tx_clone, done_rx, done_tx_clone));

        // Small yield to let the actor register its watch before we scan
        tokio::task::yield_now().await;

        // Now scan for existing keyboards
        let keyboards = discover_keyboards().unwrap_or_default();
        tracing::info!("Found {} keyboard device(s)", keyboards.len());
        for (path, device) in keyboards {
            let tx2 = tx.clone();
            let done_tx2 = done_tx.clone();
            tokio::spawn(run_device_reader(device, path, tx2, done_tx2));
        }

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

/// Scan `/dev/input/event*` and return paths + already-opened devices for confirmed keyboards.
fn discover_keyboards() -> anyhow::Result<Vec<(PathBuf, evdev::Device)>> {
    let mut keyboards = Vec::new();
    let dir = std::fs::read_dir("/dev/input")?;
    for entry in dir.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !name.starts_with("event") { continue; }
        match evdev::Device::open(&path) {
            Ok(device) => {
                if is_keyboard(&device) && !is_virtual(&device) {
                    keyboards.push((path, device));
                }
            }
            Err(_) => {}
        }
    }
    Ok(keyboards)
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
/// Accepts an already-opened device to avoid double-open.
/// Forwards key events to `tx`, and signals `done_tx` when it exits.
async fn run_device_reader(
    device: evdev::Device,
    path: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>,
    done_tx: tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    let result = read_device_loop(device, &tx).await;
    if let Err(e) = result {
        tracing::warn!("Device reader exited for {:?}: {}", path, e);
    }
    // Notify hotplug actor that this device is no longer active
    let _ = done_tx.send(path);
}

async fn read_device_loop(
    device: evdev::Device,
    tx: &tokio::sync::mpsc::UnboundedSender<KeyEvent>,
) -> anyhow::Result<()> {
    let mut stream = device.into_event_stream()?;
    loop {
        let event = stream.next_event().await?;
        if event.event_type() == evdev::EventType::KEY {
            if tx.send(KeyEvent { code: event.code(), value: event.value() }).is_err() {
                break; // receiver dropped
            }
        }
    }
    Ok(())
}

/// Watches `/dev/input` for new keyboard devices and spawns readers for them.
/// Also tracks active devices via `done_rx` so replug works correctly.
async fn hotplug_actor(
    tx: tokio::sync::mpsc::UnboundedSender<KeyEvent>,
    mut done_rx: tokio::sync::mpsc::UnboundedReceiver<PathBuf>,
    done_tx: tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    use notify::{Config as NConfig, RecommendedWatcher, RecursiveMode, Watcher, EventKind};

    let (ntx, nrx) = std::sync::mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(ntx, NConfig::default()) {
        Ok(w) => w,
        Err(e) => { tracing::error!("Failed to create notify watcher: {}", e); return; }
    };
    if let Err(e) = watcher.watch(std::path::Path::new("/dev/input"), RecursiveMode::NonRecursive) {
        tracing::error!("Failed to watch /dev/input: {}", e); return;
    }

    let (new_device_tx, mut new_device_rx) = tokio::sync::mpsc::unbounded_channel::<std::path::PathBuf>();

    // Bridge blocking notify channel → async
    std::thread::spawn(move || {
        // Keep watcher alive for the duration of this thread.
        let _watcher = watcher;
        while let Ok(event) = nrx.recv() {
            if let Ok(ev) = event {
                if matches!(ev.kind, EventKind::Create(_)) {
                    for path in ev.paths {
                        let _ = new_device_tx.send(path);
                    }
                }
            }
        }
    });

    let mut active: HashSet<std::path::PathBuf> = HashSet::new();

    loop {
        tokio::select! {
            path = done_rx.recv() => {
                match path {
                    Some(p) => { active.remove(&p); tracing::info!("Device removed from active set: {:?}", p); }
                    None => break,
                }
            }
            path = new_device_rx.recv() => {
                match path {
                    Some(p) => {
                        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                        if !name.starts_with("event") { continue; }
                        if active.contains(&p) { continue; }
                        // Try to open and check if it's a keyboard
                        match evdev::Device::open(&p) {
                            Ok(device) if is_keyboard(&device) && !is_virtual(&device) => {
                                tracing::info!("New keyboard detected: {:?}", p);
                                active.insert(p.clone());
                                let tx2 = tx.clone();
                                let done_tx2 = done_tx.clone();
                                tokio::spawn(run_device_reader(device, p, tx2, done_tx2));
                            }
                            _ => {}
                        }
                    }
                    None => break,
                }
            }
        }
    }
}
