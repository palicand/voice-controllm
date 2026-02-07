use super::*;

#[test]
fn test_engine_not_initialized_by_default() {
    let config = Config::default();
    let engine = Engine::new(config).unwrap();
    assert!(!engine.is_initialized());
}

#[test]
fn test_speech_model_to_model_id() {
    assert_eq!(
        speech_model_to_model_id(SpeechModel::WhisperBase),
        ModelId::WhisperBase
    );
    assert_eq!(
        speech_model_to_model_id(SpeechModel::WhisperLargeV3Turbo),
        ModelId::WhisperLargeV3Turbo
    );
}
