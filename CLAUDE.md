# Voice-Controllm

Offline voice dictation utility for macOS accessibility.

## Project Structure

```
voice-controllm/
├── Cargo.toml          # Workspace root
├── daemon/             # Background service - audio capture, VAD, transcription, injection
├── cli/                # CLI tool (vcm) - start/stop/status commands
├── proto/              # gRPC definitions (Phase 2)
└── menubar/            # Tauri system tray app (Phase 3)
```

## Development

```bash
# Build all crates
cargo build

# Run CLI
cargo run -p vcm -- start

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Implementation Phases

1. **Phase 0** (current): Project scaffolding, CI setup
2. **Phase 1**: Core engine - audio capture, VAD, Whisper transcription, keystroke injection
3. **Phase 2**: IPC layer - gRPC API, daemon/CLI communication, config management
4. **Phase 3**: Menu bar app - Tauri system tray with status indicator

## Key Dependencies

- `cpal` - Cross-platform audio capture
- `whisper-rs` - Whisper speech recognition with CoreML backend
- `ort` - ONNX Runtime for Silero VAD
- `enigo` - Keystroke injection
- `tonic`/`prost` - gRPC (Phase 2)
- `tauri` - Menu bar app (Phase 3)

## Configuration

Default config path: `~/.config/voice-controllm/config.toml`
Models directory: `~/.local/share/voice-controllm/models/`

## Testing

Run with microphone access for integration tests:
```bash
cargo test -- --ignored  # Runs tests requiring hardware
```
