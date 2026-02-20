//! XDG directory utilities for vcm.

use std::path::PathBuf;

use anyhow::{Context, Result};
use xdg::BaseDirectories;

const APP_NAME: &str = "vcm";

fn base_dirs() -> BaseDirectories {
    BaseDirectories::with_prefix(APP_NAME)
}

/// Return the XDG state directory, creating it if needed.
/// `~/.local/state/vcm/`
pub fn state_dir() -> Result<PathBuf> {
    let dir = base_dirs()
        .get_state_home()
        .context("Failed to get XDG state directory (HOME not set?)")?;
    std::fs::create_dir_all(&dir).context("Failed to create state directory")?;
    Ok(dir)
}

/// Return the XDG config directory (no creation - config may not exist yet).
/// `~/.config/vcm/`
pub fn config_dir() -> Result<PathBuf> {
    base_dirs()
        .get_config_home()
        .context("Could not determine config directory (HOME not set?)")
}

/// Return the XDG data directory, creating it if needed.
/// `~/.local/share/vcm/`
pub fn data_dir() -> Result<PathBuf> {
    let dir = base_dirs()
        .get_data_home()
        .context("Could not determine data directory (HOME not set?)")?;
    std::fs::create_dir_all(&dir).context("Failed to create data directory")?;
    Ok(dir)
}

/// Daemon Unix socket path.
/// `~/.local/state/vcm/daemon.sock`
pub fn socket_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("daemon.sock"))
}
