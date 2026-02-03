//! gRPC server for daemon control.

use std::pin::Pin;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use voice_controllm_proto::{
    Empty, Event, Healthy, State,
    voice_controllm_server::{VoiceControllm, VoiceControllmServer},
};

/// Event sender for broadcasting to subscribers.
pub type EventSender = broadcast::Sender<Event>;

/// gRPC service implementation.
pub struct VoiceControllmService {
    event_tx: EventSender,
}

impl VoiceControllmService {
    /// Create a new service with the given event sender.
    pub fn new(event_tx: EventSender) -> Self {
        Self { event_tx }
    }

    /// Create the tonic server.
    pub fn into_server(self) -> VoiceControllmServer<Self> {
        VoiceControllmServer::new(self)
    }
}

#[tonic::async_trait]
impl VoiceControllm for VoiceControllmService {
    async fn start_listening(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        // Stub: will be wired to controller in Task 3
        Ok(Response::new(Empty {}))
    }

    async fn stop_listening(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        // Stub: will be wired to controller in Task 3
        Ok(Response::new(Empty {}))
    }

    async fn shutdown(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        // Stub: will be wired to controller in Task 3
        Ok(Response::new(Empty {}))
    }

    async fn get_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<voice_controllm_proto::Status>, Status> {
        // Stub: return paused state
        let status = voice_controllm_proto::Status {
            status: Some(voice_controllm_proto::status::Status::Healthy(Healthy {
                state: State::Paused.into(),
            })),
        };
        Ok(Response::new(status))
    }

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Event, Status>> + Send>>;

    async fn subscribe(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let rx = self.event_tx.subscribe();
        let stream = BroadcastStream::new(rx)
            .map(|result| result.map_err(|e| Status::internal(format!("Broadcast error: {}", e))));
        Ok(Response::new(Box::pin(stream)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let (tx, _rx) = broadcast::channel(16);
        let _service = VoiceControllmService::new(tx);
    }
}
