//! gRPC server for daemon control.

use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use voice_controllm_proto::{
    Empty, Event, Healthy, State,
    voice_controllm_server::{VoiceControllm, VoiceControllmServer},
};

use crate::controller::{Controller, ControllerState};

/// gRPC service implementation.
pub struct VoiceControllmService {
    controller: Arc<Controller>,
}

impl VoiceControllmService {
    /// Create a new service with the given controller.
    pub fn new(controller: Arc<Controller>) -> Self {
        Self { controller }
    }

    /// Create the tonic server.
    pub fn into_server(self) -> VoiceControllmServer<Self> {
        VoiceControllmServer::new(self)
    }
}

#[tonic::async_trait]
impl VoiceControllm for VoiceControllmService {
    async fn start_listening(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        self.controller
            .start_listening()
            .await
            .map_err(Status::failed_precondition)?;
        Ok(Response::new(Empty {}))
    }

    async fn stop_listening(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        self.controller
            .stop_listening()
            .await
            .map_err(Status::failed_precondition)?;
        Ok(Response::new(Empty {}))
    }

    async fn shutdown(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        // Will be implemented in Task 8
        Ok(Response::new(Empty {}))
    }

    async fn get_status(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<voice_controllm_proto::Status>, Status> {
        let state = self.controller.state().await;
        let proto_state = match state {
            ControllerState::Stopped => State::Stopped,
            ControllerState::Listening => State::Listening,
            ControllerState::Paused => State::Paused,
        };
        let status = voice_controllm_proto::Status {
            status: Some(voice_controllm_proto::status::Status::Healthy(Healthy {
                state: proto_state.into(),
            })),
        };
        Ok(Response::new(status))
    }

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Event, Status>> + Send>>;

    async fn subscribe(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let rx = self.controller.event_sender().subscribe();
        let stream = BroadcastStream::new(rx)
            .map(|result| result.map_err(|e| Status::internal(format!("Broadcast error: {}", e))));
        Ok(Response::new(Box::pin(stream)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[test]
    fn test_service_creation() {
        let (tx, _rx) = broadcast::channel(16);
        let controller = Arc::new(Controller::new(tx));
        let _service = VoiceControllmService::new(controller);
    }
}
