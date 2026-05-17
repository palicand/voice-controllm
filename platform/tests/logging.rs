use tracing_subscriber::EnvFilter;
use vcm_platform::logging::{self, InitOptions, LOG_SUBSYSTEM, LogCategory};

#[test]
fn init_with_oslog_does_not_panic() {
    let opts = InitOptions {
        subsystem: LOG_SUBSYSTEM,
        category: LogCategory::Daemon,
        filter: EnvFilter::new("info"),
        with_file_sink_dir: None,
    };
    drop(logging::build_subscriber(opts).expect("build subscriber"));
}
