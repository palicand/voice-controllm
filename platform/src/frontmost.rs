use anyhow::Result;

/// Query the name of the currently-focused application.
pub trait FrontmostApp {
    fn current(&self) -> Result<String>;
}

/// Convenience free function dispatching to the platform's default impl.
#[cfg(target_os = "macos")]
pub fn current() -> Result<String> {
    crate::macos::frontmost::MacOsFrontmost.current()
}

#[cfg(not(target_os = "macos"))]
pub fn current() -> Result<String> {
    // No platform impl on non-macOS yet. Empty string short-circuits any allowlist check.
    Ok(String::new())
}
