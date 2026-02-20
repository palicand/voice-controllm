//! Quick test of audio capture and resampling.
//! Run with: cargo run -p vcm-daemon --example capture_test
//!
//! Saves two WAV files:
//! - /tmp/capture_raw.wav - original sample rate from device
//! - /tmp/capture_16k.wav - resampled to 16kHz for speech recognition

use std::time::Duration;
use vcm_daemon::audio::{AudioCapture, AudioResampler, TARGET_SAMPLE_RATE};

fn main() -> anyhow::Result<()> {
    println!("Starting audio capture test...");
    println!("Speak into your microphone for 3 seconds.\n");

    let capture = AudioCapture::start()?;
    let sample_rate = capture.sample_rate();
    let channels = capture.channels();
    println!("Device: {}Hz, {} channel(s)", sample_rate, channels);

    let mut resampler = AudioResampler::new(sample_rate, TARGET_SAMPLE_RATE, 1024)?;

    let mut raw_samples: Vec<f32> = Vec::new();
    let mut resampled_samples: Vec<f32> = Vec::new();
    let mut peak_amplitude: f32 = 0.0;

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if let Some(samples) = capture.try_recv() {
            let chunk_peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            peak_amplitude = peak_amplitude.max(chunk_peak);

            // Store raw samples
            raw_samples.extend(&samples);

            // Resample to 16kHz
            let chunk_size = resampler.chunk_size();
            if samples.len() >= chunk_size {
                let to_resample = &samples[..samples.len() - (samples.len() % chunk_size)];
                if let Ok(resampled) = resampler.process(to_resample) {
                    resampled_samples.extend(resampled);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    capture.stop();

    // Save raw audio
    let raw_path = "/tmp/capture_raw.wav";
    save_wav(raw_path, &raw_samples, sample_rate)?;
    println!("\nSaved: {} ({} Hz)", raw_path, sample_rate);

    // Save resampled audio
    let resampled_path = "/tmp/capture_16k.wav";
    save_wav(resampled_path, &resampled_samples, TARGET_SAMPLE_RATE)?;
    println!("Saved: {} ({} Hz)", resampled_path, TARGET_SAMPLE_RATE);

    println!("\nResults:");
    println!("  Raw samples: {}", raw_samples.len());
    println!("  Resampled samples: {}", resampled_samples.len());
    println!("  Peak amplitude: {:.3}", peak_amplitude);

    if peak_amplitude < 0.01 {
        println!("\n⚠ Very low audio level - check microphone permissions or input device");
    } else {
        println!("\n✓ Audio capture working!");
    }

    println!("\nPlay with: afplay {}", resampled_path);

    Ok(())
}

fn save_wav(path: &str, samples: &[f32], sample_rate: u32) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for &sample in samples {
        // Convert f32 [-1.0, 1.0] to i16
        let amplitude = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(amplitude)?;
    }

    writer.finalize()?;
    Ok(())
}
