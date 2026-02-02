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
           │ Audio  │  │  VAD   │  │Canary  │
           │Capture │  │(Silero)│  │ (NeMo) │
           └────────┘  └────────┘  └────────┘
                                        │
                                        ▼
                                  ┌──────────┐
                                  │Keystroke │
                                  │Injection │
                                  └──────────┘
```

## Phases

### Phase 1: Core Engine ✅ (In Progress)

Build the transcription pipeline.

| Component | Status | Description |
|-----------|--------|-------------|
| Audio capture | ✅ Done | cpal-based mic capture with resampling |
| VAD | ✅ Done | Silero ONNX model for speech detection |
| Model manager | ⬜ Todo | Auto-download models on first run |
| Transcriber trait | ⬜ Todo | Abstraction for speech-to-text backends |
| Canary backend | ⬜ Todo | NVIDIA NeMo via ONNX Runtime |
| Engine | ⬜ Todo | Coordinator: capture → VAD → transcribe |
| Keystroke injection | ⬜ Todo | enigo-based text injection |

### Phase 2: IPC & CLI

Daemon/CLI communication and process management.

| Component | Status | Description |
|-----------|--------|-------------|
| gRPC definitions | ⬜ Todo | proto/ with service definitions |
| Daemon server | ⬜ Todo | tonic gRPC server in daemon |
| CLI client | ⬜ Todo | CLI connects to daemon via gRPC |
| `vcm start` | ⬜ Todo | Spawn daemon as background process |
| `vcm stop` | ⬜ Todo | Send shutdown signal to daemon |
| `vcm status` | ⬜ Todo | Query daemon state |
| `vcm toggle` | ⬜ Todo | Quick on/off for listening |
| `vcm test-mic` | ⬜ Todo | Test microphone input |
| `vcm transcribe <file>` | ⬜ Todo | Transcribe audio file (debug) |

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
| canary-1b.onnx | ~1GB | Speech-to-text (Canary) |

Models auto-download on first run.

## Current Focus

**Phase 1 completion** - see `2025-02-02-transcription-plan.md` for detailed tasks.

After Phase 1, we'll have a working transcription pipeline. Phase 2 adds the CLI/daemon split, Phase 3 adds the menu bar for accessibility.
