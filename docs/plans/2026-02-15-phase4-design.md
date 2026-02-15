# Phase 4: Polish & Distribution — Design

## Scope

### Phase 4a — Implement Now

1. Rename daemon binary to `vcmd`
2. `cargo install` support for all three binaries
3. GitHub release workflow (tag-triggered, macOS ARM64)
4. Configuration documentation
5. Language switching in menu bar
6. App icon concept

### Phase 4b — Roadmap (Design Only)

- Expanded model support (Canary, Voxtral)
- Streaming transcription
- Text formatting / dictation intelligence

### Tracked Bug

- VAD speech start cutoff (pre-roll buffer fix)

---

## 1. Daemon Rename

Rename binary from `voice-controllm-daemon` to `vcmd`.

**Changes:**
- `daemon/Cargo.toml`: `[[bin]] name = "vcmd"`
- `cli/src/daemon_manager.rs`: update spawn command
- `menubar/src/bridge.rs`: update daemon binary lookup
- Package name stays `voice-controllm-daemon`

**Binary name convention:**
| Crate | Package Name | Binary Name |
|-------|-------------|-------------|
| cli | vcm | vcm |
| daemon | voice-controllm-daemon | vcmd |
| menubar | vcm-menubar | vcm-menubar |

## 2. `cargo install` Support

All three crates installable via:
```
cargo install voice-controllm-daemon  # installs vcmd
cargo install vcm                     # installs vcm
cargo install vcm-menubar             # installs vcm-menubar
```

Ensure `Cargo.toml` metadata (description, license, repository, categories, keywords) is populated for crates.io readiness.

## 3. GitHub Releases

**Trigger:** Push tag matching `v*`.

**Workflow (`.github/workflows/release.yml`):**
1. Build `--release` on macOS ARM64 (same CoreML flags as CI)
2. Strip binaries
3. Package: `voice-controllm-v{VERSION}-aarch64-apple-darwin.tar.gz` containing `vcm`, `vcmd`, `vcm-menubar`
4. Create GitHub Release with tarball attached
5. Auto-generate changelog from commits since last tag

**Release process:**
```bash
# Bump version in workspace Cargo.toml, commit
git tag v0.2.0
git push origin main --tags
```

Future: add macOS Intel, Linux, Windows runners as demand requires.

## 4. Configuration Documentation

Create `docs/configuration.md` covering:
- Config file location (`~/.config/voice-controllm/config.toml`)
- Models directory (`~/.local/share/voice-controllm/models/`)
- Full annotated example config with comments
- Available models table (name, size, description)
- Available languages (Whisper's supported list + "auto")
- Latency modes (Fast/Balanced/Accurate) and their effect
- Injection allowlist with examples
- GUI section explanation

README points to `docs/configuration.md` and mentions `cargo doc --open` for developer API docs.

## 5. Language Switching

### Config

New `[gui]` section. `language` in `[model]` remains the active language.

```toml
[model]
model = "whisper-base"
language = "en"

[gui]
languages = ["en", "cs", "de"]
```

`[gui].languages` is a display hint for the menu bar only. CLI doesn't use it.

### gRPC

```protobuf
rpc SetLanguage(SetLanguageRequest) returns (SetLanguageResponse);

message SetLanguageRequest {
  string language = 1;
}

message SetLanguageResponse {}

// Extend StatusResponse
message StatusResponse {
  // ... existing fields ...
  string active_language = N;
  repeated string available_languages = N+1;
}
```

### CLI

```bash
vcm language set cs    # any valid Whisper language code
vcm language get       # show current active language
```

### Engine

Language is a Whisper decode-time parameter. Switching is an atomic swap of the language string — no model reload, no engine restart.

### Menu Bar

```
┌─────────────────────┐
│ ● Voice-Controllm   │
├─────────────────────┤
│ ○ Pause Listening   │
│ ─────────────────── │
│ Language             │
│   ● English         │
│   ○ Czech           │
│   ○ German          │
│   ○ Auto            │
│ ─────────────────── │
│   Quit              │
└─────────────────────┘
```

"Auto" always present. Active language shown with bullet. Selecting a language calls `SetLanguage` RPC.

## 6. App Icon

**Concept: Soundwave Microphone**
- Modern rounded microphone silhouette
- Concentric sound waves emanating from mic
- Deep blue/indigo gradient background
- White/light mic and waves
- macOS squircle shape with subtle shadow

**Deliverable:** Detailed AI image generation prompt + SVG sizing template for macOS icon sizes (16x16 through 1024x1024).

---

## Roadmap Items (Design Only)

### Expanded Models (Canary, Voxtral)

The `Transcriber` trait already abstracts the speech-to-text backend. To support non-Whisper models:

- Add `TranscriberBackend` enum to config: `Whisper`, `Canary`, `Voxtral`
- Each backend has its own model download/management in `models.rs`
- Canary: ONNX export via NVIDIA NeMo, runs through `ort`
- Voxtral: LLM-based, needs candle or llama.cpp runtime
- Key challenge: different input formats, tokenizers, and output handling per backend

### Streaming Transcription

Reduce perceived latency by showing partial results during speech:

- Sliding window: transcribe overlapping windows while speech is ongoing
- Display partial results, replace with final on `SpeechEnd`
- New gRPC server-streaming RPC for real-time text updates
- Text injection needs "replace previous" mode (select + overwrite)
- Benefits from VAD fix first (accurate speech start = better partial results)

### Text Formatting / Dictation Intelligence

Post-processing pipeline between transcription and keystroke injection:

- **Phase 1 (rule-based):** Keyword detection for formatting commands
  - "new line" → `\n`
  - "new paragraph" → `\n\n`
  - "bullet point" / "dash" → `- `
  - "period" / "comma" / "question mark" → punctuation
- **Phase 2 (LLM-enhanced):** Small local model (Phi-3, TinyLlama) for context-aware formatting
  - Detect list context and auto-format
  - Smart capitalization and punctuation
  - Requires additional model download and inference pipeline

### VAD Speech Start Cutoff (Bug)

**Root cause:** VAD state machine requires 2+ consecutive speech chunks before firing `SpeechStart`. Audio before that trigger point is discarded.

**Fix:** Maintain a rolling pre-roll buffer (~300ms) of audio. When `SpeechStart` fires, prepend the pre-roll buffer to the speech buffer so the beginning of the utterance is captured.

**Location:** `daemon/src/engine.rs` — the audio buffer management in the main loop.
