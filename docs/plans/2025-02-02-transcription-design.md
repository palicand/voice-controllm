# Transcription Module Design

## Overview

Add speech-to-text transcription using NVIDIA Canary (NeMo) via ONNX Runtime. Includes a trait abstraction for future backend flexibility and an event-driven engine that coordinates audio capture, VAD, and transcription.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                        Engine                            │
│  ┌───────────┐   ┌───────────┐   ┌──────────────────┐  │
│  │  Audio    │──▶│    VAD    │──▶│   Transcriber    │  │
│  │  Capture  │   │           │   │   (Canary)       │  │
│  └───────────┘   └───────────┘   └──────────────────┘  │
│        │              │                   │             │
│        ▼              ▼                   ▼             │
│   [Resampler]   [SpeechStart/End]   [Text Result]      │
│        │              │                   │             │
│        └──────▶ [Speech Buffer] ◀────────┘             │
└─────────────────────────────────────────────────────────┘
                        │
                        ▼
                  on_transcription(text)
```

## Module Structure

```
daemon/src/
├── models.rs          # Model download/management
├── transcribe/
│   ├── mod.rs         # Transcriber trait
│   └── canary.rs      # Canary ONNX implementation
├── engine.rs          # Coordinator
├── audio.rs           # (existing)
├── vad.rs             # (existing)
└── config.rs          # (existing)
```

## Components

### Model Manager (`models.rs`)

Handles automatic model download on first run.

```rust
pub struct ModelManager {
    models_dir: PathBuf,  // ~/.local/share/voice-controllm/models/
}

pub enum ModelId {
    SileroVad,
    Canary1b,
}

impl ModelManager {
    pub fn ensure_model(&self, model: ModelId) -> Result<PathBuf>;
}
```

Each `ModelId` knows its download URL, filename, and optional checksum. Downloads show progress via tracing.

### Transcriber Trait (`transcribe/mod.rs`)

```rust
pub trait Transcriber: Send {
    fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<String>;
}
```

Simple batch API. Streaming can be added later via a separate trait or extended API.

### Canary Implementation (`transcribe/canary.rs`)

```rust
pub struct CanaryTranscriber {
    session: ort::Session,
    languages: Vec<String>,
}

impl CanaryTranscriber {
    pub fn new(model_path: &Path, languages: Vec<String>) -> Result<Self>;
}

impl Transcriber for CanaryTranscriber { ... }
```

Uses ONNX Runtime (already a dependency). Expects 16kHz audio; resamples internally if needed. Languages configured at initialization.

### Engine (`engine.rs`)

Coordinates the pipeline:

```rust
pub struct Engine {
    capture: AudioCapture,
    resampler: AudioResampler,
    vad: VoiceActivityDetector,
    transcriber: Box<dyn Transcriber>,
    speech_buffer: Vec<f32>,
}

impl Engine {
    pub fn new(config: &Config) -> Result<Self>;
    pub fn run<F>(&mut self, on_transcription: F) -> Result<()>
    where F: FnMut(&str);
}
```

Run loop:
1. Receive audio from capture
2. Resample to 16kHz
3. Feed chunks to VAD
4. On `SpeechStart` → start buffering
5. While speaking → append to buffer
6. On `SpeechEnd` → transcribe buffer → invoke callback → clear buffer

Errors are logged; engine continues listening.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Backend | Canary (NVIDIA NeMo) | Better multilingual + English support |
| Model loading | Auto-download on first run | Good UX, matches VAD approach |
| Architecture | Event-driven coordinator | Clean separation, supports future streaming |
| Initial mode | Transcribe on SpeechEnd | Simpler for POC, streaming later |
| Language config | At initialization | Keeps API simple, restart to change |
| Error handling | Propagate to coordinator | Coordinator logs and continues |

## Deferred

- Streaming transcription (word-by-word as you speak)
- Whisper backend
- Language switching without restart
- Keystroke injection (next module after this)

## Testing

- Unit tests: Mock transcriber for engine tests
- Integration tests: Canary with bundled audio file (like VAD tests)
