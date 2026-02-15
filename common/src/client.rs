//! gRPC client for communicating with the voice-controllm daemon.

use std::path::Path;

use anyhow::{Context, Result};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use voice_controllm_proto::Event;
use voice_controllm_proto::voice_controllm_client::VoiceControllmClient;

/// Connect to daemon via Unix socket.
pub async fn connect(socket_path: impl AsRef<Path>) -> Result<VoiceControllmClient<Channel>> {
    let socket_path = socket_path.as_ref().to_path_buf();

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

/// Subscribe to daemon events.
pub async fn subscribe(
    client: &mut VoiceControllmClient<Channel>,
) -> Result<tonic::Streaming<Event>> {
    let response = client
        .subscribe(voice_controllm_proto::Empty {})
        .await
        .context("Failed to subscribe to events")?;
    Ok(response.into_inner())
}

/// Check if daemon is running by attempting to connect.
pub async fn is_daemon_running(socket_path: impl AsRef<Path>) -> bool {
    let socket_path = socket_path.as_ref();
    if !socket_path.exists() {
        return false;
    }
    connect(socket_path).await.is_ok()
}
