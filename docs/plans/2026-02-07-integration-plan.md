# Controller-Engine Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the daemon's Controller to the Engine so `vcm start` / `vcm toggle` actually captures audio, transcribes speech, and injects keystrokes.

**Architecture:** The daemon starts the gRPC server first (so the CLI can subscribe to events immediately), then initializes the Engine (downloads/loads models) in a background task emitting progress events. The Controller owns the Engine and spawns/cancels audio loop tasks on toggle.

**Tech Stack:** Rust, tonic/prost (gRPC), tokio + tokio-util (async runtime + CancellationToken), cpal, whisper-rs, enigo

**Design doc:** `docs/plans/2026-02-07-integration-design.md`

---

### Task 1: Extend proto with new event types and state

**Files:**
- Modify: `proto/src/voice_controllm.proto`

**Step 1: Update the proto file**

Replace the entire contents of `proto/src/voice_controllm.proto` with:

```protobuf
syntax = "proto3";
package voice_controllm;

service VoiceControllm {
  // Control
  rpc StartListening(Empty) returns (Empty);
  rpc StopListening(Empty) returns (Empty);
  rpc Shutdown(Empty) returns (Empty);
  rpc DownloadModels(Empty) returns (Empty);

  // Query
  rpc GetStatus(Empty) returns (Status);

  // Streaming
  rpc Subscribe(Empty) returns (stream Event);
}

message Empty {}

message Status {
  oneof status {
    Healthy healthy = 1;
    Error error = 2;
  }
}

message Healthy {
  State state = 1;
}

enum State {
  STATE_STOPPED = 0;
  STATE_LISTENING = 1;
  STATE_PAUSED = 2;
  STATE_INITIALIZING = 3;
}

message Error {
  string message = 1;
}

message Event {
  oneof event {
    StateChange state_change = 1;
    Transcription transcription = 2;
    InitProgress init_progress = 3;
    DaemonError daemon_error = 4;
  }
}

message StateChange {
  oneof status {
    State new_state = 1;
    Error error = 2;
  }
}

message Transcription {
  string text = 1;
  double confidence = 2;
  bool is_partial = 3;
}

message InitProgress {
  oneof progress {
    ModelDownload model_download = 1;
    ModelLoad model_load = 2;
    Ready ready = 3;
  }
}

message ModelDownload {
  string model_name = 1;
  uint64 bytes_downloaded = 2;
  uint64 bytes_total = 3;
}

message ModelLoad {
  string model_name = 1;
}

message Ready {}

message DaemonError {
  ErrorKind kind = 1;
  string message = 2;
  string model_name = 3;
}

enum ErrorKind {
  ERROR_UNKNOWN = 0;
  ERROR_MODEL_MISSING = 1;
  ERROR_MODEL_CORRUPTED = 2;
  ERROR_MIC_ACCESS_DENIED = 3;
  ERROR_ENGINE = 4;
}
```

**Step 2: Verify proto compiles**

Run: `cargo build -p voice-controllm-proto`
Expected: SUCCESS (the rest of the workspace will have compile errors — that's fine, we'll fix them in subsequent tasks)

**Step 3: Fix compile errors in daemon and CLI from proto changes**

The `Event` oneof now has new variants. The existing code in `daemon/src/controller.rs` and `daemon/src/server.rs` should still compile since we didn't remove any existing fields. But `State` now has a new variant `Initializing` — check that match statements in `cli/src/main.rs:cmd_status()` and `cli/src/main.rs:cmd_toggle()` handle it.

Add an `Initializing` arm to `cmd_status()` (around line 180):
```rust
State::Initializing => println!("Initializing..."),
```

Add an `Initializing` arm to `cmd_toggle()` (around line 231):
```rust
State::Initializing => {
    println!("Daemon is still initializing, please wait...");
}
```

Add `Initializing` variant to `ControllerState` in `daemon/src/controller.rs` and its `From<ControllerState> for State` impl:
```rust
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
```

Update `Controller::new()` to start in `Initializing` state instead of `Paused`.

Update `start_listening()` to also reject `Initializing`:
```rust
ControllerState::Initializing => Err("Daemon is still initializing".to_string()),
```

Update `stop_listening()` similarly.

Add a new method to Controller:
```rust
pub async fn mark_ready(&self) {
    let mut state = self.state.write().await;
    if *state == ControllerState::Initializing {
        *state = ControllerState::Paused;
        self.broadcast_state_change(ControllerState::Paused);
    }
}
```

Update `daemon/src/server.rs:get_status()` to handle the new state:
```rust
ControllerState::Initializing => State::Initializing,
```

**Step 4: Verify full workspace compiles**

