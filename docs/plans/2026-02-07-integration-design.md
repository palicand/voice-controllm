# Phase 2.5: Controller-Engine Integration Design

Connect the daemon's Controller to the Engine so that `vcm start` / `vcm toggle` actually captures audio, transcribes speech, and injects keystrokes.

## Problem

All individual components work (proven by `daemon/examples/inject_test.rs`), but the Controller is a state machine that flips flags without ever creating or running the Engine. The CLI communicates with the daemon via gRPC, the daemon accepts commands, but nothing happens.

## Scope

**In scope:**
- Refactor Engine to separate initialization from the audio loop
- Wire Controller to own and manage the Engine lifecycle
- Extend proto events for startup progress (model download, loading, ready)
- Add `DownloadModels` RPC for re-download on missing/corrupt models
- CLI shows progress during `vcm start` via event subscription
- Model integrity checking (missing vs corrupted, distinct user messages)

**Out of scope:**
- Real-time/streaming transcription (future — proto will be ready for it)
- Wake-word / soft-pause mode (future — architecture won't preclude it)
- Menu bar app (Phase 3)

## Key Insight: Engine Refactor

Currently `Engine::run()` does everything in one method:
1. Download models
2. Initialize VAD, Whisper, audio capture, resampler
3. Run the audio loop

This needs to split into two phases so that model loading happens at daemon startup (eager) while the audio loop starts/stops on toggle:

```rust
impl Engine {
    /// Phase 1: Download and load models. Emits progress events.
    pub async fn initialize(&mut self, event_tx: &EventSender) -> Result<()>;

    /// Phase 2: Run the audio capture loop. Blocks until cancelled.
    pub async fn run_loop<F>(&mut self, cancel: CancellationToken, on_transcription: F) -> Result<()>
    where F: FnMut(&str);
}
```

The `initialize()` method replaces the model download + component init from `run()`.
The `run_loop()` method handles only audio capture → VAD → transcribe, using the already-initialized components.

## Architecture

```
vcm start
    │
    ▼
┌──────────┐  spawn daemon   ┌──────────────────────────────────────┐
│   CLI    │ ──────────────► │              Daemon                   │
│          │  subscribe      │                                      │
│          │ ◄──────────────►│  1. Load config                      │
│          │  events         │  2. Create Engine                    │
│  show    │                 │  3. engine.initialize()              │
│  progress│                 │     → download models (events)       │
│          │                 │     → load models (events)           │
│          │  ◄── Ready ──── │  4. Create Controller(engine)        │
│  "Done!" │                 │  5. Start gRPC server                │
└──────────┘                 └──────────────────────────────────────┘

vcm toggle
    │
    ▼
┌──────────┐  StartListening  ┌────────────┐  spawn task  ┌────────┐
│   CLI    │ ───────────────► │ Controller │ ───────────► │ Engine │
└──────────┘                  └────────────┘              │run_loop│
                                                          └───┬────┘
                                    ┌─────────────────────────┤
                                    ▼          ▼              ▼
                                  Audio      VAD ──► Whisper ──► Injector
                                  Capture              │
                                                       ▼
                                              TranscriptionFinal event
```

## Proto Changes

### New Event Types

```protobuf
message Event {
  oneof event {
    StateChange state_change = 1;
    Transcription transcription = 2;  // rename to TranscriptionFinal
    InitProgress init_progress = 3;
    DaemonError error = 4;
    TranscriptionPartial partial_transcription = 5;  // reserved for future
  }
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
  string model_name = 3;  // populated for model errors
}

enum ErrorKind {
  ERROR_UNKNOWN = 0;
  ERROR_MODEL_MISSING = 1;
  ERROR_MODEL_CORRUPTED = 2;
  ERROR_MIC_ACCESS_DENIED = 3;
  ERROR_ENGINE = 4;
}
```

### New RPC

```protobuf
service VoiceControllm {
  // ... existing RPCs ...
  rpc DownloadModels(Empty) returns (Empty);  // trigger model re-download
}
```

## Controller Changes

The Controller gains ownership of the Engine and manages its lifecycle:

```rust
pub struct Controller {
    state: Arc<RwLock<ControllerState>>,
    event_tx: EventSender,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    // New fields:
    engine: Arc<Mutex<Engine>>,
    engine_cancel: Arc<RwLock<Option<CancellationToken>>>,
    injector_config: InjectionConfig,
}
```

**start_listening:**
1. Check state is Paused
2. Create a `CancellationToken`
3. Spawn a tokio task that runs `engine.run_loop()` with a callback that:
   - Invokes `KeystrokeInjector::inject_text()`
   - Emits `TranscriptionFinal` events via broadcast
4. Store the cancel token
5. Transition to Listening

**stop_listening:**
1. Check state is Listening
2. Cancel the token (signals `run_loop` to exit)
3. Await the task to finish (audio stream released)
4. Transition to Paused

**shutdown:**
1. Stop listening if active
2. Send shutdown signal (existing behavior)

## Daemon Runner Changes

The `daemon::run()` function changes to:

1. Load config
2. Create Engine
3. Call `engine.initialize(event_tx)` — downloads/loads models, emits progress events
4. Create `KeystrokeInjector`
5. Create Controller with engine + injector config
6. Start gRPC server
7. Wait for shutdown

If `initialize()` hits a missing/corrupt model, it emits the appropriate error event and returns an error. The daemon stays alive so the CLI can send a `DownloadModels` RPC.

## CLI Changes

**`vcm start`:**
1. Spawn daemon process (existing)
2. Wait for socket (existing, but shorter timeout)
3. Subscribe to event stream
4. Display progress:
   - `ModelDownload` → "Downloading whisper-base... 45MB/150MB"
   - `ModelLoad` → "Loading whisper-base..."
   - `Ready` → "Daemon ready" (exit)
   - `ErrorModelMissing` → "Model whisper-base not found. Download? [Y/n]"
   - `ErrorModelCorrupted` → "Model whisper-base appears corrupted. Re-download? [Y/n]"
5. On user confirmation → send `DownloadModels` RPC, continue showing progress

## Model Integrity

Add verification to `ModelManager`:

```rust
pub enum ModelStatus {
    Ready(PathBuf),
    Missing,
    Corrupted(PathBuf),  // exists but failed validation
}

impl ModelManager {
    pub async fn check_model(&self, model: ModelId) -> ModelStatus;
}
```

Validation: check file exists, file size matches expected, and for ONNX models attempt a quick load. Corrupted = file exists but doesn't pass validation.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Mic access denied | `start_listening` returns error, emits `ERROR_MIC_ACCESS_DENIED` event, stays Paused |
| Model missing at init | Emits `ERROR_MODEL_MISSING` event, daemon stays alive awaiting `DownloadModels` RPC |
| Model corrupted at init | Emits `ERROR_MODEL_CORRUPTED` event, daemon stays alive awaiting `DownloadModels` RPC |
| Toggle while already in state | Idempotent no-op, returns Ok |
| Shutdown while listening | Stops audio first, then shuts down |
| Engine task panics | Controller catches it, transitions to Paused, emits `ERROR_ENGINE` event |

## Implementation Order

1. **Engine refactor** — Split `run()` into `initialize()` + `run_loop()`
2. **Proto changes** — Add new event types, `DownloadModels` RPC, `ErrorKind`
3. **Model integrity** — Add `check_model()` to `ModelManager`
4. **Controller integration** — Wire Engine into Controller with spawn/cancel
5. **Daemon runner** — Initialize engine at startup, pass to Controller
6. **gRPC server** — Implement `DownloadModels`, emit new events
7. **CLI progress** — Subscribe to events during `vcm start`, show progress, handle model errors
8. **Tests** — Integration tests for the connected pipeline

## Testing

- **Unit**: Controller state transitions with mock engine
- **Unit**: Model integrity check (missing, corrupted, valid)
- **Integration**: Daemon startup → CLI subscribe → progress events → ready
- **Integration**: Toggle listening → verify audio starts/stops
- **Manual**: Full end-to-end with real microphone and transcription
