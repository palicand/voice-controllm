//! Controller manages daemon state and coordinates components.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, broadcast, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use voice_controllm_proto::{Event, State, StateChange, Transcription};

use crate::config::{Config, InitialState, InjectionConfig};
use crate::engine::{Engine, SharedLanguage};
use crate::inject::KeystrokeInjector;

/// Controller state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerState {
    Initializing,
    Stopped,
    Listening,
    Paused,
}

impl From<ControllerState> for State {
    fn from(state: ControllerState) -> Self {
        match state {
            ControllerState::Initializing => State::Initializing,
            ControllerState::Stopped => State::Stopped,
            ControllerState::Listening => State::Listening,
            ControllerState::Paused => State::Paused,
        }
    }
}

/// Event sender type.
pub type EventSender = broadcast::Sender<Event>;

/// Handle for a running engine task.
struct EngineHandle {
    cancel: CancellationToken,
    join: JoinHandle<(Engine, anyhow::Result<()>)>,
}

/// Controller for daemon state management.
pub struct Controller {
    state: Arc<RwLock<ControllerState>>,
    event_tx: EventSender,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    engine: Arc<Mutex<Option<Engine>>>,
    engine_handle: Arc<RwLock<Option<EngineHandle>>>,
    injection_config: InjectionConfig,
    initial_state: InitialState,
    shared_language: SharedLanguage,
    config: Arc<RwLock<Config>>,
}

impl Controller {
    /// Create a new controller.
    pub fn new(
        event_tx: EventSender,
        shutdown_tx: oneshot::Sender<()>,
        engine: Engine,
        config: Config,
    ) -> Self {
        let shared_language = engine.shared_language();
        let injection_config = config.injection.clone();
        let initial_state = config.daemon.initial_state;
        Self {
            state: Arc::new(RwLock::new(ControllerState::Initializing)),
            event_tx,
            shutdown_tx: Arc::new(RwLock::new(Some(shutdown_tx))),
            engine: Arc::new(Mutex::new(Some(engine))),
            engine_handle: Arc::new(RwLock::new(None)),
            injection_config,
            initial_state,
            shared_language,
            config: Arc::new(RwLock::new(config)),
        }
    }

    /// Get the current state.
    pub async fn state(&self) -> ControllerState {
        *self.state.read().await
    }

    /// Mark initialization complete, transition to configured initial state.
    pub async fn mark_ready(&self) {
        {
            let mut state = self.state.write().await;
            if *state != ControllerState::Initializing {
                return;
            }
            *state = ControllerState::Paused;
            self.broadcast_state_change(ControllerState::Paused);
        }

        if self.initial_state == InitialState::Listening
            && let Err(e) = self.start_listening().await
        {
            error!(error = %e, "Failed to auto-start listening after initialization");
        }
    }

    /// Start listening — spawns the engine audio loop.
    pub async fn start_listening(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        match *state {
            ControllerState::Paused => {
                // Take engine out
                let engine = self
                    .engine
                    .lock()
                    .await
                    .take()
                    .ok_or("Engine not available")?;

                if !engine.is_initialized() {
                    // Put it back
                    *self.engine.lock().await = Some(engine);
                    return Err("Engine not initialized".to_string());
                }

                let cancel = CancellationToken::new();
                let cancel_clone = cancel.clone();
                let event_tx = self.event_tx.clone();
                let injection_config = self.injection_config.clone();

                let join = tokio::spawn(async move {
                    run_engine_task(engine, cancel_clone, event_tx, injection_config).await
                });

                *self.engine_handle.write().await = Some(EngineHandle { cancel, join });
                *state = ControllerState::Listening;
                self.broadcast_state_change(ControllerState::Listening);
                Ok(())
            }
            ControllerState::Listening => Ok(()),
            ControllerState::Stopped => Err("Daemon is stopped".to_string()),
            ControllerState::Initializing => Err("Daemon is still initializing".to_string()),
        }
    }

