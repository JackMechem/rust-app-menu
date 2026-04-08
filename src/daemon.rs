use std::sync::{Mutex, OnceLock, mpsc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const SOCKET_NAME: &str = "rust-app-menu.sock";

#[derive(Debug)]
pub enum DaemonCommand {
    Show,
    Reload,
}

pub static DAEMON_RX: OnceLock<Mutex<mpsc::Receiver<DaemonCommand>>> = OnceLock::new();

pub fn socket_path() -> String {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/{}", runtime_dir, SOCKET_NAME)
}

pub async fn is_running() -> bool {
    tokio::net::UnixStream::connect(socket_path()).await.is_ok()
}

pub async fn try_show_existing() -> bool {
    match tokio::net::UnixStream::connect(socket_path()).await {
        Ok(mut conn) => {
            let _ = conn.write_all(b"show").await;
            true
        }
        Err(_) => false,
    }
}

pub async fn try_reload_existing() -> bool {
    match tokio::net::UnixStream::connect(socket_path()).await {
        Ok(mut conn) => {
            let _ = conn.write_all(b"reload").await;
            true
        }
        Err(_) => false,
    }
}

pub async fn listen_for_commands(sender: mpsc::Sender<DaemonCommand>) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    eprintln!("[daemon] Binding socket at {}", path);
    let listener = match tokio::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[daemon] Failed to bind socket: {}", e);
            return;
        }
    };

    eprintln!("[daemon] Listening for commands...");
    loop {
        if let Ok((mut conn, _)) = listener.accept().await {
            let mut buf = [0u8; 8];
            if let Ok(n) = conn.read(&mut buf).await {
                let cmd_str = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
                let cmd = match cmd_str {
                    "reload" => DaemonCommand::Reload,
                    _ => DaemonCommand::Show,
                };
                eprintln!("[daemon] Received command: {:?}", cmd);
                let _ = sender.send(cmd);
            }
        }
    }
}
