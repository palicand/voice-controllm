# Voice-Controllm Implementation Plan

## Summary

Offline dictation utility with keyword detection for accessibility. Replaces Apple Voice Control with a Rust daemon + CLI + menu bar widget architecture (macOS only for now).

## Development Approach: Test-Driven Development (TDD)

For each phase:
1. **Design** - Analyze requirements, define interfaces and behavior
2. **Test** - Write tests that specify expected behavior
3. **Implement** - Write code to make tests pass
4. **Refactor** - Clean up while keeping tests green

---

## Implementation Phases

### Phase 0: Project Foundation ✅
- [x] Create Rust workspace structure
- [x] Create `README.md` with project overview
- [x] Create `CLAUDE.md` with project context
- [x] Set up GitHub Actions CI (build + clippy + test + fmt)
- [x] Initial commit with all scaffolding

### Phase 1: Core Engine (MVP)

#### 1.1 Configuration Management ✅
**Design**: Load/save TOML config, provide defaults, handle missing files
**Tests first**:
- [x] Test default config values
- [x] Test loading valid config from file
- [x] Test missing config file returns defaults
- [x] Test invalid TOML returns error
- [x] Test config paths (data dir, models dir)
**Implementation**:
- [x] `config.rs` with Config, ModelConfig, LatencyConfig, InjectionConfig structs

#### 1.2 Audio Capture
**Design**: Capture audio from default input device, resample to 16kHz mono
**Tests first**:
- [ ] [Unit] Test audio buffer creation and management
- [ ] [Unit] Test resampling logic with synthetic audio data
- [ ] [Unit] Test audio format conversion (stereo to mono, sample rates)
- [ ] [Integration] Test resampling produces correct output with fixture audio
- [ ] [Hardware] Test capture from real microphone device
**Implementation**:
- [ ] `audio.rs` with AudioCapture struct using `cpal`
- [ ] Resampling with `rubato`

#### 1.3 Voice Activity Detection (VAD)
**Design**: Detect speech segments using Silero VAD ONNX model
**Tests first**:
- [ ] [Unit] Test VAD state machine (speech start/end detection)
- [ ] [Unit] Test silence detection threshold logic
- [ ] [Unit] Test minimum speech duration filtering
- [ ] [Integration] Test VAD with real Silero model on fixture audio
- [ ] [Integration] Test VAD accuracy: speech vs silence classification
**Implementation**:
- [ ] `vad.rs` with VoiceActivityDetector using `ort`

#### 1.4 Speech Transcription
**Design**: Transcribe audio using Whisper with CoreML backend
**Tests first**:
- [ ] [Unit] Test transcription result structure
- [ ] [Unit] Test language config parsing
- [ ] [Unit] Test empty/invalid audio handling
- [ ] [Integration] Test transcription with real Whisper model
- [ ] [Integration] Test transcription accuracy on fixture phrases
**Implementation**:
- [ ] `transcribe.rs` with SpeechRecognizer trait and WhisperRecognizer

#### 1.5 Keystroke Injection
**Design**: Inject transcribed text as keystrokes
**Tests first**:
- [ ] [Unit] Test text-to-keystroke sequence conversion
- [ ] [Unit] Test special character handling (unicode, punctuation)
- [ ] [Unit] Test injection mode filtering logic (allowlist matching)
- [ ] [Hardware] Test injection into focused application
**Implementation**:
- [ ] `inject.rs` with KeystrokeInjector using `enigo`

#### 1.6 Model Download
**Design**: Download models on first run with progress indication
**Tests first**:
- [ ] Test model path resolution
- [ ] Test download progress callback
- [ ] Test existing model detection (skip download)
- [ ] Test download failure handling
**Implementation**:
- [ ] `models.rs` with ModelManager

#### 1.7 Pipeline Integration
**Design**: Connect audio → VAD → transcription → injection
**Tests first**:
- [ ] Test pipeline state transitions
- [ ] Test start/stop lifecycle
- [ ] Test error propagation
**Implementation**:
- [ ] `pipeline.rs` orchestrating all components
- [ ] Update CLI to use pipeline directly

### Phase 2: IPC Layer

#### 2.1 gRPC API Definition
**Design**: Define service for daemon control and status
**Tests first**:
- [ ] Test protobuf message serialization
- [ ] Test request/response types
**Implementation**:
- [ ] `proto/voice_controllm.proto` with Start, Stop, Status, GetConfig, SetConfig RPCs

#### 2.2 Daemon Server
**Design**: Unix socket gRPC server for daemon
**Tests first**:
- [ ] Test server startup/shutdown
- [ ] Test concurrent client connections
- [ ] Test RPC handlers
**Implementation**:
- [ ] `grpc.rs` with tonic server implementation

#### 2.3 CLI Client
**Design**: Refactor CLI to communicate via gRPC
**Tests first**:
- [ ] Test client connection handling
- [ ] Test command execution via RPC
- [ ] Test timeout and error handling
**Implementation**:
- [ ] Update `cli/src/main.rs` to use gRPC client

