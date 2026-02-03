//! Controller manages daemon state and coordinates components.

use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use voice_controllm_proto::{Event, State, StateChange};

/// Controller state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerState {
    Stopped,
    Listening,
    Paused,
}

impl From<ControllerState> for State {
    fn from(state: ControllerState) -> Self {
        match state {
            ControllerState::Stopped => State::Stopped,
            ControllerState::Listening => State::Listening,
            ControllerState::Paused => State::Paused,
        }
    }
}

/// Event sender type.
pub type EventSender = broadcast::Sender<Event>;

/// Controller for daemon state management.
pub struct Controller {
    state: Arc<RwLock<ControllerState>>,
    event_tx: EventSender,
}

impl Controller {
    /// Create a new controller.
    pub fn new(event_tx: EventSender) -> Self {
        Self {
            state: Arc::new(RwLock::new(ControllerState::Paused)),
            event_tx,
        }
    }

    /// Get the current state.
    pub async fn state(&self) -> ControllerState {
        *self.state.read().await
    }

    /// Start listening.
    pub async fn start_listening(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        match *state {
            ControllerState::Paused => {
                *state = ControllerState::Listening;
                self.broadcast_state_change(ControllerState::Listening);
                Ok(())
            }
            ControllerState::Listening => Ok(()), // Already listening
            ControllerState::Stopped => Err("Daemon is stopped".to_string()),
        }
    }

    /// Stop listening (pause).
    pub async fn stop_listening(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        match *state {
            ControllerState::Listening => {
                *state = ControllerState::Paused;
                self.broadcast_state_change(ControllerState::Paused);
                Ok(())
            }
            ControllerState::Paused => Ok(()), // Already paused
            ControllerState::Stopped => Err("Daemon is stopped".to_string()),
        }
    }

    /// Broadcast a state change event.
    fn broadcast_state_change(&self, new_state: ControllerState) {
        let event = Event {
            event: Some(voice_controllm_proto::event::Event::StateChange(
                StateChange {
                    status: Some(voice_controllm_proto::state_change::Status::NewState(
                        State::from(new_state).into(),
                    )),
                },
            )),
        };
        // Ignore send errors (no subscribers)
        let _ = self.event_tx.send(event);
    }

    /// Get the event sender for creating subscribers.
    pub fn event_sender(&self) -> EventSender {
        self.event_tx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_state_is_paused() {
        let (tx, _rx) = broadcast::channel(16);
        let controller = Controller::new(tx);
        assert_eq!(controller.state().await, ControllerState::Paused);
    }

    #[tokio::test]
    async fn test_start_listening_from_paused() {
        let (tx, _rx) = broadcast::channel(16);
        let controller = Controller::new(tx);

        controller.start_listening().await.unwrap();
        assert_eq!(controller.state().await, ControllerState::Listening);
    }

    #[tokio::test]
    async fn test_stop_listening_from_listening() {
        let (tx, _rx) = broadcast::channel(16);
        let controller = Controller::new(tx);

        controller.start_listening().await.unwrap();
        controller.stop_listening().await.unwrap();
        assert_eq!(controller.state().await, ControllerState::Paused);
    }

    #[tokio::test]
    async fn test_start_listening_broadcasts_event() {
        let (tx, mut rx) = broadcast::channel(16);
        let controller = Controller::new(tx);

        controller.start_listening().await.unwrap();

        let event = rx.recv().await.unwrap();
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
}
