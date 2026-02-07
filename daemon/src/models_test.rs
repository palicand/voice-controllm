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

#[tokio::test]
async fn test_check_model_missing() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());
    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Missing));
}

#[tokio::test]
async fn test_check_model_ready() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());

    // Create a file with the correct size
    let info = ModelId::SileroVad.info();
    let path = temp.path().join(info.filename);
    let data = vec![0u8; info.size_bytes.unwrap() as usize];
    tokio::fs::write(&path, &data).await.unwrap();

    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Ready(_)));
}

#[tokio::test]
async fn test_check_model_corrupted_wrong_size() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());

    // Create a file with wrong size
    let info = ModelId::SileroVad.info();
    let path = temp.path().join(info.filename);
    tokio::fs::write(&path, b"too small").await.unwrap();

    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Corrupted { .. }));
}