    /// Stop listening — cancels the engine audio loop.
    pub async fn stop_listening(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        match *state {
            ControllerState::Listening => {
                // Cancel and await engine task
                if let Some(handle) = self.engine_handle.write().await.take() {
                    handle.cancel.cancel();
                    match handle.join.await {
                        Ok((engine, result)) => {
                            if let Err(e) = result {
                                error!(error = %e, "Engine task finished with error");
                            }
                            *self.engine.lock().await = Some(engine);
                        }
                        Err(e) => {
                            error!(error = %e, "Engine task panicked");
                            self.broadcast_error("Engine task panicked");
                        }
                    }
                }

                *state = ControllerState::Paused;
                self.broadcast_state_change(ControllerState::Paused);
                Ok(())
            }
            ControllerState::Paused => Ok(()),
            ControllerState::Stopped => Err("Daemon is stopped".to_string()),
            ControllerState::Initializing => Err("Daemon is still initializing".to_string()),
        }
    }

    /// Trigger shutdown.
    pub async fn shutdown(&self) {
        // Stop listening first if active
        let _ = self.stop_listening().await;

        let mut state = self.state.write().await;
        *state = ControllerState::Stopped;
        self.broadcast_state_change(ControllerState::Stopped);

        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(());
        }
    }

    /// Get the engine for initialization (used by daemon runner).
    pub async fn take_engine(&self) -> Option<Engine> {
        self.engine.lock().await.take()
    }

    /// Return the engine after initialization.
    pub async fn return_engine(&self, engine: Engine) {
        *self.engine.lock().await = Some(engine);
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

    /// Broadcast an error event.
    fn broadcast_error(&self, message: &str) {
        let event = Event {
            event: Some(voice_controllm_proto::event::Event::DaemonError(
                voice_controllm_proto::DaemonError {
                    kind: voice_controllm_proto::ErrorKind::ErrorEngine.into(),
                    message: message.to_string(),
                    model_name: String::new(),
                },
            )),
        };
        let _ = self.event_tx.send(event);
    }

    /// Get the event sender for creating subscribers.
    pub fn event_sender(&self) -> EventSender {
        self.event_tx.clone()
    }

    /// Set the transcription language at runtime.
    ///
    /// Pass `"auto"` for automatic detection, or a language code like `"en"`, `"cs"`, etc.
    /// The change takes effect on the next transcription call and is persisted to the config file.
    pub async fn set_language(&self, language: &str) -> Result<(), String> {
        let lang = if language == "auto" {
            None
        } else {
            Some(language.to_string())
        };

        // Persist to config first so failures don't partially apply the change
        {
            let mut config = self.config.write().await;
            config.model.language = language.to_string();
            config
                .save()
                .map_err(|e| format!("Failed to save config: {e}"))?;
        }

        // Update shared runtime state
        {
            let mut shared = self
                .shared_language
                .lock()
                .map_err(|e| format!("Failed to lock shared language: {e}"))?;
            *shared = lang;
        }

        info!(language = language, "Language changed");
        Ok(())
    }

    /// Get the current language and the list of available languages from config.
    ///
    /// Returns `(active_language, available_languages)`.
    pub async fn get_language_info(&self) -> (String, Vec<String>) {
        let active = {
            let shared = self.shared_language.lock().ok();
            match shared.as_deref() {
                Some(Some(lang)) => lang.to_string(),
                _ => "auto".to_string(),
            }
        };
        let available = self.config.read().await.gui.languages.clone();
        (active, available)
    }
}

/// Run the engine in a background task, returning the engine when done.
async fn run_engine_task(
    mut engine: Engine,
    cancel: CancellationToken,
    event_tx: EventSender,
    injection_config: InjectionConfig,
) -> (Engine, anyhow::Result<()>) {
    let result = match KeystrokeInjector::new(injection_config) {
        Ok(mut injector) => {
            let tx = event_tx.clone();
            engine
                .run_loop(cancel, move |text| {
                    info!(text = %text, "Transcription -> injecting");
                    if let Err(e) = injector.inject_text(text) {
                        error!(error = %e, "Keystroke injection failed");
                    }
                    // Broadcast transcription event
                    let event = Event {
                        event: Some(voice_controllm_proto::event::Event::Transcription(
                            Transcription {
                                text: text.to_string(),
                                confidence: 0.0,
                                is_partial: false,
                            },
                        )),
                    };
                    let _ = tx.send(event);
                })
                .await
        }
        Err(e) => Err(e),
    };

    (engine, result)
}

#[cfg(test)]
#[path = "controller_test.rs"]
mod tests;
