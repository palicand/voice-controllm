//! Keystroke injection for transcribed text.
//!
//! Injects text as keystrokes into the focused application, with optional
//! filtering based on an application allowlist.

use crate::config::InjectionConfig;
use anyhow::{Context, Result};
use enigo::{Enigo, Keyboard, Settings};
use std::process::Command;
use tracing::{debug, info, warn};

/// Injects transcribed text as keystrokes.
pub struct KeystrokeInjector {
    config: InjectionConfig,
    enigo: Enigo,
}

impl KeystrokeInjector {
    /// Create a new keystroke injector with the given configuration.
    ///
    /// On macOS, this requires Accessibility permissions to be granted.
    pub fn new(config: InjectionConfig) -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| anyhow::anyhow!("Failed to initialize enigo: {}", e))?;

        Ok(Self { config, enigo })
    }

    /// Inject text as keystrokes into the focused application.
    ///
    /// If an allowlist is configured and the focused application is not in it,
    /// the text will not be injected and this method returns Ok(()).
    pub fn inject_text(&mut self, text: &str) -> Result<()> {
        // Check allowlist if configured
        if !self.config.allowlist.is_empty() {
            let frontmost = get_frontmost_app().unwrap_or_else(|e| {
                warn!(error = %e, "Failed to get frontmost app, skipping allowlist check");
                String::new()
            });

            if !frontmost.is_empty() && !self.is_allowed(&frontmost) {
                debug!(
                    app = %frontmost,
                    "Skipping injection: app not in allowlist"
                );
                return Ok(());
            }
        }

        // Inject the text
        info!(text = %text, "Injecting text as keystrokes");
        self.enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("Failed to inject text: {}", e))?;

        Ok(())
    }

    /// Check if an application is in the allowlist.
    fn is_allowed(&self, app_name: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        self.config
            .allowlist
            .iter()
            .any(|allowed| app_lower.contains(&allowed.to_lowercase()))
    }
}

/// Get the name of the frontmost (focused) application on macOS.
#[cfg(target_os = "macos")]
fn get_frontmost_app() -> Result<String> {
    let output = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
        ])
        .output()
        .context("Failed to execute osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript failed: {}", stderr.trim());
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(name)
}

/// Get the name of the frontmost application (stub for non-macOS platforms).
#[cfg(not(target_os = "macos"))]
fn get_frontmost_app() -> Result<String> {
    // On non-macOS platforms, return empty string to skip allowlist check
    Ok(String::new())
}

#[cfg(test)]
#[path = "inject_test.rs"]
mod tests;
