use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::Subscriber;
use tracing_oslog::OsLogger;
use tracing_subscriber::{Registry, fmt, layer::SubscriberExt};

use crate::logging::{InitOptions, LogCategory, LoggingSink};

pub struct MacOsLogging;

impl LoggingSink for MacOsLogging {
    fn layered(
        self,
        registry: Registry,
        opts: InitOptions<'_>,
    ) -> Result<Box<dyn Subscriber + Send + Sync>> {
        let oslog = OsLogger::new(opts.subsystem, opts.category.as_str());
        let env_filter = opts.filter;

        if let Some(dir) = opts.with_file_sink_dir {
            let file_writer = file_writer(dir, opts.category)?;
            let file_layer = fmt::layer().with_writer(file_writer).with_ansi(false);
            Ok(Box::new(
                registry.with(env_filter).with(oslog).with(file_layer),
            ))
        } else {
            Ok(Box::new(registry.with(env_filter).with(oslog)))
        }
    }
}

fn file_writer(
    dir: PathBuf,
    category: LogCategory,
) -> Result<tracing_appender::rolling::RollingFileAppender> {
    std::fs::create_dir_all(&dir).context("Failed to create log dir")?;
    Ok(tracing_appender::rolling::never(
        dir,
        format!("{}.log", category.as_str()),
    ))
}
