pub mod audio;
pub mod config;
pub mod controller;
pub mod daemon;
pub mod dirs;
pub mod engine;
pub mod inject;
pub mod models;
pub mod platform;
pub mod server;
pub mod socket;
pub mod transcribe;
pub mod vad;

use tracing_subscriber::EnvFilter;

use crate::platform::PlatformLogging;

/// Logging subsystem identifier — matches the macOS bundle identifier so
/// `log show --predicate 'subsystem == "com.palicka.vcm"'` works after install.
pub const LOG_SUBSYSTEM: &str = "com.palicka.vcm";

/// Application-specific environment variable for log filtering (overrides config).
const LOG_ENV_VAR: &str = "VCM_LOG";

/// Entry point for the daemon process: configures logging and launches the daemon.
pub async fn run() -> anyhow::Result<()> {
    let config = config::Config::load().unwrap_or_default();

    let filter = EnvFilter::builder()
        .with_env_var(LOG_ENV_VAR)
        .with_default_directive(config.logging.level.as_directive().parse()?)
        .from_env()?;

    platform::Logging::init(LOG_SUBSYSTEM, "daemon", filter)?;

    whisper_rs::install_logging_hooks();

    daemon::run().await
}
