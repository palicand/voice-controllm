# Voice-Controllm

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
|   CLI (vcm)       |<------------------------>|       Daemon           |
+-------------------+                          |                        |
                                               |  Audio (cpal)          |
+-------------------+     Unix Socket/gRPC     |       |                |
|  Menu Bar App     |<------------------------>|  VAD (Silero)          |
|  (Tauri, planned) |                          |       |                |
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

## Building

```bash
cargo build --release
```

## Quick Start

```bash
# Initialize config (optional - defaults work out of the box)
vcm config init

# Start the daemon (downloads models on first run)
vcm start

# Toggle listening on/off
vcm toggle

# Check current state
vcm status

# Stop the daemon
vcm stop
```

On first launch, `vcm start` downloads the required models (~150 MB for whisper-base) and shows progress. Subsequent starts are fast.

## Configuration

Configuration file: `~/.config/voice-controllm/config.toml`

```toml
[model]
model = "whisper-base"  # whisper-tiny, whisper-base, whisper-small, whisper-medium, whisper-large-v3, whisper-large-v3-turbo
languages = ["auto"]    # or ["en"], ["en", "cs", "de"], etc.

[latency]
mode = "balanced"  # "fast" | "balanced" | "accurate"
min_chunk_seconds = 1.0

[injection]
# Empty allowlist = inject to all apps (default)
# With allowlist = only inject to matching apps (case-insensitive, partial match)
allowlist = []
```

Models are stored in `~/.local/share/voice-controllm/models/` and download automatically from Hugging Face.

## License

MIT
