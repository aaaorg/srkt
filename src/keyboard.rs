#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: u16,
    pub value: i32,
}
pub struct KeyboardStream {
    rx: tokio::sync::mpsc::UnboundedReceiver<KeyEvent>,
}
impl KeyboardStream {
    pub async fn new() -> anyhow::Result<Self> { todo!() }
    pub async fn next_event(&mut self) -> Option<KeyEvent> { todo!() }
}
