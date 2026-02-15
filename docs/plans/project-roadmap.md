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
│  (tray-icon) │     │   (Rust)     │     │    (vcm)     │
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

### Phase 3: Menu Bar App ✅ Complete

System tray application using tray-icon/muda/tao (lightweight, cross-platform).

| Component | Status | Description |
|-----------|--------|-------------|
| Crate scaffold | ✅ Done | menubar/ crate with tao event loop |
| Icon assets | ✅ Done | Lucide mic icons with colored state dots |
| AppState types | ✅ Done | State enum with proto conversion + menu helpers |
| Tray icon + menu | ✅ Done | Dynamic menu rebuild on state change |
| gRPC client | ✅ Done | Unix socket connection (same pattern as CLI) |
| Async bridge | ✅ Done | tokio↔tao channel bridge for commands/events |
| Daemon lifecycle | ✅ Done | Spawn, connect, reconnect, shutdown |
| Toggle listening | ✅ Done | Pause/resume via menu |
| Quit | ✅ Done | Shutdown daemon and exit |
| Settings access | ⬜ Deferred | Open config or preferences (Phase 4) |

### Phase 4a: Polish & Distribution

Production readiness and installability.

| Component | Status | Description |
|-----------|--------|-------------|
| Rename daemon to `vcmd` | ⬜ Todo | Unix convention for daemon binary name |
| `cargo install` support | ⬜ Todo | All three binaries installable via cargo |
| GitHub release workflow | ⬜ Todo | Tag-triggered builds, macOS ARM64 tarball |
| Config documentation | ⬜ Todo | `docs/configuration.md` reference |
| Language switching | ⬜ Todo | Menu bar + CLI language selection, `SetLanguage` RPC |
| App icon | ⬜ Todo | Soundwave microphone concept for AI generation |
| VAD speech cutoff fix | ⬜ Todo | Pre-roll buffer to capture speech start |

### Phase 4b: Future Enhancements

| Component | Status | Description |
|-----------|--------|-------------|
| Expanded models | ⬜ Design only | Canary (NeMo/ONNX), Voxtral (LLM-based) via extensible transcriber trait |
| Streaming transcription | ⬜ Design only | Sliding window partial results for lower perceived latency |
| Text formatting | ⬜ Design only | Post-processing pipeline: rule-based then LLM-enhanced dictation intelligence |
| launchd integration | ⬜ Todo | `vcm install` to run on login |
| DMG packaging | ⬜ Todo | Distributable macOS installer |
| Code signing | ⬜ Todo | Sign for Gatekeeper |
| Accessibility permissions | ⬜ Todo | Guide user through granting permissions |
| Error recovery | ⬜ Todo | Handle mic disconnect, model errors gracefully |
| Model integrity hashes | ⬜ Todo | SHA256 verification for downloaded models (currently size-only check) |
| Linux/Windows support | ⬜ Todo | Cross-platform with GPU runtime support |

## CLI Commands

```
vcm start              # Start daemon (vcmd) in background
vcm stop               # Stop daemon
vcm status             # Show daemon state (listening/paused/stopped)
vcm toggle             # Toggle listening on/off
vcm language get       # Show current language
vcm language set <code> # Switch language (any valid Whisper code)

vcm test-mic           # Test microphone input (debug)
vcm transcribe <file>  # Transcribe audio file (debug)

vcm install            # Install launchd service (Phase 4b)
vcm uninstall          # Remove launchd service (Phase 4b)
```

## Menu Bar

```
┌─────────────────────┐
│ ● Voice-Controllm   │  ← Green dot = listening
├─────────────────────┤
│ ○ Pause Listening   │  ← Toggle
│ ─────────────────── │
│ Language             │
│   ● English         │  ← Radio group from [gui].languages
│   ○ Czech           │
│   ○ German          │
│   ○ Auto            │  ← Always present
│ ─────────────────── │
│   Quit              │
└─────────────────────┘
```

## Configuration

Location: `~/.config/voice-controllm/config.toml`

```toml
[model]
model = "whisper-base"
language = "en"

[latency]
mode = "balanced"
min_chunk_seconds = 1.0

[injection]
allowlist = []  # empty = inject everywhere

[logging]
level = "info"

[gui]
languages = ["en", "cs", "de"]  # menu bar language list
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

**Phase 4a: Polish & Distribution** - Rename, releases, language switching, docs, icon.

See `docs/plans/2026-02-15-phase4-design.md` for full design.
