//! Unix socket utilities for daemon communication.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;
use xdg::BaseDirectories;

/// Get the daemon socket path.
pub fn socket_path() -> Result<PathBuf> {
    let xdg = BaseDirectories::with_prefix("voice-controllm");
    let state_dir = xdg
        .get_state_home()
        .context("Failed to get XDG state directory (HOME not set?)")?;
    std::fs::create_dir_all(&state_dir).context("Failed to create state directory")?;
    Ok(state_dir.join("daemon.sock"))
}

/// Get the daemon PID file path.
pub fn pid_path() -> Result<PathBuf> {
    let xdg = BaseDirectories::with_prefix("voice-controllm");
    let state_dir = xdg
        .get_state_home()
        .context("Failed to get XDG state directory (HOME not set?)")?;
    std::fs::create_dir_all(&state_dir).context("Failed to create state directory")?;
    Ok(state_dir.join("daemon.pid"))
}

/// Create a Unix listener, removing stale socket if present.
pub fn create_listener(path: &Path) -> Result<UnixListener> {
    // Remove existing socket if present
    if path.exists() {
        std::fs::remove_file(path).context("Failed to remove existing socket")?;
    }

    UnixListener::bind(path).context("Failed to bind Unix socket")
}

/// Remove the socket file.
pub fn cleanup_socket(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_in_xdg_state() {
        let path = socket_path().unwrap();
        assert!(path.to_string_lossy().contains("voice-controllm"));
        assert!(path.to_string_lossy().ends_with("daemon.sock"));
    }

    #[test]
    fn test_pid_path_in_xdg_state() {
        let path = pid_path().unwrap();
        assert!(path.to_string_lossy().contains("voice-controllm"));
        assert!(path.to_string_lossy().ends_with("daemon.pid"));
    }

    #[tokio::test]
    async fn test_create_listener() {
        let temp = tempfile::tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");

        let listener = create_listener(&sock_path).unwrap();
        assert!(sock_path.exists());

        drop(listener);
        cleanup_socket(&sock_path);
        assert!(!sock_path.exists());
    }
}
