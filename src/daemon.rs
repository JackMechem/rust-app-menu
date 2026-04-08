use std::sync::{Mutex, OnceLock, mpsc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const SOCKET_NAME: &str = "rust-app-menu.sock";

pub static SHOW_RX: OnceLock<Mutex<mpsc::Receiver<()>>> = OnceLock::new();

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
            eprintln!("[client] Found existing instance, sending show signal");
            let _ = conn.write_all(b"show").await;
            true
        }
        Err(e) => {
            eprintln!("[client] No existing instance ({}), starting daemon", e);
            false
        }
    }
}

pub async fn listen_for_show(sender: mpsc::Sender<()>) {
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

    eprintln!("[daemon] Listening for show signals...");
    loop {
        if let Ok((mut conn, _)) = listener.accept().await {
            eprintln!("[daemon] Got connection");
            let mut buf = [0u8; 8];
            if conn.read(&mut buf).await.is_ok() {
                eprintln!("[daemon] Received show signal, notifying iced");
                let _ = sender.send(());
            }
        }
    }
}
