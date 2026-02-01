//! Integration tests for VAD that require the model and test audio files.

use voice_controllm_daemon::vad::{
    VAD_CHUNK_SIZES, VAD_SAMPLE_RATE, VadConfig, VadEvent, VoiceActivityDetector,
};

/// Get the VAD model path, checking VAD_MODEL_PATH env var first, then project default.
fn get_model_path() -> String {
    std::env::var("VAD_MODEL_PATH").unwrap_or_else(|_| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("models/silero_vad.onnx")
            .to_string_lossy()
            .to_string()
    })
}

/// Get path to test audio file.
fn get_test_audio_path(filename: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(filename)
}

/// Load WAV file as f32 samples at 16kHz mono.
fn load_wav_samples(path: &std::path::Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("Failed to open WAV file");
    let spec = reader.spec();

    assert_eq!(spec.sample_rate, VAD_SAMPLE_RATE, "WAV must be 16kHz");

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / max_val)
                .collect()
        }
    };

    // Convert to mono if stereo
    if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / spec.channels as f32)
            .collect()
    } else {
        samples
    }
}

/// Process all audio through VAD and collect events.
fn process_audio_collect_events(vad: &mut VoiceActivityDetector, samples: &[f32]) -> Vec<VadEvent> {
    let chunk_size = vad.chunk_size();
    let mut events = Vec::new();

    for chunk in samples.chunks(chunk_size) {
        if chunk.len() < chunk_size {
            break;
        }
        if let Some(event) = vad.process(chunk).expect("VAD processing failed") {
            events.push(event);
        }
    }

    events
}

#[test]
fn test_vad_with_silence() {
    let model_path = get_model_path();

    let mut vad =
        VoiceActivityDetector::new(&model_path, VadConfig::default()).expect("Failed to load VAD");

    // Pure silence
    let silence = vec![0.0f32; 512];
    let prob = vad.process_chunk(&silence).expect("VAD failed");

    // Should be very low probability
    assert!(prob < 0.3, "Silence detected as speech: {}", prob);
}

#[test]
fn test_vad_with_noise() {
    let model_path = get_model_path();

    let mut vad =
        VoiceActivityDetector::new(&model_path, VadConfig::default()).expect("Failed to load VAD");

    // Generate white noise (not speech)
    let noise: Vec<f32> = (0..512).map(|i| ((i * 7) % 100) as f32 / 1000.0).collect();
    let prob = vad.process_chunk(&noise).expect("VAD failed");

    // Random noise should not be detected as speech
    assert!(prob < 0.5, "Noise detected as speech: {}", prob);
}

#[test]
fn test_vad_chunk_size_validation() {
    let model_path = get_model_path();

    // Valid chunk sizes
    for &size in &VAD_CHUNK_SIZES {
        let result =
            VoiceActivityDetector::with_chunk_size(&model_path, VadConfig::default(), size);
        assert!(result.is_ok(), "Failed with valid chunk size {}", size);
    }

    // Invalid chunk size
    let result = VoiceActivityDetector::with_chunk_size(&model_path, VadConfig::default(), 100);
    assert!(result.is_err(), "Should reject invalid chunk size");
}

#[test]
fn test_vad_state_persistence() {
    let model_path = get_model_path();

    let mut vad =
        VoiceActivityDetector::new(&model_path, VadConfig::default()).expect("Failed to load VAD");

    // Process multiple chunks - state should be maintained
    let silence = vec![0.0f32; 512];
    let mut probs = Vec::new();

    for _ in 0..5 {
        let prob = vad.process_chunk(&silence).expect("VAD failed");
        probs.push(prob);
    }

    // All should be low for silence
    for (i, prob) in probs.iter().enumerate() {
        assert!(*prob < 0.3, "Chunk {} detected as speech: {}", i, prob);
    }
}

#[test]
fn test_e2e_speech_detection() {
    let model_path = get_model_path();
    let test_audio_path = get_test_audio_path("speech.wav");

    assert!(
        test_audio_path.exists(),
        "Test audio file not found: {}",
        test_audio_path.display()
    );

    let samples = load_wav_samples(&test_audio_path);
    assert!(!samples.is_empty(), "No samples loaded from speech.wav");

    let mut vad =
        VoiceActivityDetector::new(&model_path, VadConfig::default()).expect("Failed to load VAD");

    let events = process_audio_collect_events(&mut vad, &samples);

    // Speech file should trigger at least one SpeechStart
    let speech_starts = events
        .iter()
        .filter(|e| matches!(e, VadEvent::SpeechStart))
        .count();
    let speech_ends = events
        .iter()
        .filter(|e| matches!(e, VadEvent::SpeechEnd))
        .count();

    assert!(
        speech_starts >= 1,
        "Expected at least 1 SpeechStart event, got {}. Events: {:?}",
        speech_starts,
        events
    );

    println!(
        "Speech detection: {} starts, {} ends from {:.1}s of audio",
        speech_starts,
        speech_ends,
        samples.len() as f32 / VAD_SAMPLE_RATE as f32
    );
}

#[test]
fn test_e2e_silence_no_speech() {
    let model_path = get_model_path();
    let test_audio_path = get_test_audio_path("silence.wav");

    assert!(
        test_audio_path.exists(),
        "Test audio file not found: {}",
        test_audio_path.display()
    );

    let samples = load_wav_samples(&test_audio_path);
    assert!(!samples.is_empty(), "No samples loaded from silence.wav");

    let mut vad =
        VoiceActivityDetector::new(&model_path, VadConfig::default()).expect("Failed to load VAD");

    let events = process_audio_collect_events(&mut vad, &samples);

    // Silence file should NOT trigger any speech events
    assert!(
        events.is_empty(),
        "Expected no VAD events for silence, got: {:?}",
        events
    );

    println!(
        "Silence test passed: no events from {:.1}s of silence",
        samples.len() as f32 / VAD_SAMPLE_RATE as f32
    );
}
