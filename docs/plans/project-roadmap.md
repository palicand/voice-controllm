# Voice-Controllm Project Roadmap

Offline voice dictation utility for macOS accessibility.

## Design Goals

- **Continuous listening** - always on, toggle via menu bar
- **Offline-first** - all processing local, no internet required
- **Accessibility-focused** - designed for users who cannot easily use keyboard

## Architecture Overview

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Menu Bar   │────▶│    Daemon    │◀────│     CLI      │
│    (Tauri)   │     │   (Rust)     │     │    (vcm)     │
└──────────────┘     └──────────────┘     └──────────────┘
                            │
                ┌───────────┼───────────┐
                ▼           ▼           ▼
           ┌────────┐  ┌────────┐  ┌────────┐
           │ Audio  │  │  VAD   │  │Whisper │
           │Capture │  │(Silero)│  │(CoreML)│
           └────────┘  └────────┘  └────────┘
                                        │
                                        ▼
                                  ┌──────────┐
                                  │Keystroke │
                                  │Injection │
                                  └──────────┘
```

## Phases

### Phase 1: Core Engine ✅ Complete

Build the transcription pipeline.

| Component | Status | Description |
|-----------|--------|-------------|
| Audio capture | ✅ Done | cpal-based mic capture with resampling |
| VAD | ✅ Done | Silero ONNX model for speech detection |
| Model manager | ✅ Done | Auto-download GGML + CoreML models from HF |
| Transcriber trait | ✅ Done | Abstraction for speech-to-text backends |
| Whisper backend | ✅ Done | whisper-rs with CoreML acceleration |
| Engine | ✅ Done | Coordinator: capture → VAD → transcribe |
| Keystroke injection | ✅ Done | enigo-based text injection with allowlist |

### Phase 2: IPC & CLI ✅ Complete

Daemon/CLI communication and process management.

| Component | Status | Description |
|-----------|--------|-------------|
| gRPC definitions | ✅ Done | proto/ with service definitions |
| Daemon server | ✅ Done | tonic gRPC server on Unix socket |
| CLI client | ✅ Done | CLI connects to daemon via gRPC |
| `vcm start` | ✅ Done | Spawn daemon as background process |
| `vcm stop` | ✅ Done | Send shutdown signal to daemon |
| `vcm status` | ✅ Done | Query daemon state |
| `vcm toggle` | ✅ Done | Quick on/off for listening |
| `vcm test-mic` | ⬜ Deferred | Test microphone input (Phase 4) |
| `vcm transcribe <file>` | ⬜ Deferred | Transcribe audio file (Phase 4) |

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

### Phase 3: Menu Bar App

System tray application for easy control.

| Component | Status | Description |
|-----------|--------|-------------|
| Tauri setup | ⬜ Todo | menubar/ crate with Tauri |
| Status indicator | ⬜ Todo | Icon: green=listening, gray=off, red=error |
| Toggle listening | ⬜ Todo | Click to turn on/off |
| Settings access | ⬜ Todo | Open config or preferences |
| Quit | ⬜ Todo | Exit daemon and app |
| Daemon lifecycle | ⬜ Todo | Menu bar manages daemon process |

### Phase 4: Polish & Distribution

Production readiness.

| Component | Status | Description |
|-----------|--------|-------------|
| launchd integration | ⬜ Todo | `vcm install` to run on login |
| DMG packaging | ⬜ Todo | Distributable macOS installer |
| Code signing | ⬜ Todo | Sign for Gatekeeper |
| Accessibility permissions | ⬜ Todo | Guide user through granting permissions |
| Error recovery | ⬜ Todo | Handle mic disconnect, model errors gracefully |
| Model integrity hashes | ⬜ Todo | SHA256 verification for downloaded models (currently size-only check) |
| Streaming transcription | ⬜ Todo | Real-time partial results for lower perceived latency |

## CLI Commands

```
vcm start              # Start daemon in background
vcm stop               # Stop daemon
vcm status             # Show daemon state (listening/paused/stopped)
vcm toggle             # Toggle listening on/off

vcm test-mic           # Test microphone input (debug)
vcm transcribe <file>  # Transcribe audio file (debug)

vcm install            # Install launchd service (Phase 4)
vcm uninstall          # Remove launchd service (Phase 4)
```

## Menu Bar

```
┌─────────────────────┐
│ ● Voice-Controllm   │  ← Green dot = listening
├─────────────────────┤
│ ○ Pause Listening   │  ← Toggle
│ ─────────────────── │
│   Settings...       │  ← Opens config
│   Quit              │
└─────────────────────┘
```

## Configuration

Location: `~/.config/voice-controllm/config.toml`

```toml
[model]
model = "canary-1b"
languages = ["en", "de", "cs"]

[latency]
mode = "balanced"
min_chunk_seconds = 0.5

[injection]
allowlist = []  # empty = inject everywhere
```

## Models

Location: `~/.local/share/voice-controllm/models/`

| Model | Size | Purpose |
|-------|------|---------|
| silero_vad.onnx | ~2MB | Voice activity detection |
| ggml-*.bin | 75MB-3GB | Whisper GGML model |
| ggml-*-encoder.mlmodelc | 50MB-1.2GB | CoreML encoder (macOS) |

Models auto-download on first run from Hugging Face.

## Current Focus

**Phase 3: Menu Bar App** - Building Tauri system tray application.

Phase 2.5 complete: Controller-Engine integration wired up with progress events and CLI display.
