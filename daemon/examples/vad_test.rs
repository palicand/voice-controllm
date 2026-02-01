//! Test VAD with live microphone input.
//! Run with: cargo run -p voice-controllm-daemon --example vad_test
//!
//! Requires the Silero VAD model. Download from:
//! https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx
//!
//! Set VAD_MODEL_PATH environment variable or place at models/silero_vad.onnx
//!
//! Set RUST_LOG for tracing output:
//!   RUST_LOG=trace  - all logs including per-chunk probabilities
//!   RUST_LOG=debug  - model loading and speech events
//!
//! Audio is saved to /tmp/vad_test.wav on exit.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_subscriber::EnvFilter;
use voice_controllm_daemon::audio::{AudioCapture, AudioResampler, TARGET_SAMPLE_RATE};
use voice_controllm_daemon::vad::{VAD_SAMPLE_RATE, VadConfig, VadEvent, VoiceActivityDetector};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let model_path =
        std::env::var("VAD_MODEL_PATH").unwrap_or_else(|_| "models/silero_vad.onnx".to_string());

    println!("Loading VAD model from: {}", model_path);

    let mut vad = VoiceActivityDetector::new(&model_path, VadConfig::default())?;
    let chunk_size = vad.chunk_size();

    println!("VAD loaded. Chunk size: {} samples", chunk_size);
    println!("Starting audio capture... Press Ctrl+C to stop.\n");

    let capture = AudioCapture::start()?;
    let sample_rate = capture.sample_rate();
    println!(
        "Device: {}Hz → resampling to {}Hz",
        sample_rate, VAD_SAMPLE_RATE
    );

    let mut resampler = AudioResampler::new(sample_rate, TARGET_SAMPLE_RATE, 1024)?;

    // Handle Ctrl+C
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    let mut input_buffer: Vec<f32> = Vec::new(); // Buffer for resampler input
    let mut vad_buffer: Vec<f32> = Vec::new(); // Buffer for VAD input (resampled)
    let mut all_audio: Vec<f32> = Vec::new();
    let resampler_chunk = resampler.chunk_size();

    while running.load(Ordering::SeqCst) {
        if let Some(samples) = capture.try_recv() {
            input_buffer.extend(samples);

            // Process complete resampler chunks
            while input_buffer.len() >= resampler_chunk {
                let chunk: Vec<f32> = input_buffer.drain(..resampler_chunk).collect();
                if let Ok(resampled) = resampler.process(&chunk) {
                    vad_buffer.extend(resampled);
                }
            }

            // Process complete VAD chunks
            while vad_buffer.len() >= chunk_size {
                let chunk: Vec<f32> = vad_buffer.drain(..chunk_size).collect();
                all_audio.extend(&chunk);

                match vad.process(&chunk)? {
                    Some(VadEvent::SpeechStart) => {
                        println!("🎤 Speech started");
                    }
                    Some(VadEvent::SpeechEnd) => {
                        println!("🔇 Speech ended");
                    }
                    None => {}
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    capture.stop();
    println!("\nStopped.");

    // Save captured audio
    let wav_path = "/tmp/vad_test.wav";
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: VAD_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(wav_path, spec)?;
    for sample in &all_audio {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    println!(
        "Saved {} samples ({:.1}s) to {}",
        all_audio.len(),
        all_audio.len() as f32 / VAD_SAMPLE_RATE as f32,
        wav_path
    );

    Ok(())
}
