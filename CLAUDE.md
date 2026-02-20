# VCM

Offline voice dictation utility for macOS accessibility.

## Project Structure

```
vcm/
├── Cargo.toml          # Workspace root + root package with binary wrappers
├── src/bin/            # Thin binary wrappers (vcmd, vcmctl, vcm)
├── daemon/             # Background service - audio capture, VAD, transcription, injection
├── cli/                # CLI tool (vcmctl) - start/stop/status commands
├── proto/              # gRPC definitions
└── menubar/            # System tray app - tray-icon/muda/tao
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
cargo run --bin vcmctl -- start

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Key Dependencies

- `cpal` - Cross-platform audio capture
- `whisper-rs` - Whisper speech recognition with CoreML backend
- `ort` - ONNX Runtime for Silero VAD
- `enigo` - Keystroke injection
- `tonic`/`prost` - gRPC
- `tray-icon`/`muda`/`tao` - System tray menu bar app

## Configuration

Default config path: `~/.config/vcm/config.toml`
Models directory: `~/.local/share/vcm/models/`

## Testing

Run with microphone access for integration tests:
```bash
cargo test -- --ignored  # Runs tests requiring hardware
```
