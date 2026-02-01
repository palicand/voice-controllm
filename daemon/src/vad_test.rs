use super::*;

#[test]
fn test_state_machine_initial_state() {
    let sm = VadStateMachine::new(VadConfig::default());
    assert!(!sm.is_speaking());
}

#[test]
fn test_state_machine_speech_start() {
    let config = VadConfig {
        threshold: 0.5,
        min_speech_chunks: 2,
        min_silence_chunks: 3,
    };
    let mut sm = VadStateMachine::new(config);

    // First speech chunk - not enough yet
    assert_eq!(sm.process(0.8), None);
    assert!(!sm.is_speaking());

    // Second speech chunk - triggers SpeechStart
    assert_eq!(sm.process(0.9), Some(VadEvent::SpeechStart));
    assert!(sm.is_speaking());

    // More speech - no new event
    assert_eq!(sm.process(0.7), None);
    assert!(sm.is_speaking());
}

#[test]
fn test_state_machine_speech_end() {
    let config = VadConfig {
        threshold: 0.5,
        min_speech_chunks: 1,
        min_silence_chunks: 2,
    };
    let mut sm = VadStateMachine::new(config);

    // Start speaking
    assert_eq!(sm.process(0.8), Some(VadEvent::SpeechStart));

    // First silence chunk - not enough
    assert_eq!(sm.process(0.2), None);
    assert!(sm.is_speaking());

    // Second silence chunk - triggers SpeechEnd
    assert_eq!(sm.process(0.1), Some(VadEvent::SpeechEnd));
    assert!(!sm.is_speaking());
}

#[test]
fn test_state_machine_threshold() {
    let config = VadConfig {
        threshold: 0.7,
        min_speech_chunks: 1,
        min_silence_chunks: 1,
    };
    let mut sm = VadStateMachine::new(config);

    // Below threshold - silence
    assert_eq!(sm.process(0.69), None);
    assert!(!sm.is_speaking());

    // At threshold - speech
    assert_eq!(sm.process(0.70), Some(VadEvent::SpeechStart));
    assert!(sm.is_speaking());

    // Just below threshold - silence
    assert_eq!(sm.process(0.69), Some(VadEvent::SpeechEnd));
    assert!(!sm.is_speaking());
}

#[test]
fn test_state_machine_interrupted_speech() {
    let config = VadConfig {
        threshold: 0.5,
        min_speech_chunks: 3,
        min_silence_chunks: 3,
    };
    let mut sm = VadStateMachine::new(config);

    // Two speech chunks
    sm.process(0.8);
    sm.process(0.8);
    assert!(!sm.is_speaking());

    // Silence resets speech count
    sm.process(0.2);

    // Need 3 consecutive again
    sm.process(0.8);
    sm.process(0.8);
    assert!(!sm.is_speaking());

    sm.process(0.8);
    assert!(sm.is_speaking());
}

#[test]
fn test_state_machine_reset() {
    let config = VadConfig {
        threshold: 0.5,
        min_speech_chunks: 1,
        min_silence_chunks: 1,
    };
    let mut sm = VadStateMachine::new(config);

    // Get into speaking state
    sm.process(0.8);
    assert!(sm.is_speaking());

    // Reset
    sm.reset();
    assert!(!sm.is_speaking());
}

#[test]
fn test_default_config() {
    let config = VadConfig::default();
    assert!((config.threshold - 0.5).abs() < f32::EPSILON);
    assert_eq!(config.min_speech_chunks, 2);
    assert_eq!(config.min_silence_chunks, 8);
}
