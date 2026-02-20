//! Unix socket utilities for daemon communication.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;

/// Get the daemon socket path.
pub fn socket_path() -> Result<PathBuf> {
    vcm_common::dirs::socket_path()
}

/// Get the daemon PID file path.
pub fn pid_path() -> Result<PathBuf> {
    Ok(crate::dirs::state_dir()?.join("daemon.pid"))
}

/// Get the daemon log file path.
pub fn log_path() -> Result<PathBuf> {
    Ok(crate::dirs::state_dir()?.join("daemon.log"))
}

/// Create a Unix listener, removing stale socket if present.
pub fn create_listener(path: impl AsRef<Path>) -> Result<UnixListener> {
    let path = path.as_ref();
    // Remove existing socket if present
    if path.exists() {
        std::fs::remove_file(path).context("Failed to remove existing socket")?;
    }

    UnixListener::bind(path).context("Failed to bind Unix socket")
}

/// Remove the socket file.
pub fn cleanup_socket(path: impl AsRef<Path>) {
    let _ = std::fs::remove_file(path.as_ref());
}

#[cfg(test)]
#[path = "socket_test.rs"]
mod tests;
