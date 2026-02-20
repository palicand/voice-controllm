# VCM

Offline voice dictation utility for macOS accessibility. Designed to replace Apple Voice Control with a more accurate, configurable solution.

**Status**: Usable - daemon captures audio, transcribes speech, and injects keystrokes. Controlled via CLI.

## Features

- **Offline dictation** - All processing local, no cloud services required
- **Low latency** - 1-2 second transcription delay
- **Multilingual** - 99+ languages via Whisper
- **Auto model management** - Models download automatically on first run
- **App allowlisting** - Restrict keystroke injection to specific applications
- **CoreML acceleration** - Native Apple Silicon performance via CoreML encoder

## Architecture

```
+-------------------+     Unix Socket/gRPC     +------------------------+
|   CLI (vcmctl)    |<------------------------>|     Daemon (vcmd)      |
+-------------------+                          |                        |
                                               |  Audio (cpal)          |
+-------------------+     Unix Socket/gRPC     |       |                |
|  Menu Bar App     |<------------------------>|  VAD (Silero)          |
|  (vcm)            |                          |       |                |
+-------------------+                          |  Whisper (CoreML)      |
                                               |       |                |
                                               |  Keystroke Injection   |
                                               +------------------------+
```

## Requirements

- macOS (Apple Silicon recommended for CoreML acceleration)
- Rust toolchain (1.85+)
- Microphone permissions (grant in System Settings > Privacy & Security > Microphone)
- Accessibility permissions for keystroke injection (grant in System Settings > Privacy & Security > Accessibility)

## Installation

```bash
# From crates.io (once published)
cargo install vcm

# From source
cargo install --path .

# Or just build locally
cargo build --release
```

This installs three binaries: `vcmd` (daemon), `vcmctl` (CLI), and `vcm` (menu bar app).

## Quick Start

```bash
# Initialize config (optional - defaults work out of the box)
vcmctl config init

# Start the daemon (downloads models on first run)
vcmctl start

# Toggle listening on/off
vcmctl toggle

# Check current state
vcmctl status

# Stop the daemon
vcmctl stop
```

On first launch, `vcmctl start` downloads the required models (~150 MB for whisper-base) and shows progress. Subsequent starts are fast.

## Configuration

Configuration file: `~/.config/vcm/config.toml`

```toml
[model]
model = "whisper-base"  # whisper-tiny through whisper-large-v3-turbo
language = "auto"       # "auto" for detection, or "en", "english", "sk", etc.

[latency]
mode = "balanced"       # "fast" | "balanced" | "accurate"

[injection]
allowlist = []          # Empty = all apps; ["Terminal", "kitty"] = only those apps

[logging]
level = "info"          # error, warn, info, debug, trace
```

Models are stored in `~/.local/share/vcm/models/` and download automatically from Hugging Face.

See [docs/configuration.md](docs/configuration.md) for the full configuration reference with all options, model sizes, and defaults.

## Documentation

Generate the API documentation for developers:

```bash
cargo doc --open
```

## Known Issues

**Slow CoreML model loading on every start** — The CoreML encoder model recompiles for the device on each daemon launch instead of being cached by macOS. This is an [upstream whisper.cpp issue](https://github.com/ggml-org/whisper.cpp/issues/2126) that affects larger models (medium, large, large-v3-turbo). Smaller models (tiny, base) cache more reliably. macOS 15+ has improved caching. Clearing `~/Library/Application Support/coreMLCache/` and restarting may help.

## Logging

Daemon logs are written to `~/.local/state/vcm/daemon.log`. Set the log level in config or override with `VCM_LOG`:

```bash
VCM_LOG=debug vcmctl start
```

## License

MIT
