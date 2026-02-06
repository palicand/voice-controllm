//! Daemon runner that orchestrates all components.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tonic::transport::Server;
use tracing::info;

use crate::controller::Controller;
use crate::server::VoiceControllmService;
use crate::socket::{cleanup_socket, create_listener};

/// Paths used by the daemon at runtime.
pub struct DaemonPaths {
    pub socket: PathBuf,
    pub pid: PathBuf,
}

impl DaemonPaths {
    /// Create paths using XDG state directory defaults.
    pub fn from_xdg() -> Result<Self> {
        Ok(Self {
            socket: crate::socket::socket_path()?,
            pid: crate::socket::pid_path()?,
        })
    }
}

/// Run the daemon with default XDG paths.
pub async fn run() -> Result<()> {
    run_with_paths(DaemonPaths::from_xdg()?).await
}

/// Run the daemon with custom paths.
pub async fn run_with_paths(paths: DaemonPaths) -> Result<()> {
    let sock_path = paths.socket;
    let pid_file = paths.pid;

    // Write PID file
    let pid = std::process::id();
    std::fs::write(&pid_file, pid.to_string()).context("Failed to write PID file")?;
    info!(pid = pid, path = %pid_file.display(), "Wrote PID file");

    // Create Unix socket listener
    let listener = create_listener(&sock_path)?;
    info!(path = %sock_path.display(), "Listening on Unix socket");

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Create controller with event channel
    let (event_tx, _) = broadcast::channel(256);
    let controller = Arc::new(Controller::new(event_tx, shutdown_tx));

    // Create gRPC service
    let service = VoiceControllmService::new(controller.clone());

    // Convert UnixListener to stream
    let incoming = async_stream::stream! {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => yield Ok::<_, std::io::Error>(stream),
                Err(e) => {
                    tracing::error!(error = %e, "Accept error");
                }
            }
        }
    };

    // Run server with graceful shutdown
    info!("Daemon started");
    let server = Server::builder()
        .add_service(service.into_server())
        .serve_with_incoming_shutdown(incoming, async {
            let _ = shutdown_rx.await;
            info!("Shutdown signal received");
        });

    let result = server.await;

    // Cleanup
    cleanup_socket(&sock_path);
    let _ = std::fs::remove_file(&pid_file);
    info!("Daemon stopped");

    result.context("Server error")
}
