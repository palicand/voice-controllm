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

use anyhow::Context;
use tracing_subscriber::EnvFilter;

const LOG_ENV_VAR: &str = "VCM_LOG";

pub async fn run() -> anyhow::Result<()> {
    let config = config::Config::load().unwrap_or_default();

    let with_file_sink_dir = if std::env::var("VCM_LOG_FILE").is_ok() {
        let path = socket::log_path()
            .context("VCM_LOG_FILE set but log path unresolvable")?;
        Some(path.parent().expect("log path has parent").to_path_buf())
    } else {
        None
    };

    let filter = EnvFilter::builder()
        .with_env_var(LOG_ENV_VAR)
        .with_default_directive(config.logging.level.as_directive().parse()?)
        .from_env()?;

    let subscriber = vcm_platform::logging::build_subscriber(vcm_platform::logging::InitOptions {
        subsystem: vcm_platform::logging::LOG_SUBSYSTEM,
        category: "daemon",
        filter,
        with_file_sink_dir,
    })?;
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to install global tracing subscriber")?;

    whisper_rs::install_logging_hooks();

    daemon::run().await
}
