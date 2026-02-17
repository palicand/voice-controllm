use super::*;
use crate::config::Config;
use crate::engine::Engine;
use tokio::sync::{broadcast, oneshot};

#[test]
fn test_service_creation() {
    let (tx, _rx) = broadcast::channel(16);
    let (shutdown_tx, _shutdown_rx) = oneshot::channel();
    let config = Config::default();
    let engine = Engine::new(config.clone()).unwrap();
    let controller = Arc::new(Controller::new(tx, shutdown_tx, engine, config));
    let _service = VoiceControllmService::new(controller);
}
