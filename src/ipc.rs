use std::path::{Path, PathBuf};

use anyhow::anyhow;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> anyhow::Result<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|_| anyhow!("XDG_RUNTIME_DIR not set"))?;
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
    pub async fn new(path: &Path) -> anyhow::Result<Self> {
        // Remove stale socket file if it exists.
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        Ok(Self { listener })
    }

    pub async fn accept(&self) -> anyhow::Result<IpcCmd> {
        let (stream, _addr) = self.listener.accept().await?;
        let mut reader = BufReader::new(stream);
        let mut buf = String::new();
        reader.read_line(&mut buf).await?;
        match buf.trim() {
            "reload" => Ok(IpcCmd::Reload),
            "status" => Ok(IpcCmd::Status),
            other => Err(anyhow!("unknown IPC command: {:?}", other)),
        }
    }
}

pub async fn send_cmd(path: &Path, cmd: &str) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(path).await?;
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;
    // Signal that we are done writing.
    stream.shutdown().await?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).await?;
    Ok(response.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a temporary socket path inside a TempDir.
    fn tmp_sock(dir: &TempDir) -> PathBuf {
        dir.path().join("test.sock")
    }

    #[tokio::test]
    async fn test_server_receives_reload_command() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        let server = IpcServer::new(&path).await.unwrap();

        // Spawn a client task that sends "reload\n".
        let path_clone = path.clone();
        tokio::spawn(async move {
            send_cmd(&path_clone, "reload").await.unwrap();
        });

        let cmd = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Reload));
    }

    #[tokio::test]
    async fn test_server_receives_status_command() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        let server = IpcServer::new(&path).await.unwrap();

        let path_clone = path.clone();
        tokio::spawn(async move {
            send_cmd(&path_clone, "status").await.unwrap();
        });

        let cmd = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Status));
    }

    #[tokio::test]
    async fn test_stale_socket_is_removed_on_startup() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        // Create a stale file at the socket path.
        std::fs::write(&path, b"stale").unwrap();
        assert!(path.exists());

        // IpcServer::new should remove the stale file and bind successfully.
        let _server = IpcServer::new(&path).await.unwrap();
        // If we get here without error the stale-removal logic worked.
    }
}