#### 2.4 Keyword Detection
**Design**: Match transcripts against configurable patterns
**Tests first**:
- [ ] Test exact keyword matching
- [ ] Test pattern matching (regex)
- [ ] Test keyword action mapping
**Implementation**:
- [ ] `keywords.rs` with KeywordDetector

#### 2.5 Configuration Management
**Design**: Runtime config updates via gRPC
**Tests first**:
- [ ] Test config reload
- [ ] Test config validation
- [ ] Test config persistence
**Implementation**:
- [ ] Extend config.rs with runtime updates

### Phase 3: Menu Bar App

#### 3.1 Tauri Project Setup
**Tests first**:
- [ ] Test gRPC client integration
- [ ] Test state management
**Implementation**:
- [ ] Create Tauri v2 project in `menubar/`
- [ ] Configure system tray with LSUIElement=true

#### 3.2 Status Indicator
**Tests first**:
- [ ] Test icon state changes
- [ ] Test status polling
**Implementation**:
- [ ] Tray icon reflecting daemon state (listening/stopped/error)

#### 3.3 Quick Settings
**Tests first**:
- [ ] Test menu item actions
- [ ] Test config updates from UI
**Implementation**:
- [ ] Dropdown menu with toggle, language selection, etc.

### Future: Enhancements
- [ ] Emoji mapping (spoken → emoji)
- [ ] Punctuation voice commands ("period", "comma")
- [ ] Markdown mode (experimental)
- [ ] User vocabulary customization
- [ ] Cross-platform support (Windows/Linux)

---

## Architecture

```
┌─────────────────┐     Unix Socket/gRPC     ┌──────────────────────┐
│   CLI Utility   │◄───────────────────────►│       Daemon         │
└─────────────────┘                          │                      │
                                             │  Audio (cpal)        │
┌─────────────────┐     Unix Socket/gRPC     │       ↓              │
│  Menu Bar App   │◄───────────────────────►│  VAD (Silero)        │
│  (Tauri)        │                          │       ↓              │
└─────────────────┘                          │  Whisper (CoreML)    │
                                             │       ↓              │
                                             │  Post-Processing     │
                                             │       ↓              │
                                             │  Keystroke Injection │
                                             └──────────────────────┘
```

## Rust Crate Stack

| Purpose | Crate |
|---------|-------|
| Audio capture | `cpal` |
| Resampling | `rubato` |
| Whisper inference | `whisper-rs` |
| VAD | `ort` (Silero ONNX) |
| gRPC IPC | `tonic` + `prost` |
| Keystroke injection | `enigo` |
| Menu bar | `tauri` v2 (system tray) |
| Async runtime | `tokio` |
| Config | `serde` + `toml` |

## Configuration

Config file: `~/.config/voice-controllm/config.toml`

```toml
[model]
name = "large-v3-turbo"  # or custom path
languages = ["auto"]     # e.g., ["cs", "en", "de"] for multilingual

[latency]
mode = "balanced"        # "fast" | "balanced" | "accurate"
min_chunk_seconds = 1.0

[injection]
mode = "always"          # or "allowlist"
# allowlist = ["kitty", "alacritty", "IntelliJ IDEA"]
```

## Testing Strategy

### Test Categories

| Category | Runs in CI | Marker | Description |
|----------|------------|--------|-------------|
| **Unit** | ✅ Always | (none) | Pure logic, mocked dependencies |
| **Integration** | ✅ With setup | `#[cfg(feature = "integration")]` | Requires models/fixtures, CI downloads them |
| **Hardware** | ❌ Manual | `#[ignore]` | Requires microphone, accessibility permissions |

### Unit Tests
- Pure logic tests with no external dependencies
- Mock audio data, mock model responses
- Run with `cargo test`

### Integration Tests
- Use real models (Silero VAD, Whisper) with test audio fixtures
- CI workflow downloads models before running tests
- Run with `cargo test --features integration`
- Examples: VAD accuracy on test audio, transcription quality

### Hardware Tests
- Require physical microphone or accessibility permissions
- Marked with `#[ignore]` - only run manually
- Run with `cargo test -- --ignored`
- Future: explore virtual audio devices for CI

### Test Fixtures
- `tests/fixtures/` - test audio files (WAV, 16kHz mono)
- `tests/fixtures/models/` - gitignored, downloaded by CI/setup script
- Consider: record test phrases, generate synthetic audio

### Test Coverage Goals
- Config management: 100%
- Audio processing logic: 90%+ (mock/fixture audio)
- VAD state machine: 100%
- Transcription wrapper: 80%+ (integration tests with real model)
- Pipeline orchestration: 90%+
- gRPC handlers: 90%+

---

## Verification Plan

1. **Phase 0**: CI passes on empty workspace ✅
2. **Phase 1**: All unit tests pass, integration test: speak → keystrokes in TextEdit
3. **Phase 2**: CLI start/stop via gRPC, keyword detection works
4. **Phase 3**: Menu bar shows status, toggle via click
5. **End-to-end**: Dictate in JetBrains IDE / kitty
