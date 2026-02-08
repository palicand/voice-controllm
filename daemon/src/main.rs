use anyhow::Context;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use voice_controllm_daemon::socket::log_path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_path = log_path().context("Failed to determine log path")?;
    let log_dir = log_path.parent().expect("log path has parent");
    let log_filename = log_path.file_name().expect("log path has filename");

    let file_appender = tracing_appender::rolling::never(log_dir, log_filename);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(EnvFilter::from_default_env().add_directive("voice_controllm_daemon=info".parse()?))
        .init();

    voice_controllm_daemon::daemon::run().await
}
