//! End-to-end integration test for daemon gRPC lifecycle.
//!
//! Starts the daemon in-process with a temporary socket/PID directory,
//! then exercises the full control flow through the gRPC client.
//!
//! Note: start_listening/stop_listening require initialized engine (real models),
//! so we test the control plane without audio here.

use std::time::Duration;

use vcm_daemon::config::Config;
use vcm_daemon::daemon::{DaemonPaths, run_with_paths_and_config};
use vcm_proto::vcm_client::VcmClient;
use vcm_proto::{Empty, State, status::Status as StatusVariant};

/// Connect to the daemon, retrying until the socket is ready.
async fn connect_with_retry(
    socket_path: &std::path::Path,
    timeout: Duration,
) -> VcmClient<tonic::transport::Channel> {
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
            Ok(channel) => return VcmClient::new(channel),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

fn extract_state(status: vcm_proto::Status) -> State {
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

    // Use default config — init will fail fast because models aren't in the
    // default XDG dir (test runs with a clean model manager).
    let config = Config::default();

    // Spawn daemon as background task
    let daemon_handle = tokio::spawn(async move { run_with_paths_and_config(paths, config).await });

    // Connect (with retry for startup race)
    let mut client = connect_with_retry(&sock_path, Duration::from_secs(5)).await;

    // Wait for init attempt to complete (should fail quickly with no models)
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Get current state — Initializing (init failed, engine returned but not marked ready)
    let status = client.get_status(Empty {}).await.unwrap().into_inner();
    let state = extract_state(status);
    assert!(
        state == State::Paused || state == State::Initializing,
        "Expected Paused or Initializing, got {:?}",
        state
    );

    // Shutdown should always work regardless of state
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
