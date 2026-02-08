//! Test the full transcription pipeline.
//!
//! Run with: cargo run -p voice-controllm-daemon --example transcribe_test
//!
//! This example runs the engine and prints transcriptions to stdout.
//! Press Ctrl+C to stop.
//!
//! Set RUST_LOG for debug output:
//!   RUST_LOG=info  - model loading and transcription results
//!   RUST_LOG=debug - speech events and timing

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing_subscriber::EnvFilter;
use voice_controllm_daemon::config::Config;
use voice_controllm_daemon::engine::Engine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("Voice-Controllm Transcription Test");
    println!("===================================");
    println!();
    println!("Press Ctrl+C to stop at any time.");
    println!();

    let config = Config::load()?;
    println!("Model: {:?}", config.model.model);
    println!("Language: {:?}", config.model.language);
    println!();

    let mut engine = Engine::new(config)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    println!("Starting engine...");
    println!();

    // Race engine against Ctrl+C - when Ctrl+C fires, the engine future is dropped
    tokio::select! {
        biased;

        _ = signal::ctrl_c() => {
            println!("\nCtrl+C received, stopping...");
            r.store(false, Ordering::SeqCst);
        }

        result = engine.run(running, |text| {
            println!(">>> {}", text);
        }) => {
            if let Err(e) = result {
                eprintln!("Engine error: {:#}", e);
            }
        }
    }

    println!("Done.");
    Ok(())
}
