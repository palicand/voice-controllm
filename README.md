# Voice-Controllm

Offline voice dictation utility with keyword detection for accessibility. Designed to replace Apple Voice Control with a more accurate, configurable solution.

**Status**: Phase 1 (Core Engine) - Audio capture, VAD, transcription, keystroke injection working

## Features (Planned)

- **Offline dictation** - No cloud services, all processing local
- **Low latency** - 1-2 second transcription delay
- **Keyword detection** - Trigger actions with voice commands
- **Menu bar integration** - macOS system tray widget
- **Multilingual** - Support for 99+ languages via Whisper

## Architecture

```
+-------------------+     Unix Socket/gRPC     +------------------------+
|   CLI (vcm)       |<------------------------>|       Daemon           |
+-------------------+                          |                        |
                                               |  Audio (cpal)          |
+-------------------+     Unix Socket/gRPC     |       |                |
|  Menu Bar App     |<------------------------>|  VAD (Silero)          |
|  (Tauri)          |                          |       |                |
+-------------------+                          |  Whisper (CoreML)      |
                                               |       |                |
                                               |  Keystroke Injection   |
                                               +------------------------+
```

## Requirements

- macOS (Apple Silicon recommended for CoreML acceleration)
- Rust toolchain (1.85+)
- Microphone permissions

## Building

```bash
cargo build --release
```

## Usage

```bash
# Start dictation
vcm start

# Stop dictation
vcm stop

# Check status
vcm status
```

## Configuration

Configuration file: `~/.config/voice-controllm/config.toml`

```toml
[model]
model = "whisper-large-v3-turbo"  # or whisper-base, whisper-small, etc.
languages = ["auto"]  # or ["en", "cs", "de"]

[latency]
mode = "balanced"  # "fast" | "balanced" | "accurate"
min_chunk_seconds = 1.0

[injection]
# Empty allowlist = inject to all apps (default)
# With allowlist = only inject to matching apps (case-insensitive, partial match)
allowlist = ["Terminal", "Code", "IntelliJ"]
```

## License

MIT
