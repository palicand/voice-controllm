use super::*;
use tokio::sync::{broadcast, oneshot};

#[test]
fn test_service_creation() {
    let (tx, _rx) = broadcast::channel(16);
    let (shutdown_tx, _shutdown_rx) = oneshot::channel();
    let controller = Arc::new(Controller::new(tx, shutdown_tx));
    let _service = VoiceControllmService::new(controller);
}
