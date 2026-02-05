//! Controller manages daemon state and coordinates components.

use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, oneshot};
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
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl Controller {
    /// Create a new controller with a shutdown channel.
    pub fn new(event_tx: EventSender, shutdown_tx: oneshot::Sender<()>) -> Self {
        Self {
            state: Arc::new(RwLock::new(ControllerState::Paused)),
            event_tx,
            shutdown_tx: Arc::new(RwLock::new(Some(shutdown_tx))),
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
            ControllerState::Listening => Ok(()),
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
            ControllerState::Paused => Ok(()),
            ControllerState::Stopped => Err("Daemon is stopped".to_string()),
        }
    }

    /// Trigger shutdown.
    pub async fn shutdown(&self) {
        let mut state = self.state.write().await;
        *state = ControllerState::Stopped;
        self.broadcast_state_change(ControllerState::Stopped);

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
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
        let _ = self.event_tx.send(event);
    }

    /// Get the event sender for creating subscribers.
    pub fn event_sender(&self) -> EventSender {
        self.event_tx.clone()
    }
}

#[cfg(test)]
#[path = "controller_test.rs"]
mod tests;
