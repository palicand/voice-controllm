# Voice-Controllm

Offline voice dictation utility for macOS accessibility.

## Project Structure

```
voice-controllm/
├── Cargo.toml          # Workspace root
├── daemon/             # Background service - audio capture, VAD, transcription, injection
├── cli/                # CLI tool (vcmctl) - start/stop/status commands
├── proto/              # gRPC definitions (Phase 2)
└── menubar/            # System tray app - tray-icon/muda/tao (Phase 3)
```

## Design Principles

- **Parse, don't validate**: Use types that make invalid states unrepresentable. Prefer enums over stringly-typed fields where the set of valid values is known at compile time.

## Development

**IMPORTANT**: All dependency changes must go through `cargo add`/`cargo remove` commands. Do not manually edit `Cargo.toml` for dependency changes.

## Commits

- Subject line only (e.g., `feat(daemon): add configuration management`)
- Body is optional - only include when there's non-obvious context to convey
- Don't describe code in the body - the diff shows that
- No references to internal planning phases or documents

```bash
# Build all crates
cargo build

# Run CLI
cargo run -p vcmctl -- start

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Implementation Phases

1. **Phase 0**: Project scaffolding, CI setup
2. **Phase 1**: Core engine - audio capture, VAD, Whisper transcription, keystroke injection
3. **Phase 2**: IPC layer - gRPC API, daemon/CLI communication, config management
4. **Phase 3**: Menu bar app - tray-icon/muda/tao system tray with status indicator
5. **Phase 4**: Polish & distribution - launchd, DMG, code signing

## Key Dependencies

- `cpal` - Cross-platform audio capture
- `whisper-rs` - Whisper speech recognition with CoreML backend
- `ort` - ONNX Runtime for Silero VAD
- `enigo` - Keystroke injection
- `tonic`/`prost` - gRPC (Phase 2)
- `tray-icon`/`muda`/`tao` - System tray menu bar app (Phase 3)

## Configuration

Default config path: `~/.config/voice-controllm/config.toml`
Models directory: `~/.local/share/voice-controllm/models/`

## Testing

Run with microphone access for integration tests:
```bash
cargo test -- --ignored  # Runs tests requiring hardware
```
