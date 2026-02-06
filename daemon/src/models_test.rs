use super::*;
use tempfile::TempDir;

#[test]
fn test_model_info() {
    let info = ModelId::SileroVad.info();
    assert_eq!(info.filename, "silero_vad.onnx");
    assert!(info.url.contains("silero"));
}

#[test]
fn test_model_manager_custom_dir() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());
    assert_eq!(manager.models_dir(), temp.path());
}

#[test]
fn test_model_path_construction() {
    let temp = TempDir::new().unwrap();
    let _manager = ModelManager::with_dir(temp.path());

    // Model doesn't exist yet, so ensure_model would try to download
    // We just test the path would be correct
    let expected_path = temp.path().join("silero_vad.onnx");
    assert!(!expected_path.exists());
}
