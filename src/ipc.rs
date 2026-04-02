use std::path::{Path, PathBuf};
pub fn socket_path() -> anyhow::Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
    Ok(PathBuf::from(runtime_dir).join("srkt.sock"))
}
pub enum IpcCmd { Reload, Status }
pub struct IpcServer { listener: tokio::net::UnixListener }
impl IpcServer {
    pub async fn new(_path: &Path) -> anyhow::Result<Self> { todo!() }
    pub async fn accept(&self) -> anyhow::Result<IpcCmd> { todo!() }
}
pub async fn send_cmd(_path: &Path, _cmd: &str) -> anyhow::Result<String> { todo!() }
