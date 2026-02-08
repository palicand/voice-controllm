//! Test keystroke injection with the transcription pipeline.
//!
//! Run with: cargo run -p voice-controllm-daemon --example inject_test
//!
//! This example runs the engine and injects transcriptions as keystrokes.
//! Press Ctrl+C to stop.
//!
//! IMPORTANT: On macOS, you must grant Accessibility permissions to your terminal
//! (e.g., Terminal.app or iTerm2) in System Preferences > Security & Privacy >
//! Privacy > Accessibility for keystroke injection to work.
//!
//! Set RUST_LOG for debug output:
//!   RUST_LOG=info  - model loading, transcription results, and injection
//!   RUST_LOG=debug - speech events and timing

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use tracing::error;
use tracing_subscriber::EnvFilter;
use voice_controllm_daemon::config::Config;
use voice_controllm_daemon::engine::Engine;
use voice_controllm_daemon::inject::KeystrokeInjector;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    println!("Voice-Controllm Injection Test");
    println!("===============================");
    println!();
    println!("Press Ctrl+C to stop at any time.");
    println!();
    println!("IMPORTANT: Ensure Accessibility permissions are granted.");
    println!();

    let config = Config::load()?;
    println!("Model: {:?}", config.model.model);
    println!("Language: {:?}", config.model.language);
    if config.injection.allowlist.is_empty() {
        println!("Allowlist: (disabled - injecting to all apps)");
    } else {
        println!("Allowlist: {:?}", config.injection.allowlist);
    }
    println!();

    // Create the keystroke injector
    let mut injector = KeystrokeInjector::new(config.injection.clone())?;

    let mut engine = Engine::new(config)?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    println!("Starting engine...");
    println!();
    println!("Speak into your microphone. Transcribed text will be injected as keystrokes.");
    println!();

    // Race engine against Ctrl+C - when Ctrl+C fires, the engine future is dropped
    tokio::select! {
        biased;

        _ = signal::ctrl_c() => {
            println!("\nCtrl+C received, stopping...");
            r.store(false, Ordering::SeqCst);
        }

        result = engine.run(running, |text| {
            // Print the transcription for visibility
            println!(">>> {}", text);

            // Inject the text as keystrokes
            if let Err(e) = injector.inject_text(text) {
                error!(error = %e, "Keystroke injection failed");
            }
        }) => {
            if let Err(e) = result {
                eprintln!("Engine error: {:#}", e);
            }
        }
    }

    println!("Done.");
    Ok(())
}
