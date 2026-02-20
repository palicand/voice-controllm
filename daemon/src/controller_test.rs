use super::*;
use crate::config::{Config, DaemonConfig, InitialState};

fn create_controller() -> (Controller, oneshot::Receiver<()>) {
    create_controller_with_initial_state(InitialState::Paused)
}

fn create_controller_with_initial_state(
    initial_state: InitialState,
) -> (Controller, oneshot::Receiver<()>) {
    let (event_tx, _) = broadcast::channel(16);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let config = Config {
        daemon: DaemonConfig { initial_state },
        ..Config::default()
    };
    let engine = Engine::new(config.clone()).unwrap();
    (
        Controller::new(event_tx, shutdown_tx, engine, config),
        shutdown_rx,
    )
}

#[tokio::test]
async fn test_initial_state_is_initializing() {
    let (controller, _) = create_controller();
    assert_eq!(controller.state().await, ControllerState::Initializing);
}

#[tokio::test]
async fn test_mark_ready_transitions_to_paused() {
    let (controller, _) = create_controller();
    assert_eq!(controller.state().await, ControllerState::Initializing);
    controller.mark_ready().await;
    assert_eq!(controller.state().await, ControllerState::Paused);
}

#[tokio::test]
async fn test_start_listening_fails_during_initializing() {
    let (controller, _) = create_controller();
    let result = controller.start_listening().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_start_listening_requires_engine() {
    let (controller, _) = create_controller();
    controller.mark_ready().await;
    // Engine is not initialized (no models loaded), so this should fail
    let result = controller.start_listening().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_stop_listening_from_paused_is_noop() {
    let (controller, _) = create_controller();
    controller.mark_ready().await;
    let result = controller.stop_listening().await;
    assert!(result.is_ok());
    assert_eq!(controller.state().await, ControllerState::Paused);
}

#[tokio::test]
async fn test_shutdown_sends_signal() {
    let (controller, shutdown_rx) = create_controller();
    controller.shutdown().await;
    assert_eq!(controller.state().await, ControllerState::Stopped);
    // Receiver should complete (not hang)
    assert!(shutdown_rx.await.is_ok());
}

#[tokio::test]
async fn test_mark_ready_with_listening_initial_state_falls_back_to_paused() {
    // When initial_state is Listening but engine isn't initialized (no models),
    // start_listening fails gracefully and state remains Paused.
    let (controller, _) = create_controller_with_initial_state(InitialState::Listening);
    controller.mark_ready().await;
    assert_eq!(controller.state().await, ControllerState::Paused);
}

#[tokio::test]
async fn test_mark_ready_broadcasts_event() {
    let (event_tx, mut event_rx) = broadcast::channel(16);
    let (shutdown_tx, _) = oneshot::channel();
    let config = Config {
        daemon: DaemonConfig {
            initial_state: InitialState::Paused,
        },
        ..Config::default()
    };
    let engine = Engine::new(config.clone()).unwrap();
    let controller = Controller::new(event_tx, shutdown_tx, engine, config);

    controller.mark_ready().await;

    let event = event_rx.recv().await.unwrap();
    match event.event {
        Some(vcm_proto::event::Event::StateChange(change)) => match change.status {
            Some(vcm_proto::state_change::Status::NewState(state)) => {
                assert_eq!(state, State::Paused.into());
            }
            _ => panic!("Expected NewState"),
        },
        _ => panic!("Expected StateChange event"),
    }
}
