pub struct Injector {
    tx: std::sync::mpsc::SyncSender<InjectionCmd>,
    keymap: KeymapLookup,
}
#[derive(Debug)]
pub enum InjectionCmd {
    Key { evdev_code: u32, state: u32 },
    Modifiers { depressed: u32, latched: u32, locked: u32, group: u32 },
}
#[derive(Clone)]
pub struct KeyInfo { pub evdev_code: u32, pub mods_depressed: u32 }
#[derive(Clone)]
pub struct KeymapLookup { table: std::collections::HashMap<char, KeyInfo> }
impl KeymapLookup {
    pub fn build(_keymap_str: &str) -> Self { Self { table: Default::default() } }
    pub fn lookup(&self, ch: char) -> Option<&KeyInfo> { self.table.get(&ch) }
}
impl Injector {
    pub fn spawn() -> anyhow::Result<Self> { todo!() }
    pub fn backspace(&self, _count: usize) { todo!() }
    pub fn type_text(&self, _text: &str) { todo!() }
}
