# Configuration Reference

voice-controllm is configured via a TOML file at:

```
~/.config/voice-controllm/config.toml
```

If the file does not exist, all settings use their defaults. You can generate a default config with `vcmctl config init`.

The XDG base directory spec is respected: set `$XDG_CONFIG_HOME` to override `~/.config`.

## Models directory

Speech recognition models are stored in:

```
~/.local/share/voice-controllm/models/
```

Models download automatically from Hugging Face on first use. Downloads support resume if interrupted. On macOS, CoreML encoder models are also downloaded and extracted alongside the main model for Apple Silicon acceleration.

Override the base path with `$XDG_DATA_HOME`.

## Full annotated example

```toml
[model]
model = "whisper-base"     # Speech recognition model (default: whisper-base)
language = "auto"          # Language for transcription (default: auto-detect)

[latency]
mode = "balanced"          # Latency/accuracy trade-off (default: balanced)
min_chunk_seconds = 1.0    # Minimum audio chunk before transcribing, in seconds (default: 1.0)

[injection]
# allowlist = ["Terminal", "kitty"]  # Omit or leave empty to inject into all apps

[logging]
level = "info"             # Log verbosity (default: info)

[daemon]
initial_state = "listening"  # State after initialization (default: listening)

[gui]
# languages = ["en", "cs", "de"]  # Language codes shown in menu bar switcher
```

All sections and fields are optional. Missing fields use the defaults shown above.

## `[model]` section

### `model`

Selects the Whisper model variant. Larger models are more accurate but slower and use more memory.

| Config value              | Size    | Notes                                      |
|---------------------------|---------|--------------------------------------------|
| `whisper-tiny`            | ~75 MB  | Fastest, lowest accuracy                   |
| `whisper-tiny-en`         | ~75 MB  | English-only, slightly better than tiny    |
| **`whisper-base`**        | ~150 MB | **Default.** Good balance for most users   |
| `whisper-base-en`         | ~150 MB | English-only base                          |
| `whisper-small`           | ~500 MB | Noticeably more accurate                   |
| `whisper-small-en`        | ~500 MB | English-only small                         |
| `whisper-medium`          | ~1.5 GB | High accuracy, slower                      |
| `whisper-medium-en`       | ~1.5 GB | English-only medium                        |
| `whisper-large-v3`        | ~3 GB   | Best accuracy, significant resource usage  |
| `whisper-large-v3-turbo`  | ~1.5 GB | Near large-v3 accuracy, medium-like speed  |

English-only (`-en`) models are slightly more accurate for English but cannot transcribe other languages. Multilingual models support 99+ languages.

### `language`

Controls the transcription language.

- `"auto"` (default) -- Whisper detects the spoken language automatically.
- A language name or code -- Forces transcription in that language. Accepts either the full name (`"english"`, `"slovak"`) or the ISO 639-1 code (`"en"`, `"sk"`).

For the full list of supported languages, see the [Whisper language list](https://github.com/openai/whisper/blob/main/whisper/tokenizer.py#L10-L119).

Note: English-only models (`-en` variants) ignore this setting and always transcribe in English.

## `[latency]` section

### `mode`

Controls the trade-off between transcription speed and accuracy.

| Mode         | Description                                                  |
|--------------|--------------------------------------------------------------|
| `fast`       | Transcribe as soon as possible. Lower accuracy, less context.|
| **`balanced`** | **Default.** Waits for natural pauses before transcribing. |
| `accurate`   | Waits longer to accumulate more context. Higher accuracy.    |

### `min_chunk_seconds`

Minimum duration of audio (in seconds) to accumulate before sending to the transcription model. Lower values reduce latency but may decrease accuracy.

**Default:** `1.0`

## `[injection]` section

### `allowlist`

Controls which applications receive injected keystrokes.

- **Empty or omitted** (default) -- Injects into all applications.
- **List of app names** -- Only injects into applications whose name matches an entry.

```toml
[injection]
allowlist = ["Terminal", "kitty", "IntelliJ IDEA"]
```

## `[logging]` section

### `level`

Sets the daemon log verbosity. Logs are written to `~/.local/state/voice-controllm/daemon.log`.

| Level   | Description                              |
|---------|------------------------------------------|
| `error` | Errors only                              |
| `warn`  | Errors and warnings                      |
| **`info`** | **Default.** Normal operational messages |
| `debug` | Detailed diagnostic output               |
| `trace` | Everything, including per-frame data     |

You can override the config level at runtime with the `VCM_LOG` environment variable:

```bash
VCM_LOG=debug vcmctl start
```

## `[daemon]` section

### `initial_state`

Controls the state the daemon enters after model initialization completes.

| Value         | Description                                           |
|---------------|-------------------------------------------------------|
| **`listening`** | **Default.** Start listening immediately after init. |
| `paused`      | Stay paused -- user must toggle listening manually.   |

```toml
[daemon]
initial_state = "paused"
```

## `[gui]` section

### `languages`

A list of language codes to display in the menu bar app's language switcher. This only affects the GUI -- it does not change what languages the model can transcribe.

```toml
[gui]
languages = ["en", "cs", "de"]
```

**Default:** empty (no language switcher shown).
