//! End-to-end integration test for daemon gRPC lifecycle.
//!
//! Starts the daemon in-process with a temporary socket/PID directory,
//! then exercises the full control flow through the gRPC client.
//!
//! Note: start_listening/stop_listening require initialized engine (real models),
//! so we test the control plane without audio here.

use std::time::Duration;

use voice_controllm_daemon::daemon::{DaemonPaths, run_with_paths};
use voice_controllm_proto::voice_controllm_client::VoiceControllmClient;
use voice_controllm_proto::{Empty, State, status::Status as StatusVariant};

/// Connect to the daemon, retrying until the socket is ready.
async fn connect_with_retry(
    socket_path: &std::path::Path,
    timeout: Duration,
) -> VoiceControllmClient<tonic::transport::Channel> {
    let start = std::time::Instant::now();
    loop {
        if start.elapsed() > timeout {
            panic!("Timed out waiting for daemon at {}", socket_path.display());
        }
        let path = socket_path.to_path_buf();
        let result = tonic::transport::Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector(tower::service_fn(move |_: tonic::transport::Uri| {
                let p = path.clone();
                async move {
                    let stream = tokio::net::UnixStream::connect(p).await?;
                    Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(stream))
                }
            }))
            .await;
        match result {
            Ok(channel) => return VoiceControllmClient::new(channel),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

fn extract_state(status: voice_controllm_proto::Status) -> State {
    match status.status {
        Some(StatusVariant::Healthy(h)) => State::try_from(h.state).unwrap(),
        other => panic!("Expected Healthy status, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_daemon_grpc_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let sock_path = tmp.path().join("daemon.sock");
    let pid_path = tmp.path().join("daemon.pid");

    let paths = DaemonPaths {
        socket: sock_path.clone(),
        pid: pid_path.clone(),
    };

    // Spawn daemon as background task
    let daemon_handle = tokio::spawn(async move { run_with_paths(paths).await });

    // Connect (with retry for startup race)
    let mut client = connect_with_retry(&sock_path, Duration::from_secs(5)).await;

    // Initial status: Paused (mark_ready called during startup)
    let status = client.get_status(Empty {}).await.unwrap().into_inner();
    assert_eq!(extract_state(status), State::Paused);

    // Start listening should fail — engine not initialized (no real models)
    let result = client.start_listening(Empty {}).await;
    assert!(result.is_err(), "start_listening should fail without initialized engine");

    // Status should still be Paused
    let status = client.get_status(Empty {}).await.unwrap().into_inner();
    assert_eq!(extract_state(status), State::Paused);

    // Stop listening from Paused should be a no-op
    client.stop_listening(Empty {}).await.unwrap();
    let status = client.get_status(Empty {}).await.unwrap().into_inner();
    assert_eq!(extract_state(status), State::Paused);

    // Shutdown
    client.shutdown(Empty {}).await.unwrap();

    // Wait for daemon to exit
    let result = tokio::time::timeout(Duration::from_secs(5), daemon_handle)
        .await
        .expect("Daemon did not shut down in time")
        .expect("Daemon task panicked");
    result.expect("Daemon returned error");

    // Verify cleanup
    assert!(!sock_path.exists(), "Socket should be cleaned up");
    assert!(!pid_path.exists(), "PID file should be cleaned up");
}
