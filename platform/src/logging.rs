use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing_subscriber::{EnvFilter, Registry};

pub const LOG_SUBSYSTEM: &str = "com.github.palicand.vcm";
pub const VCM_LOG_ENV: &str = "VCM_LOG";
pub const VCM_LOG_FILE_ENV: &str = "VCM_LOG_FILE";

pub struct InitOptions<'a> {
    pub subsystem: &'a str,
    pub category: &'a str,
    pub filter: EnvFilter,
    /// If Some, also write logs to a rolling file in this directory.
    pub with_file_sink_dir: Option<PathBuf>,
}

pub trait LoggingSink {
    fn layered(
        self,
        registry: Registry,
        opts: InitOptions<'_>,
    ) -> Result<Box<dyn tracing::Subscriber + Send + Sync>>;
}

/// Install the platform's tracing subscriber as the global default.
///
/// `default_directive` is the EnvFilter default (e.g. `"info"`); it is overridden by the
/// `VCM_LOG` env var. When `VCM_LOG_FILE` is set, a rolling file sink is added at
/// `state_dir/<category>.log` in addition to the platform sink.
pub fn init(category: &str, default_directive: &str, state_dir: PathBuf) -> Result<()> {
    let filter = EnvFilter::builder()
        .with_env_var(VCM_LOG_ENV)
        .with_default_directive(default_directive.parse().context("parse log directive")?)
        .from_env_lossy();

    let with_file_sink_dir = std::env::var_os(VCM_LOG_FILE_ENV).map(|_| state_dir);

    let subscriber = build_subscriber(InitOptions {
        subsystem: LOG_SUBSYSTEM,
        category,
        filter,
        with_file_sink_dir,
    })?;
    tracing::subscriber::set_global_default(subscriber).context("install global tracing subscriber")
}

#[cfg(target_os = "macos")]
pub fn build_subscriber(
    opts: InitOptions<'_>,
) -> Result<Box<dyn tracing::Subscriber + Send + Sync>> {
    crate::macos::logging::MacOsLogging.layered(Registry::default(), opts)
}

#[cfg(not(target_os = "macos"))]
pub fn build_subscriber(
    opts: InitOptions<'_>,
) -> Result<Box<dyn tracing::Subscriber + Send + Sync>> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = Registry::default().with(opts.filter).with(fmt::layer());
    Ok(Box::new(subscriber))
}
