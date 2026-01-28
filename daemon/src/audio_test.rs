use super::*;

#[test]
fn test_audio_buffer_creation() {
    let samples = vec![0.1, 0.2, 0.3, 0.4];
    let buffer = AudioBuffer::new(samples.clone(), 16000);

    assert_eq!(buffer.samples, samples);
    assert_eq!(buffer.sample_rate, 16000);
}

#[test]
fn test_audio_buffer_empty() {
    let buffer = AudioBuffer::empty(44100);

    assert!(buffer.samples.is_empty());
    assert_eq!(buffer.sample_rate, 44100);
}

#[test]
fn test_audio_buffer_duration() {
    // 16000 samples at 16kHz = 1 second
    let samples = vec![0.0; 16000];
    let buffer = AudioBuffer::new(samples, 16000);

    assert!((buffer.duration_secs() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_audio_buffer_duration_half_second() {
    // 8000 samples at 16kHz = 0.5 seconds
    let samples = vec![0.0; 8000];
    let buffer = AudioBuffer::new(samples, 16000);

    assert!((buffer.duration_secs() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_audio_buffer_append() {
    let mut buffer1 = AudioBuffer::new(vec![0.1, 0.2], 16000);
    let buffer2 = AudioBuffer::new(vec![0.3, 0.4], 16000);

    buffer1.append(&buffer2);

    assert_eq!(buffer1.samples, vec![0.1, 0.2, 0.3, 0.4]);
}

#[test]
#[should_panic(expected = "different sample rates")]
fn test_audio_buffer_append_mismatched_rates() {
    let mut buffer1 = AudioBuffer::new(vec![0.1], 16000);
    let buffer2 = AudioBuffer::new(vec![0.2], 44100);

    buffer1.append(&buffer2);
}

#[test]
fn test_audio_buffer_clear() {
    let mut buffer = AudioBuffer::new(vec![0.1, 0.2, 0.3], 16000);
    buffer.clear();

    assert!(buffer.samples.is_empty());
    assert_eq!(buffer.sample_rate, 16000);
}

#[test]
fn test_stereo_to_mono() {
    // Stereo: L=0.2, R=0.4 -> Mono: 0.3
    let stereo = vec![0.2, 0.4, 0.6, 0.8];
    let mono = stereo_to_mono(&stereo);

    assert_eq!(mono.len(), 2);
    assert!((mono[0] - 0.3).abs() < f32::EPSILON);
    assert!((mono[1] - 0.7).abs() < f32::EPSILON);
}

#[test]
fn test_stereo_to_mono_empty() {
    let mono = stereo_to_mono(&[]);
    assert!(mono.is_empty());
}

#[test]
fn test_to_mono_passthrough() {
    let samples = vec![0.1, 0.2, 0.3];
    let mono = to_mono(&samples, 1);

    assert_eq!(mono, samples);
}

#[test]
fn test_to_mono_stereo() {
    let stereo = vec![0.2, 0.4, 0.6, 0.8];
    let mono = to_mono(&stereo, 2);

    assert_eq!(mono.len(), 2);
    assert!((mono[0] - 0.3).abs() < f32::EPSILON);
    assert!((mono[1] - 0.7).abs() < f32::EPSILON);
}

#[test]
fn test_to_mono_quad() {
    // 4 channels: average of 0.1, 0.2, 0.3, 0.4 = 0.25
    let quad = vec![0.1, 0.2, 0.3, 0.4];
    let mono = to_mono(&quad, 4);

    assert_eq!(mono.len(), 1);
    assert!((mono[0] - 0.25).abs() < f32::EPSILON);
}

#[test]
fn test_resampler_creation() {
    let resampler = AudioResampler::new(48000, 16000, 1024);
    assert!(resampler.is_ok());
}

#[test]
fn test_resampler_chunk_sizes() {
    let resampler = AudioResampler::new(48000, 16000, 1024).unwrap();

    assert_eq!(resampler.chunk_size(), 1024);
    // Output chunk size is determined by rubato internally
    // 1024 * (16000/48000) ≈ 341-342 depending on rounding
    let output_size = resampler.output_chunk_size();
    assert!((341..=342).contains(&output_size));
}

#[test]
fn test_resampler_downsample() {
    let mut resampler = AudioResampler::new(48000, 16000, 480).unwrap();

    // Generate 480 samples of a 1kHz sine wave at 48kHz
    let input: Vec<f32> = (0..480)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    let output = resampler.process(&input).unwrap();

    // Output should be roughly 1/3 the size (480 * 16000/48000 = 160)
    assert_eq!(output.len(), 160);

    // Output should still be a valid waveform (not all zeros, reasonable amplitude)
    let max_amplitude = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        max_amplitude > 0.5,
        "Output amplitude too low: {}",
        max_amplitude
    );
}

#[test]
fn test_resampler_upsample() {
    let mut resampler = AudioResampler::new(16000, 48000, 160).unwrap();

    // Generate 160 samples of a 1kHz sine wave at 16kHz
    let input: Vec<f32> = (0..160)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 16000.0).sin())
        .collect();

    let output = resampler.process(&input).unwrap();

    // Output should be 3x the size (160 * 48000/16000 = 480)
    assert_eq!(output.len(), 480);
}

#[test]
fn test_resampler_empty_input() {
    let mut resampler = AudioResampler::new(48000, 16000, 480).unwrap();
    let output = resampler.process(&[]).unwrap();

    assert!(output.is_empty());
}

#[test]
fn test_resampler_multiple_chunks() {
    let mut resampler = AudioResampler::new(48000, 16000, 480).unwrap();

    // 2 chunks of 480 samples = 960 samples
    let input: Vec<f32> = (0..960)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    let output = resampler.process(&input).unwrap();

    // Should produce 2 output chunks: 960 * (16000/48000) = 320
    assert_eq!(output.len(), 320);
}

// Hardware tests - require actual microphone
#[test]
#[ignore]
fn test_audio_capture_start_stop() {
    let capture = AudioCapture::start();
    assert!(
        capture.is_ok(),
        "Failed to start capture: {:?}",
        capture.err()
    );

    let capture = capture.unwrap();
    assert!(capture.sample_rate() > 0);
    assert!(capture.channels() > 0);

    capture.stop();
}

#[test]
#[ignore]
fn test_audio_capture_receives_samples() {
    let capture = AudioCapture::start().expect("Failed to start capture");

    // Wait a bit for samples to accumulate
    std::thread::sleep(std::time::Duration::from_millis(100));

    let samples = capture.try_recv();
    assert!(samples.is_some(), "No samples received");
    assert!(!samples.unwrap().is_empty(), "Received empty samples");

    capture.stop();
}
