use std::path::PathBuf;

use anyhow::{Context, Result};

fn state_dir() -> Result<PathBuf> {
    xdg::BaseDirectories::with_prefix("voice-controllm")
        .get_state_home()
        .context("Failed to determine XDG state directory (HOME not set?)")
}

/// Daemon Unix socket path.
pub fn socket_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("daemon.sock"))
}

/// Daemon PID file path.
pub fn pid_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("daemon.pid"))
}
