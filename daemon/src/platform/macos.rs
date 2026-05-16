//! macOS implementations of platform traits.

use anyhow::{Context, Result};
use std::process::Command;
use tracing_subscriber::{EnvFilter, prelude::*};

use super::{FrontmostApp, PlatformLogging};

pub struct MacOsFrontmostApp;

impl FrontmostApp for MacOsFrontmostApp {
    fn name() -> Result<String> {
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

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

pub struct MacOsLogging;

impl PlatformLogging for MacOsLogging {
    fn init(subsystem: &str, category: &str, filter: EnvFilter) -> Result<()> {
        let oslog = tracing_oslog::OsLogger::new(subsystem, category);

        if std::env::var("VCM_LOG_FILE").is_ok() {
            let log_path = crate::socket::log_path().context("Failed to determine log path")?;
            let log_dir = log_path.parent().context("log path has no parent")?;
            let log_filename = log_path.file_name().context("log path has no filename")?;
            let appender = tracing_appender::rolling::never(log_dir, log_filename);
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            Box::leak(Box::new(guard));

            tracing_subscriber::registry()
                .with(filter)
                .with(oslog)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(non_blocking)
                        .with_ansi(false),
                )
                .try_init()
                .context("Failed to install tracing subscriber")?;
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(oslog)
                .try_init()
                .context("Failed to install tracing subscriber")?;
        }

        Ok(())
    }
}
