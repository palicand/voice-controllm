use std::path::PathBuf;

use anyhow::Result;
use tracing_subscriber::{EnvFilter, Registry};

pub const LOG_SUBSYSTEM: &str = "com.github.palicand.vcm";

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
