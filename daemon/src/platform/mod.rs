//! Platform-specific implementations.
//!
//! Cross-platform abstractions over OS-specific behavior. Today only macOS is
//! implemented; trait shapes anticipate future Linux/Windows backends.

use anyhow::Result;

/// Reports the name of the frontmost (focused) application — used to gate
/// keystroke injection by an allowlist.
pub trait FrontmostApp {
    fn name() -> Result<String>;
}

/// Initialises a tracing subscriber that routes through the platform's native
/// logging system (e.g. macOS unified logging / `os_log`).
pub trait PlatformLogging {
    fn init(subsystem: &str, category: &str, filter: tracing_subscriber::EnvFilter) -> Result<()>;
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{MacOsFrontmostApp as Frontmost, MacOsLogging as Logging};
