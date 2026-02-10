use anyhow::Context;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use voice_controllm_daemon::config::Config;
use voice_controllm_daemon::socket::log_path;

/// Application-specific environment variable for log filtering (overrides config).
const LOG_ENV_VAR: &str = "VCM_LOG";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load().unwrap_or_default();

    let log_path = log_path().context("Failed to determine log path")?;
    let log_dir = log_path.parent().expect("log path has parent");
    let log_filename = log_path.file_name().expect("log path has filename");

    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // VCM_LOG env var overrides config file level
    let filter = EnvFilter::builder()
        .with_env_var(LOG_ENV_VAR)
        .with_default_directive(config.logging.level.as_directive().parse()?)
        .from_env()?;

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(filter)
        .init();

    // Route whisper.cpp and GGML logs through tracing
    whisper_rs::install_logging_hooks();

    voice_controllm_daemon::daemon::run().await
}
