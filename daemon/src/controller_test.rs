use super::*;

fn create_controller() -> (Controller, oneshot::Receiver<()>) {
    let (event_tx, _) = broadcast::channel(16);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    (Controller::new(event_tx, shutdown_tx), shutdown_rx)
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
async fn test_start_listening_from_paused() {
    let (controller, _) = create_controller();
    controller.mark_ready().await;
    controller.start_listening().await.unwrap();
    assert_eq!(controller.state().await, ControllerState::Listening);
}

#[tokio::test]
async fn test_stop_listening_from_listening() {
    let (controller, _) = create_controller();
    controller.mark_ready().await;
    controller.start_listening().await.unwrap();
    controller.stop_listening().await.unwrap();
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
async fn test_start_listening_broadcasts_event() {
    let (event_tx, mut event_rx) = broadcast::channel(16);
    let (shutdown_tx, _) = oneshot::channel();
    let controller = Controller::new(event_tx, shutdown_tx);

    controller.mark_ready().await;
    // Drain the mark_ready state change event
    let _ = event_rx.recv().await;

    controller.start_listening().await.unwrap();

    let event = event_rx.recv().await.unwrap();
    match event.event {
        Some(voice_controllm_proto::event::Event::StateChange(change)) => match change.status {
            Some(voice_controllm_proto::state_change::Status::NewState(state)) => {
                assert_eq!(state, State::Listening.into());
            }
            _ => panic!("Expected NewState"),
        },
        _ => panic!("Expected StateChange event"),
    }
}
