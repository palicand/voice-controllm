//! gRPC client for communicating with daemon.

use std::path::Path;

use anyhow::{Context, Result};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use voice_controllm_proto::voice_controllm_client::VoiceControllmClient;

/// Connect to daemon via Unix socket.
pub async fn connect(socket_path: &Path) -> Result<VoiceControllmClient<Channel>> {
    let socket_path = socket_path.to_path_buf();

    // Create channel with Unix socket connector
    let channel = Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = socket_path.clone();
            async move {
                let stream = UnixStream::connect(path).await?;
                Ok::<_, std::io::Error>(TokioIo::new(stream))
            }
        }))
        .await
        .context("Failed to connect to daemon")?;

    Ok(VoiceControllmClient::new(channel))
}

/// Check if daemon is running by attempting to connect.
pub async fn is_daemon_running(socket_path: &Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    connect(socket_path).await.is_ok()
}