Run: `cargo build`
Expected: SUCCESS

**Step 5: Update existing controller tests**

In `daemon/src/controller_test.rs`, update `test_initial_state_is_paused` to expect `Initializing`:
```rust
#[tokio::test]
async fn test_initial_state_is_initializing() {
    let (controller, _) = create_controller();
    assert_eq!(controller.state().await, ControllerState::Initializing);
}
```

Add test for `mark_ready`:
```rust
#[tokio::test]
async fn test_mark_ready_transitions_to_paused() {
    let (controller, _) = create_controller();
    assert_eq!(controller.state().await, ControllerState::Initializing);
    controller.mark_ready().await;
    assert_eq!(controller.state().await, ControllerState::Paused);
}
```

Add test that `start_listening` fails during Initializing:
```rust
#[tokio::test]
async fn test_start_listening_fails_during_initializing() {
    let (controller, _) = create_controller();
    let result = controller.start_listening().await;
    assert!(result.is_err());
}
```

Update `test_start_listening_from_paused` and other tests that assume initial state is Paused to call `controller.mark_ready().await` first.

**Step 6: Run tests**

Run: `cargo test -p voice-controllm-daemon`
Expected: All tests pass

**Step 7: Commit**

```bash
git add proto/src/voice_controllm.proto daemon/src/controller.rs daemon/src/controller_test.rs daemon/src/server.rs cli/src/main.rs
git commit -m "feat(proto): add initialization events, error types, and DownloadModels RPC"
```

---

### Task 2: Add model integrity checking to ModelManager

**Files:**
- Modify: `daemon/src/models.rs`
- Modify: `daemon/src/models_test.rs`

**Step 1: Write failing tests**

Add to `daemon/src/models_test.rs`:

```rust
#[tokio::test]
async fn test_check_model_missing() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());
    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Missing));
}

#[tokio::test]
async fn test_check_model_ready() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());

    // Create a file with the correct size
    let info = ModelId::SileroVad.info();
    let path = temp.path().join(info.filename);
    let data = vec![0u8; info.size_bytes.unwrap() as usize];
    tokio::fs::write(&path, &data).await.unwrap();

    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Ready(_)));
}

#[tokio::test]
async fn test_check_model_corrupted_wrong_size() {
    let temp = TempDir::new().unwrap();
    let manager = ModelManager::with_dir(temp.path());

    // Create a file with wrong size
    let info = ModelId::SileroVad.info();
    let path = temp.path().join(info.filename);
    tokio::fs::write(&path, b"too small").await.unwrap();

    let status = manager.check_model(ModelId::SileroVad).await;
    assert!(matches!(status, ModelStatus::Corrupted { .. }));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p voice-controllm-daemon check_model`
Expected: FAIL — `ModelStatus` doesn't exist, `check_model` doesn't exist

**Step 3: Implement ModelStatus and check_model**

Add to `daemon/src/models.rs`, after the `ModelId` enum:

```rust
/// Result of checking a model's status on disk.
#[derive(Debug)]
pub enum ModelStatus {
    /// Model file exists and passes validation.
    Ready(PathBuf),
    /// Model file does not exist.
    Missing,
    /// Model file exists but failed validation.
    Corrupted { path: PathBuf, reason: String },
}
```

