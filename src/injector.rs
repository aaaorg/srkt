use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::io::RawFd;
use std::sync::mpsc;
use std::thread;
use wayland_client::{
    protocol::{wl_keyboard, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1, zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct KeyInfo {
    pub evdev_code: u32,
    pub mods_depressed: u32,
}

#[derive(Clone)]
pub struct KeymapLookup {
    table: HashMap<char, KeyInfo>,
}

impl KeymapLookup {
    pub fn build(keymap_str: &str) -> Self {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let Some(keymap) = xkb::Keymap::new_from_string(
            &ctx,
            keymap_str.to_string(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            tracing::error!("Failed to parse XKB keymap string");
            return Self {
                table: HashMap::new(),
            };
        };

        let mut table: HashMap<char, KeyInfo> = HashMap::new();

        keymap.key_for_each(|km, kc| {
            let xkb_code = kc.raw();
            if xkb_code < 8 {
                return;
            } // below minimum
            let evdev_code = xkb_code - 8;
            let num_layouts = km.num_layouts_for_key(kc);
            for layout in 0..num_layouts {
                let num_levels = km.num_levels_for_key(kc, layout);
                for level in 0..num_levels {
                    for sym in km.key_get_syms_by_level(kc, layout, level) {
                        if let Some(ch) = keysym_to_char(*sym) {
                            table.entry(ch).or_insert(KeyInfo {
                                evdev_code,
                                mods_depressed: if level == 1 { 1 } else { 0 },
                            });
                        }
                    }
                }
            }
        });

        Self { table }
    }

    pub fn lookup(&self, ch: char) -> Option<&KeyInfo> {
        self.table.get(&ch)
    }
}

fn keysym_to_char(keysym: xkb::Keysym) -> Option<char> {
    let cp = xkb::keysym_to_utf32(keysym);
    if cp == 0 {
        return None;
    }
    char::from_u32(cp)
}

// ---------------------------------------------------------------------------
// Injection commands
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum InjectionCmd {
    Key { evdev_code: u32, state: u32 },
    Modifiers { depressed: u32, latched: u32, locked: u32, group: u32 },
}

// ---------------------------------------------------------------------------
// Injector — main-thread handle
// ---------------------------------------------------------------------------

pub struct Injector {
    tx: mpsc::SyncSender<InjectionCmd>,
    keymap: KeymapLookup,
}

impl Injector {
    pub fn spawn() -> Result<Self> {
        let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapLookup>();
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<InjectionCmd>(256);

        thread::Builder::new()
            .name("srkt-wayland".into())
            .spawn(move || {
                if let Err(e) = wayland_thread(keymap_tx, cmd_rx) {
                    tracing::error!("Wayland thread exited with error: {}", e);
                }
            })
            .context("Failed to spawn Wayland thread")?;

        let keymap = keymap_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .context("Timed out waiting for Wayland keymap (is WAYLAND_DISPLAY set?)")?;

        tracing::info!(
            "Keymap received, {} chars in lookup table",
            keymap.table.len()
        );
        Ok(Self { tx: cmd_tx, keymap })
    }

    pub fn backspace(&self, count: usize) {
        for _ in 0..count {
            let _ = self
                .tx
                .send(InjectionCmd::Key { evdev_code: 14, state: 1 }); // press
            let _ = self
                .tx
                .send(InjectionCmd::Key { evdev_code: 14, state: 0 }); // release
        }
    }

    pub fn type_text(&self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                let _ = self
                    .tx
                    .send(InjectionCmd::Key { evdev_code: 28, state: 1 });
                let _ = self
                    .tx
                    .send(InjectionCmd::Key { evdev_code: 28, state: 0 });
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
                None => tracing::warn!("No keycode for char {:?}, skipping", ch),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Wayland thread
// ---------------------------------------------------------------------------

struct WaylandState {
    seat: Option<wl_seat::WlSeat>,
    vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    vk: Option<ZwpVirtualKeyboardV1>,
    keymap_str: Option<String>,
    keymap_sent: bool,
    keymap_tx: Option<mpsc::Sender<KeymapLookup>>,
    cmd_rx: mpsc::Receiver<InjectionCmd>,
}

fn wayland_thread(
    keymap_tx: mpsc::Sender<KeymapLookup>,
    cmd_rx: mpsc::Receiver<InjectionCmd>,
) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("Failed to connect to Wayland display — is WAYLAND_DISPLAY set?")?;
    let display = conn.display();
    let mut event_queue = conn.new_event_queue::<WaylandState>();
    let qh = event_queue.handle();

    let mut state = WaylandState {
        seat: None,
        vk_manager: None,
        vk: None,
        keymap_str: None,
        keymap_sent: false,
        keymap_tx: Some(keymap_tx),
        cmd_rx,
    };

    display.get_registry(&qh, ());
    event_queue.roundtrip(&mut state)?;
    event_queue.roundtrip(&mut state)?;

    // Create virtual keyboard
    match (&state.seat, &state.vk_manager) {
        (Some(seat), Some(manager)) => {
            let vk = manager.create_virtual_keyboard(seat, &qh, ());
            state.vk = Some(vk);
        }
        _ => anyhow::bail!("Wayland: missing seat or virtual keyboard manager"),
    }

    // Get wl_keyboard to receive keymap
    if let Some(ref seat) = state.seat.clone() {
        seat.get_keyboard(&qh, ());
    }

    // Receive keymap via roundtrip
    event_queue.roundtrip(&mut state)?;

    if !state.keymap_sent {
        anyhow::bail!("Wayland: no keymap received from compositor");
    }

    // Main loop
    loop {
        event_queue.dispatch_pending(&mut state)?;
        conn.flush()?;

        loop {
            match state.cmd_rx.try_recv() {
                Ok(cmd) => {
                    handle_cmd(state.vk.as_ref(), &cmd);
                    conn.flush()?;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // Poll with 5ms timeout
        let raw_fd = conn.as_fd().as_raw_fd();
        let mut pollfd = libc::pollfd {
            fd: raw_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe {
            libc::poll(&mut pollfd as *mut libc::pollfd, 1, 5);
        }
    }
}

fn handle_cmd(vk: Option<&ZwpVirtualKeyboardV1>, cmd: &InjectionCmd) {
    let Some(vk) = vk else { return };
    let ts = timestamp();
    match *cmd {
        InjectionCmd::Key { evdev_code, state } => {
            vk.key(ts, evdev_code, state);
        }
        InjectionCmd::Modifiers { depressed, latched, locked, group } => {
            vk.modifiers(depressed, latched, locked, group);
        }
    }
}

fn timestamp() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u32
}

fn set_keymap_on_vk(vk: &ZwpVirtualKeyboardV1, keymap_str: &str) -> Result<()> {
    let bytes = keymap_str.as_bytes();
    // Use memfd_create via libc for an anonymous in-memory file
    let name = std::ffi::CString::new("srkt-keymap").unwrap();
    let fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        anyhow::bail!("memfd_create failed");
    }
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut file = std::fs::File::from(owned);
    file.write_all(bytes)?;
    file.write_all(b"\0")?; // null terminator required by wl_keyboard protocol
    // Virtual keyboard expects the fd; transfer ownership
    let raw: RawFd = file.into_raw_fd();
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    // format=1 is XKB_V1, size includes null terminator per protocol convention
    vk.keymap(1, owned.as_fd(), (bytes.len() + 1) as u32);
    // Keep owned alive until after the call
    drop(owned);
    Ok(())
}

// ---------------------------------------------------------------------------
// Dispatch implementations
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
        let wl_registry::Event::Global { name, interface, version: _ } = event else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => {
                let seat: wl_seat::WlSeat = registry.bind(name, 1, qh, ());
                state.seat = Some(seat);
            }
            "zwp_virtual_keyboard_manager_v1" => {
                let mgr: ZwpVirtualKeyboardManagerV1 = registry.bind(name, 1, qh, ());
                state.vk_manager = Some(mgr);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _kb: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format: _, fd, size } = event {
            if state.keymap_sent {
                return;
            }

            // Read keymap string from fd
            let keymap_str = {
                use std::io::Read;
                let mut file = unsafe { std::fs::File::from_raw_fd(fd.into_raw_fd()) };
                let mut s = String::with_capacity(size as usize);
                let _ = file.read_to_string(&mut s);
                s
            };

            // Set keymap on virtual keyboard
            if let Some(ref vk) = state.vk {
                if let Err(e) = set_keymap_on_vk(vk, &keymap_str) {
                    tracing::error!("Failed to set virtual keyboard keymap: {}", e);
                }
            }

            // Build lookup table and send to main thread
            let lookup = KeymapLookup::build(&keymap_str);
            if let Some(tx) = state.keymap_tx.take() {
                let _ = tx.send(lookup);
            }
            state.keymap_str = Some(keymap_str);
            state.keymap_sent = true;
        }
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardManagerV1,
        _: zwp_virtual_keyboard_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpVirtualKeyboardV1,
        _: zwp_virtual_keyboard_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
