pub mod audio;
pub mod config;
pub mod controller;
pub mod daemon;
pub mod dirs;
pub mod engine;
pub mod inject;
pub mod models;
pub mod server;
pub mod socket;
pub mod transcribe;
pub mod vad;

pub async fn run() -> anyhow::Result<()> {
    let config = config::Config::load().unwrap_or_default();
    let state_dir = vcm_common::dirs::state_dir()?;

    vcm_platform::logging::init(
        vcm_platform::logging::LogCategory::Daemon,
        config.logging.level.as_directive(),
        state_dir,
    )?;

    whisper_rs::install_logging_hooks();

    daemon::run().await
}
