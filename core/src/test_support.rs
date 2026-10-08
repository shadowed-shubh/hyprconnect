//! Deterministic building blocks for future core integration tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::TcpListener;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub struct TempConfigDir {
    path: PathBuf,
}

impl TempConfigDir {
    pub fn new() -> std::io::Result<Self> {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("hyprconnect-test-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempConfigDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[allow(dead_code)]
pub async fn ephemeral_tcp_listener() -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", 0)).await
}

#[cfg(test)]
mod tests {
    use super::TempConfigDir;

    #[test]
    fn temporary_config_directory_is_created_and_removed() {
        let path = {
            let directory = TempConfigDir::new().unwrap();
            assert!(directory.path().is_dir());
            directory.path().to_path_buf()
        };
        assert!(!path.exists());
    }
}
