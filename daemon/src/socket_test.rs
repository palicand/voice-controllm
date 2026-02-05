use super::*;

#[test]
fn test_socket_path_in_xdg_state() {
    let path = socket_path().unwrap();
    assert!(path.to_string_lossy().contains("voice-controllm"));
    assert!(path.to_string_lossy().ends_with("daemon.sock"));
}

#[test]
fn test_pid_path_in_xdg_state() {
    let path = pid_path().unwrap();
    assert!(path.to_string_lossy().contains("voice-controllm"));
    assert!(path.to_string_lossy().ends_with("daemon.pid"));
}

#[tokio::test]
async fn test_create_listener() {
    let temp = tempfile::tempdir().unwrap();
    let sock_path = temp.path().join("test.sock");

    let listener = create_listener(&sock_path).unwrap();
    assert!(sock_path.exists());

    drop(listener);
    cleanup_socket(&sock_path);
    assert!(!sock_path.exists());
}
