use std::process::Command;

use anyhow::{Context, Result};

use crate::frontmost::FrontmostApp;

pub struct MacOsFrontmost;

impl FrontmostApp for MacOsFrontmost {
    fn current(&self) -> Result<String> {
        let output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output()
            .context("Failed to execute osascript")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("osascript failed: {}", stderr.trim());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