Make `ModelId::info()` pub (currently it's `fn info` — change to `pub fn info`) so tests can access it.

Add `Display` impl for `ModelId`:
```rust
impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelId::SileroVad => write!(f, "silero-vad"),
            ModelId::WhisperTiny => write!(f, "whisper-tiny"),
            ModelId::WhisperTinyEn => write!(f, "whisper-tiny-en"),
            ModelId::WhisperBase => write!(f, "whisper-base"),
            ModelId::WhisperBaseEn => write!(f, "whisper-base-en"),
            ModelId::WhisperSmall => write!(f, "whisper-small"),
            ModelId::WhisperSmallEn => write!(f, "whisper-small-en"),
            ModelId::WhisperMedium => write!(f, "whisper-medium"),
            ModelId::WhisperMediumEn => write!(f, "whisper-medium-en"),
            ModelId::WhisperLargeV3 => write!(f, "whisper-large-v3"),
            ModelId::WhisperLargeV3Turbo => write!(f, "whisper-large-v3-turbo"),
        }
    }
}
```

Add method to `ModelManager`:

```rust
/// Check model status without downloading.
pub async fn check_model(&self, model: ModelId) -> ModelStatus {
    let info = model.info();
    let model_path = self.models_dir.join(info.filename);

    if !model_path.exists() {
        return ModelStatus::Missing;
    }

    if let Some(expected_size) = info.size_bytes {
        match fs::metadata(&model_path).await {
            Ok(metadata) if metadata.len() != expected_size => {
                return ModelStatus::Corrupted {
                    path: model_path,
                    reason: format!(
                        "expected {} bytes, found {}",
                        expected_size,
                        metadata.len()
                    ),
                };
            }
            Err(e) => {
                return ModelStatus::Corrupted {
                    path: model_path,
                    reason: format!("cannot read file metadata: {}", e),
                };
            }
            Ok(_) => {}
        }
    }

    ModelStatus::Ready(model_path)
}
```

**Step 4: Run tests**

Run: `cargo test -p voice-controllm-daemon check_model`
Expected: All 3 tests pass

**Step 5: Commit**

```bash
git add daemon/src/models.rs daemon/src/models_test.rs
git commit -m "feat(daemon): add model integrity checking"
```

---

### Task 3: Add tokio-util dependency

**Files:**
- Modify: `daemon/Cargo.toml` (via `cargo add`)

**Step 1: Add dependency**

Run: `cargo add tokio-util --features rt -p voice-controllm-daemon`

**Step 2: Verify it compiles**

Run: `cargo build -p voice-controllm-daemon`
Expected: SUCCESS

**Step 3: Commit**

```bash
git add daemon/Cargo.toml Cargo.lock
git commit -m "build(deps): add tokio-util for CancellationToken"
```

---

### Task 4: Refactor Engine — split run() into initialize() + run_loop()

**Files:**
- Modify: `daemon/src/engine.rs`
- Modify: `daemon/src/engine_test.rs`

**Step 1: Write failing tests for the new API**

Add to `daemon/src/engine_test.rs`:

```rust
#[test]
fn test_engine_not_initialized_by_default() {
    let config = Config::default();
    let engine = Engine::new(config).unwrap();
    assert!(!engine.is_initialized());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p voice-controllm-daemon test_engine_not_initialized`
Expected: FAIL — `is_initialized` doesn't exist

**Step 3: Refactor Engine struct**

Replace `daemon/src/engine.rs` with the refactored version. Key changes:

1. Add `InitEvent` enum for progress callbacks:
```rust
/// Events emitted during engine initialization.
#[derive(Debug, Clone)]
pub enum InitEvent {
    /// Model is being downloaded.
    Downloading {
        model: String,
        bytes: u64,
        total: u64,
    },
    /// Model is being loaded into memory.
    Loading { model: String },
    /// Engine is ready.
    Ready,
}
```

2. Add `InitializedComponents` struct to hold loaded models:
```rust
struct InitializedComponents {
    vad: VoiceActivityDetector,
    transcriber: WhisperTranscriber,
}
```

3. Update `Engine` struct:
```rust
pub struct Engine {
    config: Config,
    model_manager: ModelManager,
    components: Option<InitializedComponents>,
}
```

4. Remove `EngineState` enum (no longer needed — state is managed by Controller).

5. Replace `run()` with two methods:

```rust
/// Check if the engine has been initialized (models loaded).
pub fn is_initialized(&self) -> bool {
    self.components.is_some()
}

/// Initialize the engine: download and load models.
///
/// Calls `on_progress` with status updates suitable for UI display.
/// After this returns Ok(()), the engine is ready for `run_loop()`.
pub async fn initialize(
    &mut self,
    on_progress: impl Fn(InitEvent) + Send,
) -> Result<()> {
    info!("Initializing engine");

    // Ensure VAD model
    on_progress(InitEvent::Loading {
        model: "silero-vad".to_string(),
    });
    let vad_model_path = self
        .model_manager
        .ensure_model(ModelId::SileroVad)
        .await
        .context("Failed to ensure VAD model")?;

    // Ensure Whisper model
    let whisper_model_id = speech_model_to_model_id(self.config.model.model);
    on_progress(InitEvent::Loading {
        model: whisper_model_id.to_string(),
    });
    let whisper_model_path = self
        .model_manager
        .ensure_model(whisper_model_id)
        .await
        .context("Failed to ensure Whisper model")?;

    info!("Models ready, initializing components");

    // Initialize VAD
    let vad = VoiceActivityDetector::new(&vad_model_path, VadConfig::default())
        .context("Failed to initialize VAD")?;

    // Initialize transcriber
    let language = if self.config.model.languages.first().map(|s| s.as_str()) == Some("auto") {
        None
    } else {
        self.config.model.languages.first().cloned()
    };
    let transcriber = WhisperTranscriber::new(&whisper_model_path, language)
        .context("Failed to initialize Whisper")?;

    self.components = Some(InitializedComponents { vad, transcriber });

    on_progress(InitEvent::Ready);
    info!("Engine initialized");

    Ok(())
}

/// Run the audio capture and transcription loop.
///
/// Blocks until the `cancel` token is cancelled.
/// Requires `initialize()` to have been called first.
pub async fn run_loop(
    &mut self,
    cancel: CancellationToken,
    mut on_transcription: impl FnMut(&str),
) -> Result<()> {
    let components = self
        .components
        .as_mut()
        .context("Engine not initialized — call initialize() first")?;

    info!("Starting audio capture");

    // Initialize audio capture
    let capture = AudioCapture::start().context("Failed to start audio capture")?;
    let sample_rate = capture.sample_rate();
    info!(
        sample_rate = sample_rate,
        target_rate = TARGET_SAMPLE_RATE,
        "Audio capture started"
    );

    // Initialize resampler
    let mut resampler = AudioResampler::new(sample_rate, TARGET_SAMPLE_RATE, 1024)
        .context("Failed to create resampler")?;

    // Buffers
    let mut input_buffer: Vec<f32> = Vec::new();
    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut speech_buffer: Vec<f32> = Vec::new();

    let resampler_chunk = resampler.chunk_size();
    let vad_chunk_size = components.vad.chunk_size();

    info!("Listening for speech...");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Cancellation received, stopping audio capture");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                if let Some(samples) = capture.try_recv() {
                    input_buffer.extend(samples);

                    // Process complete resampler chunks
                    while input_buffer.len() >= resampler_chunk {
                        let chunk: Vec<f32> = input_buffer.drain(..resampler_chunk).collect();
                        if let Ok(resampled) = resampler.process(&chunk) {
                            vad_buffer.extend(resampled);
                        }
                    }

                    // Process complete VAD chunks
                    while vad_buffer.len() >= vad_chunk_size {
                        let chunk: Vec<f32> = vad_buffer.drain(..vad_chunk_size).collect();

                        if components.vad.is_speaking() {
                            speech_buffer.extend(&chunk);
                        }

                        match components.vad.process(&chunk) {
                            Ok(Some(VadEvent::SpeechStart)) => {
                                debug!("Speech started");
                                speech_buffer.clear();
                                speech_buffer.extend(&chunk);
                            }
                            Ok(Some(VadEvent::SpeechEnd)) => {
                                debug!(
                                    samples = speech_buffer.len(),
                                    duration_secs =
                                        speech_buffer.len() as f32 / VAD_SAMPLE_RATE as f32,
                                    "Speech ended, transcribing"
                                );

                                if !speech_buffer.is_empty() {
                                    match components
                                        .transcriber
                                        .transcribe(&speech_buffer, VAD_SAMPLE_RATE)
                                    {
                                        Ok(text) => {
                                            if !text.is_empty() {
                                                info!(text = %text, "Transcription complete");
                                                on_transcription(&text);
                                            }
                                        }
                                        Err(e) => {
                                            error!(error = %e, "Transcription failed");
                                        }
                                    }
                                }
                                speech_buffer.clear();
                            }
                            Ok(None) => {}
                            Err(e) => {
                                warn!(error = %e, "VAD processing error");
                            }
                        }
                    }
                }
            }
        }
    }

    capture.stop();
    info!("Audio capture stopped");

    Ok(())
}
```

6. Keep the old `run()` method as a convenience that calls both (so examples still work):
```rust
/// Run the full pipeline (initialize + loop). Convenience for examples/tests.
///
/// Deprecated: prefer calling `initialize()` + `run_loop()` separately.
pub async fn run<F>(&mut self, running: Arc<AtomicBool>, on_transcription: F) -> Result<()>
where
    F: FnMut(&str),
{
    self.initialize(|_| {}).await?;

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Bridge AtomicBool to CancellationToken
    tokio::spawn(async move {
        loop {
            if !running.load(Ordering::SeqCst) {
                cancel_clone.cancel();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });

    self.run_loop(cancel, on_transcription).await
}
```

Add necessary imports at the top:
```rust
use tokio_util::sync::CancellationToken;
```

**Step 4: Run tests**

Run: `cargo test -p voice-controllm-daemon`
Expected: All tests pass (including the new `test_engine_not_initialized_by_default` and existing tests)

**Step 5: Verify examples still compile**

Run: `cargo build --examples -p voice-controllm-daemon`
Expected: SUCCESS (the `run()` compat method keeps them working)

**Step 6: Commit**

```bash
git add daemon/src/engine.rs daemon/src/engine_test.rs
git commit -m "refactor(daemon): split Engine::run into initialize() + run_loop()"
```

---

### Task 5: Wire Engine into Controller

**Files:**
- Modify: `daemon/src/controller.rs`
- Modify: `daemon/src/controller_test.rs`

**Step 1: Write failing test**

Add to `daemon/src/controller_test.rs`:

```rust
#[tokio::test]
async fn test_start_listening_requires_engine() {
    let (event_tx, _) = broadcast::channel(16);
    let (shutdown_tx, _) = oneshot::channel();
    let config = Config::default();
    let engine = Engine::new(config).unwrap();
    let injection_config = InjectionConfig::default();
    let controller = Controller::new(event_tx, shutdown_tx, engine, injection_config);
    controller.mark_ready().await;
    // This will fail because engine is not initialized
    // But it should attempt it — proving the engine is wired
    let result = controller.start_listening().await;
    assert!(result.is_err()); // Engine not initialized
}
```

**Step 2: Run to verify it fails**

Run: `cargo test -p voice-controllm-daemon test_start_listening_requires_engine`
Expected: FAIL — `Controller::new` doesn't accept engine/injection_config args

**Step 3: Update Controller**

Rewrite `daemon/src/controller.rs`:

```rust
//! Controller manages daemon state and coordinates components.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, broadcast, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use voice_controllm_proto::{Event, State, StateChange, Transcription};

use crate::config::InjectionConfig;
use crate::engine::Engine;
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
}

impl Controller {
    /// Create a new controller.
    pub fn new(
        event_tx: EventSender,
        shutdown_tx: oneshot::Sender<()>,
        engine: Engine,
        injection_config: InjectionConfig,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(ControllerState::Initializing)),
            event_tx,
            shutdown_tx: Arc::new(RwLock::new(Some(shutdown_tx))),
            engine: Arc::new(Mutex::new(Some(engine))),
            engine_handle: Arc::new(RwLock::new(None)),
            injection_config,
        }
    }

    /// Get the current state.
    pub async fn state(&self) -> ControllerState {
        *self.state.read().await
    }

    /// Mark initialization complete, transition to Paused.
    pub async fn mark_ready(&self) {
        let mut state = self.state.write().await;
        if *state == ControllerState::Initializing {
            *state = ControllerState::Paused;
            self.broadcast_state_change(ControllerState::Paused);
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
                    let result =
                        run_engine_task(engine, cancel_clone, event_tx, injection_config).await;
                    result
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
                    info!(text = %text, "Transcription → injecting");
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
```

**Step 4: Update all call sites**

In `daemon/src/daemon.rs`, update `Controller::new()` call to pass engine and injection config. For now, create a placeholder engine:

```rust
use crate::config::Config;
use crate::engine::Engine;

// In run_with_paths():
let config = Config::load().context("Failed to load config")?;
let engine = Engine::new(config.clone()).context("Failed to create engine")?;
let controller = Arc::new(Controller::new(event_tx, shutdown_tx, engine, config.injection.clone()));
```

In `daemon/src/server_test.rs`, update to pass the new Controller args.

**Step 5: Update controller tests**

Update `create_controller` helper in `daemon/src/controller_test.rs`:

```rust
fn create_controller() -> (Controller, oneshot::Receiver<()>) {
    let (event_tx, _) = broadcast::channel(16);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let config = Config::default();
    let engine = Engine::new(config.clone()).unwrap();
    (
        Controller::new(event_tx, shutdown_tx, engine, config.injection),
        shutdown_rx,
    )
}
```

Add the necessary imports at top of test file:
```rust
use crate::config::Config;
use crate::engine::Engine;
use crate::config::InjectionConfig;
```

**Step 6: Run tests**

Run: `cargo test -p voice-controllm-daemon`
Expected: All tests pass

**Step 7: Commit**

```bash
git add daemon/src/controller.rs daemon/src/controller_test.rs daemon/src/daemon.rs daemon/src/server_test.rs
git commit -m "feat(daemon): wire Engine into Controller with spawn/cancel lifecycle"
```

---

### Task 6: Update daemon runner for engine initialization

**Files:**
- Modify: `daemon/src/daemon.rs`

**Step 1: Update daemon runner**

Replace `daemon/src/daemon.rs` with:

```rust
//! Daemon runner that orchestrates all components.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tonic::transport::Server;
use tracing::{error, info};
use voice_controllm_proto::{
    DaemonError, ErrorKind, Event, InitProgress, ModelLoad, Ready,
};

use crate::config::Config;
use crate::controller::Controller;
use crate::engine::{Engine, InitEvent};
use crate::models::{ModelId, ModelStatus};
use crate::server::VoiceControllmService;
use crate::socket::{cleanup_socket, create_listener};

/// Paths used by the daemon at runtime.
pub struct DaemonPaths {
    pub socket: PathBuf,
    pub pid: PathBuf,
}

impl DaemonPaths {
    pub fn from_xdg() -> Result<Self> {
        Ok(Self {
            socket: crate::socket::socket_path()?,
            pid: crate::socket::pid_path()?,
        })
    }
}

/// Run the daemon with default XDG paths.
pub async fn run() -> Result<()> {
    run_with_paths(DaemonPaths::from_xdg()?).await
}

/// Run the daemon with custom paths.
pub async fn run_with_paths(paths: DaemonPaths) -> Result<()> {
    let sock_path = paths.socket;
    let pid_file = paths.pid;

    // Load config
    let config = Config::load().context("Failed to load config")?;
    info!(model = ?config.model.model, "Loaded configuration");

    // Write PID file
    let pid = std::process::id();
    std::fs::write(&pid_file, pid.to_string()).context("Failed to write PID file")?;
    info!(pid = pid, path = %pid_file.display(), "Wrote PID file");

    // Create Unix socket listener
    let listener = create_listener(&sock_path)?;
    info!(path = %sock_path.display(), "Listening on Unix socket");

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Create event channel
    let (event_tx, _) = broadcast::channel(256);

    // Create engine
    let engine = Engine::new(config.clone()).context("Failed to create engine")?;

    // Create controller (starts in Initializing state)
    let controller = Arc::new(Controller::new(
        event_tx.clone(),
        shutdown_tx,
        engine,
        config.injection.clone(),
    ));

    // Create gRPC service
    let service = VoiceControllmService::new(controller.clone());

    // Convert UnixListener to stream
    let incoming = async_stream::stream! {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => yield Ok::<_, std::io::Error>(stream),
                Err(e) => {
                    tracing::error!(error = %e, "Accept error");
                }
            }
        }
    };

    // Spawn initialization task
    let init_controller = controller.clone();
    let init_event_tx = event_tx.clone();
    tokio::spawn(async move {
        initialize_engine(init_controller, init_event_tx).await;
    });

    // Run server with graceful shutdown
    info!("Daemon started");
    let server = Server::builder()
        .add_service(service.into_server())
        .serve_with_incoming_shutdown(incoming, async {
            let _ = shutdown_rx.await;
            info!("Shutdown signal received");
        });

    let result = server.await;

    // Cleanup
    cleanup_socket(&sock_path);
    let _ = std::fs::remove_file(&pid_file);
    info!("Daemon stopped");

    result.context("Server error")
}

/// Initialize the engine in a background task.
async fn initialize_engine(controller: Arc<Controller>, event_tx: broadcast::Sender<Event>) {
    let mut engine = match controller.take_engine().await {
        Some(e) => e,
        None => {
            error!("No engine available for initialization");
            return;
        }
    };

    let tx = event_tx.clone();
    let result = engine
        .initialize(move |event| {
            let proto_event = match event {
                InitEvent::Loading { model } => Event {
                    event: Some(voice_controllm_proto::event::Event::InitProgress(
                        InitProgress {
                            progress: Some(
                                voice_controllm_proto::init_progress::Progress::ModelLoad(
                                    ModelLoad { model_name: model },
                                ),
                            ),
                        },
                    )),
                },
                InitEvent::Downloading {
                    model,
                    bytes,
                    total,
                } => Event {
                    event: Some(voice_controllm_proto::event::Event::InitProgress(
                        InitProgress {
                            progress: Some(
                                voice_controllm_proto::init_progress::Progress::ModelDownload(
                                    voice_controllm_proto::ModelDownload {
                                        model_name: model,
                                        bytes_downloaded: bytes,
                                        bytes_total: total,
                                    },
                                ),
                            ),
                        },
                    )),
                },
                InitEvent::Ready => Event {
                    event: Some(voice_controllm_proto::event::Event::InitProgress(
                        InitProgress {
                            progress: Some(
                                voice_controllm_proto::init_progress::Progress::Ready(Ready {}),
                            ),
                        },
                    )),
                },
            };
            let _ = tx.send(proto_event);
        })
        .await;

    match result {
        Ok(()) => {
            controller.return_engine(engine).await;
            controller.mark_ready().await;
            info!("Engine initialization complete");
        }
        Err(e) => {
            error!(error = %e, "Engine initialization failed");
            controller.return_engine(engine).await;
            // Broadcast error event
            let error_event = Event {
                event: Some(voice_controllm_proto::event::Event::DaemonError(
                    DaemonError {
                        kind: ErrorKind::ErrorEngine.into(),
                        message: format!("{:#}", e),
                        model_name: String::new(),
                    },
                )),
            };
            let _ = event_tx.send(error_event);
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: SUCCESS

**Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add daemon/src/daemon.rs
git commit -m "feat(daemon): initialize engine at startup with progress events"
```

---

### Task 7: Implement DownloadModels RPC in gRPC server

**Files:**
- Modify: `daemon/src/server.rs`
- Modify: `daemon/src/server_test.rs`

**Step 1: Add DownloadModels to gRPC service impl**

In `daemon/src/server.rs`, add the method to the `VoiceControllm` impl:

```rust
async fn download_models(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
    // Re-trigger engine initialization
    let controller = self.controller.clone();
    tokio::spawn(async move {
        // Take engine, re-initialize, return
        if let Some(mut engine) = controller.take_engine().await {
            let result = engine.initialize(|_| {}).await;
            controller.return_engine(engine).await;
            match result {
                Ok(()) => controller.mark_ready().await,
                Err(e) => {
                    tracing::error!(error = %e, "Model re-download failed");
                }
            }
        }
    });
    Ok(Response::new(Empty {}))
}
```

Also update the `get_status` match to include Initializing (if not already done in Task 1).

**Step 2: Verify it compiles**

Run: `cargo build`
Expected: SUCCESS

**Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add daemon/src/server.rs daemon/src/server_test.rs
git commit -m "feat(daemon): implement DownloadModels gRPC endpoint"
```

---

### Task 8: Update CLI to show progress during startup

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/src/client.rs`

**Step 1: Add subscribe_events to client**

In `cli/src/client.rs`, add:

```rust
use voice_controllm_proto::Event;
use tokio_stream::Stream;
use std::pin::Pin;

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
```

**Step 2: Rewrite cmd_start to show progress**

Replace `cmd_start()` in `cli/src/main.rs`:

```rust
async fn cmd_start() -> Result<()> {
    let sock_path = socket_path()?;

    if client::is_daemon_running(&sock_path).await {
        let pid_path = voice_controllm_daemon::socket::pid_path()?;
        let pid = std::fs::read_to_string(&pid_path).unwrap_or_else(|_| "unknown".to_string());
        println!("Daemon already running (PID: {})", pid.trim());
        return Ok(());
    }

    // Spawn daemon as detached process
    let daemon_path = std::env::current_exe()?
        .parent()
        .context("No parent directory")?
        .join("voice-controllm-daemon");

    if !daemon_path.exists() {
        anyhow::bail!("Daemon binary not found at: {}", daemon_path.display());
    }

    println!("Starting daemon...");

    std::process::Command::new(&daemon_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn daemon")?;

    // Wait for socket to appear (up to 5 seconds)
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if client::is_daemon_running(&sock_path).await {
            break;
        }
    }

    if !client::is_daemon_running(&sock_path).await {
        anyhow::bail!("Daemon failed to start");
    }

    // Subscribe to events and show initialization progress
    let mut grpc_client = client::connect(&sock_path).await?;

    // Check if already ready
    let status = grpc_client
        .get_status(Empty {})
        .await
        .context("Failed to get status")?
        .into_inner();

    if let Some(StatusVariant::Healthy(h)) = status.status {
        let state = State::try_from(h.state).unwrap_or(State::Stopped);
        if state != State::Initializing {
            let pid_path = voice_controllm_daemon::socket::pid_path()?;
            let pid =
                std::fs::read_to_string(&pid_path).unwrap_or_else(|_| "unknown".to_string());
            println!("Daemon ready (PID: {})", pid.trim());
            return Ok(());
        }
    }

    // Stream events until Ready or error
    let mut stream = client::subscribe(&mut grpc_client).await?;

    use voice_controllm_proto::event::Event as EventType;
    use voice_controllm_proto::init_progress::Progress;

    while let Some(event) = stream.message().await? {
        match event.event {
            Some(EventType::InitProgress(progress)) => match progress.progress {
                Some(Progress::ModelDownload(dl)) => {
                    let mb_done = dl.bytes_downloaded as f64 / 1_000_000.0;
                    let mb_total = dl.bytes_total as f64 / 1_000_000.0;
                    if mb_total > 0.0 {
                        print!(
                            "\rDownloading {}... {:.0}/{:.0} MB",
                            dl.model_name, mb_done, mb_total
                        );
                    } else {
                        print!("\rDownloading {}... {:.0} MB", dl.model_name, mb_done);
                    }
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                Some(Progress::ModelLoad(load)) => {
                    println!("Loading {}...", load.model_name);
                }
                Some(Progress::Ready(_)) => {
                    let pid_path = voice_controllm_daemon::socket::pid_path()?;
                    let pid = std::fs::read_to_string(&pid_path)
                        .unwrap_or_else(|_| "unknown".to_string());
                    println!("Daemon ready (PID: {})", pid.trim());
                    return Ok(());
                }
                None => {}
            },
            Some(EventType::DaemonError(err)) => {
                let kind = voice_controllm_proto::ErrorKind::try_from(err.kind)
                    .unwrap_or(voice_controllm_proto::ErrorKind::ErrorUnknown);
                match kind {
                    voice_controllm_proto::ErrorKind::ErrorModelMissing => {
                        println!(
                            "Model '{}' not found. Download it? [Y/n] ",
                            err.model_name
                        );
                        // For now, auto-accept (interactive prompts come later)
                        grpc_client
                            .download_models(Empty {})
                            .await
                            .context("Failed to trigger model download")?;
                        println!("Downloading...");
                    }
                    voice_controllm_proto::ErrorKind::ErrorModelCorrupted => {
                        println!(
                            "Model '{}' appears corrupted: {}. Re-download? [Y/n] ",
                            err.model_name, err.message
                        );
                        grpc_client
                            .download_models(Empty {})
                            .await
                            .context("Failed to trigger model re-download")?;
                        println!("Re-downloading...");
                    }
                    _ => {
                        eprintln!("Daemon error: {}", err.message);
                        std::process::exit(1);
                    }
                }
            }
            Some(EventType::StateChange(_)) => {
                // Ignore state changes during startup
            }
            Some(EventType::Transcription(_)) | None => {}
        }
    }

    // Stream ended without Ready
    let pid_path = voice_controllm_daemon::socket::pid_path()?;
    let pid = std::fs::read_to_string(&pid_path).unwrap_or_else(|_| "unknown".to_string());
    println!("Daemon started (PID: {})", pid.trim());
    Ok(())
}
```

Add `tokio-stream` dependency to CLI if not already present:

Run: `cargo add tokio-stream -p vcm`

**Step 3: Verify it compiles**

Run: `cargo build -p vcm`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add cli/src/main.rs cli/src/client.rs cli/Cargo.toml Cargo.lock
git commit -m "feat(cli): show initialization progress during vcm start"
```

---

### Task 9: Manual end-to-end verification

This task cannot be TDD'd — it requires real hardware (microphone).

**Step 1: Build everything**

Run: `cargo build`
Expected: SUCCESS

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Fix any warnings.

**Step 3: Run all tests**

Run: `cargo test`
Expected: All pass

**Step 4: Format**

Run: `cargo fmt`

**Step 5: Manual test (requires microphone)**

```bash
# Terminal 1: Start daemon with logging
RUST_LOG=info cargo run -p voice-controllm-daemon

# Terminal 2: Use CLI
cargo run -p vcm -- status     # Should show "Initializing..." then "Paused"
cargo run -p vcm -- toggle     # Should show "Listening"
# Speak into microphone — text should be injected as keystrokes
cargo run -p vcm -- toggle     # Should show "Paused"
cargo run -p vcm -- stop       # Should show "Daemon stopped"
```

Alternatively, test the full flow:
```bash
cargo run -p vcm -- start      # Should show download/loading progress, then "Daemon ready"
cargo run -p vcm -- toggle     # Start listening
cargo run -p vcm -- stop       # Stop
```

**Step 6: Commit any fixes**

```bash
git add -A
git commit -m "fix(daemon): address clippy warnings and formatting"
```

---

### Task 10: Update roadmap

**Files:**
- Modify: `docs/plans/project-roadmap.md`

**Step 1: Update Phase 2 status and add Phase 2.5**

Add a new section between Phase 2 and Phase 3 in the roadmap:

```markdown
### Phase 2.5: Controller-Engine Integration ✅ Complete

Connect the daemon's control layer to the audio pipeline.

| Component | Status | Description |
|-----------|--------|-------------|
| Engine refactor | ✅ Done | Split run() into initialize() + run_loop() |
| Controller integration | ✅ Done | Controller spawns/cancels engine tasks |
| Init progress events | ✅ Done | Proto events for model download/load/ready |
| CLI progress display | ✅ Done | vcm start shows initialization progress |
| Model integrity check | ✅ Done | Detect missing vs corrupted models |
| DownloadModels RPC | ✅ Done | Re-download models on demand |
```

Update "Current Focus" at the bottom to point to Phase 3.

**Step 2: Commit**

```bash
git add docs/plans/project-roadmap.md
git commit -m "docs: update roadmap with Phase 2.5 integration"
```

---

Plan complete and saved to `docs/plans/2026-02-07-integration-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
