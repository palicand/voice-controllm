//! Daemon runner that orchestrates all components.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tonic::transport::Server;
use tracing::info;

use crate::controller::Controller;
use crate::server::VoiceControllmService;
use crate::socket::{cleanup_socket, create_listener, pid_path, socket_path};

/// Run the daemon.
pub async fn run() -> Result<()> {
    // Get paths
    let sock_path = socket_path()?;
    let pid_file = pid_path()?;

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
