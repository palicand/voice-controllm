# Phase 4a: Polish & Distribution — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make voice-controllm installable, releasable, and add language switching.

**Architecture:** Binary rename + cargo metadata for installability, tag-triggered GitHub Actions for releases, new `SetLanguage` gRPC RPC with dynamic language swap (no engine restart), menu bar language radio list driven by `[gui]` config section.

**Tech Stack:** Rust, tonic/prost (gRPC), tray-icon/muda/tao (menu bar), clap (CLI), GitHub Actions (releases)

---

### Task 1: Rename Daemon Binary to `vcmd`

**Files:**
- Modify: `daemon/Cargo.toml:12-14` (binary name)
- Modify: `cli/src/main.rs:96-99` (daemon spawn path)
- Modify: `menubar/src/bridge.rs:240-244` (daemon spawn path)

**Step 1: Update binary name in daemon Cargo.toml**

In `daemon/Cargo.toml`, change the `[[bin]]` section:
```toml
[[bin]]
name = "vcmd"
path = "src/main.rs"
```

**Step 2: Update CLI daemon spawn**

In `cli/src/main.rs`, find `.join("voice-controllm-daemon")` and change to `.join("vcmd")`.

**Step 3: Update menubar daemon spawn**

In `menubar/src/bridge.rs`, find `.join("voice-controllm-daemon")` and change to `.join("vcmd")`.

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles. Binary at `target/debug/vcmd`.

Run: `ls target/debug/vcmd`
Expected: File exists.

**Step 5: Run existing tests**

Run: `cargo test`
Expected: All existing tests pass (no test references the old binary name directly).

**Step 6: Commit**

```bash
git add daemon/Cargo.toml cli/src/main.rs menubar/src/bridge.rs
git commit -m "feat(daemon): rename binary to vcmd"
```

---

### Task 2: Cargo Install Metadata

**Files:**
- Modify: `Cargo.toml` (workspace metadata)
- Modify: `daemon/Cargo.toml` (package metadata)
- Modify: `cli/Cargo.toml` (package metadata)
- Modify: `menubar/Cargo.toml` (package metadata)

**Step 1: Add workspace-level metadata**

In root `Cargo.toml`, add to `[workspace.package]`:
```toml
[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/palicand/voice-controllm"
description = "Offline voice dictation for macOS accessibility"
categories = ["accessibility", "multimedia::audio"]
keywords = ["voice", "dictation", "speech-to-text", "accessibility", "macos"]
```

**Step 2: Inherit metadata in each crate**

In each crate's `Cargo.toml`, ensure they inherit from workspace and add crate-specific descriptions:

`daemon/Cargo.toml`:
```toml
[package]
name = "voice-controllm-daemon"
description = "Background daemon for voice-controllm — audio capture, VAD, transcription"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
```

`cli/Cargo.toml`:
```toml
[package]
name = "vcm"
description = "CLI for voice-controllm — start/stop daemon, configure settings"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
```

`menubar/Cargo.toml`:
```toml
[package]
name = "vcm-menubar"
description = "System tray app for voice-controllm — menu bar control and status"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
```

Also update `proto/Cargo.toml` and `common/Cargo.toml` similarly (they're internal but should have consistent metadata).

**Step 3: Verify build**

Run: `cargo build`
Expected: Compiles without errors.

**Step 4: Verify cargo install works locally**

Run: `cargo install --path daemon`
Expected: Installs `vcmd` to `~/.cargo/bin/vcmd`.

Run: `cargo install --path cli`
Expected: Installs `vcm` to `~/.cargo/bin/vcm`.

Clean up: `cargo uninstall voice-controllm-daemon vcm`

**Step 5: Commit**

```bash
git add Cargo.toml daemon/Cargo.toml cli/Cargo.toml menubar/Cargo.toml proto/Cargo.toml common/Cargo.toml
git commit -m "chore: add cargo install metadata to all crates"
```

---

### Task 3: GitHub Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Step 1: Write the release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  release:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install protoc
        run: brew install protobuf

      - name: Build release binaries
        run: cargo build --release
        env:
          CMAKE_C_FLAGS: "-march=armv8.2-a+crypto+simd"
          CMAKE_CXX_FLAGS: "-march=armv8.2-a+crypto+simd"

      - name: Strip binaries
        run: |
          strip target/release/vcm
          strip target/release/vcmd
          strip target/release/vcm-menubar

      - name: Package
        run: |
          VERSION=${GITHUB_REF_NAME}
          tar czf voice-controllm-${VERSION}-aarch64-apple-darwin.tar.gz \
            -C target/release vcm vcmd vcm-menubar

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: voice-controllm-*.tar.gz
          generate_release_notes: true
```

**Step 2: Verify workflow syntax**

Run: `cd /Users/palicand/projects/voice-controllm && cat .github/workflows/release.yml | python3 -c "import sys, yaml; yaml.safe_load(sys.stdin); print('Valid YAML')"` (or use `actionlint` if installed)

**Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add tag-triggered release workflow"
```

Note: Actual workflow testing happens when we push a tag. No local verification possible for GitHub Actions.

---

### Task 4: Add `[gui]` Config Section

**Files:**
- Modify: `daemon/src/config.rs:20-28` (add GuiConfig struct)
- Modify: `daemon/src/config_test.rs` (add tests)

**Step 1: Write the failing test for GuiConfig parsing**

In `daemon/src/config_test.rs`, add:

```rust
#[test]
fn gui_languages_parsed() {
    let toml = r#"
[gui]
languages = ["en", "cs", "de"]
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.gui.languages, vec!["en", "cs", "de"]);
}

#[test]
fn gui_defaults_to_empty_languages() {
    let config: Config = toml::from_str("").unwrap();
    assert!(config.gui.languages.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p voice-controllm-daemon gui_languages`
Expected: FAIL — no field `gui` on `Config`.

**Step 3: Add GuiConfig struct and wire into Config**

In `daemon/src/config.rs`, add the struct (near the other config structs):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GuiConfig {
    pub languages: Vec<String>,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
        }
    }
}
```

Add field to `Config`:
```rust
pub struct Config {
    // ... existing fields ...
    pub gui: GuiConfig,
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p voice-controllm-daemon gui_languages`
Expected: PASS.

**Step 5: Run all tests**

Run: `cargo test`
Expected: All pass.

**Step 6: Commit**

```bash
git add daemon/src/config.rs daemon/src/config_test.rs
git commit -m "feat(config): add [gui] section with languages list"
```

---

### Task 5: Add `SetLanguage` gRPC RPC

**Files:**
- Modify: `proto/src/voice_controllm.proto` (new RPC + messages)
- Modify: `daemon/src/grpc.rs` (implement server handler)
- Modify: `common/src/grpc.rs` (if client helper exists)

**Step 1: Add proto definitions**

In `proto/src/voice_controllm.proto`, add to the service block:

```protobuf
rpc SetLanguage(SetLanguageRequest) returns (Empty);
rpc GetLanguage(Empty) returns (GetLanguageResponse);
```

Add messages:

```protobuf
message SetLanguageRequest {
  string language = 1;
}

message GetLanguageResponse {
  string language = 1;
  repeated string available_languages = 2;
}
```

**Step 2: Rebuild proto**

Run: `cargo build -p voice-controllm-proto`
Expected: Compiles, generates new Rust types.

**Step 3: Add stub server implementation**

In the daemon's gRPC service implementation (likely `daemon/src/grpc.rs`), add handler stubs:

```rust
async fn set_language(
    &self,
    request: Request<SetLanguageRequest>,
) -> Result<Response<Empty>, Status> {
    let lang = request.into_inner().language;
    self.controller
        .set_language(&lang)
        .map_err(|e| Status::invalid_argument(e.to_string()))?;
    Ok(Response::new(Empty {}))
}

async fn get_language(
    &self,
    _request: Request<Empty>,
) -> Result<Response<GetLanguageResponse>, Status> {
    let (language, available) = self.controller.get_language_info();
    Ok(Response::new(GetLanguageResponse {
        language,
        available_languages: available,
    }))
}
```

These will fail to compile until Task 6 adds the controller methods. That's fine — commit the proto changes first.

**Step 4: Verify proto builds**

Run: `cargo build -p voice-controllm-proto`
Expected: Compiles.

**Step 5: Commit**

```bash
git add proto/src/voice_controllm.proto
git commit -m "feat(proto): add SetLanguage and GetLanguage RPCs"
```

---

### Task 6: Engine Dynamic Language Switching

**Files:**
- Modify: `daemon/src/transcribe/whisper.rs:18-21` (add set_language method)
- Modify: `daemon/src/engine.rs:108-112` (expose language change)
- Modify: `daemon/src/controller.rs` (add set_language, get_language_info)
- Modify: `daemon/src/grpc.rs` (wire up stubs from Task 5)

**Step 1: Add `set_language` to WhisperTranscriber**

In `daemon/src/transcribe/whisper.rs`, add method:

```rust
pub fn set_language(&mut self, language: Option<String>) {
    self.language = language;
}
```

This takes effect on the next `transcribe()` call since `language` is read in `FullParams` setup each time.

**Step 2: Add language mutation to Engine**

The Engine holds the transcriber. Add a method to forward language changes. The engine runs in a tokio task, so this needs a channel or `Arc<Mutex>`. Check how the engine is currently structured — if the transcriber is behind a mutex, add a method. If not, add a language channel.

The simplest approach: store language as `Arc<Mutex<Option<String>>>` shared between the engine loop and the controller. The engine reads it before each transcription call.

In `daemon/src/engine.rs`, add:
```rust
pub fn set_language(&self, language: Option<String>) {
    *self.language.lock().unwrap() = language;
}
```

And in the transcription path, before calling `transcriber.transcribe()`:
```rust
let lang = self.language.lock().unwrap().clone();
transcriber.set_language(lang);
```

**Step 3: Add controller methods**

In `daemon/src/controller.rs`, add:

```rust
pub fn set_language(&self, language: &str) -> Result<()> {
    let lang = if language == "auto" { None } else { Some(language.to_string()) };
    self.engine.set_language(lang);
    // Persist to config
    let mut config = Config::load()?;
    config.model.language = language.to_string();
    config.save()?;
    Ok(())
}

pub fn get_language_info(&self) -> (String, Vec<String>) {
    let config = Config::load().unwrap_or_default();
    (config.model.language.clone(), config.gui.languages.clone())
}
```

**Step 4: Wire gRPC handlers**

Complete the stub implementations from Task 5 — they should now compile with the controller methods in place.

**Step 5: Build and test**

Run: `cargo build`
Expected: Compiles.

Run: `cargo test`
Expected: All pass.

**Step 6: Commit**

```bash
git add daemon/src/transcribe/whisper.rs daemon/src/engine.rs daemon/src/controller.rs daemon/src/grpc.rs
git commit -m "feat(engine): dynamic language switching without restart"
```

---

### Task 7: CLI Language Commands

**Files:**
- Modify: `cli/src/main.rs:20-35` (add Language command)

**Step 1: Add Language subcommand**

In `cli/src/main.rs`, add to `Commands` enum:

```rust
Language {
    #[command(subcommand)]
    action: LanguageAction,
},
```

Add enum:
```rust
#[derive(Subcommand)]
enum LanguageAction {
    /// Show current language
    Get,
    /// Switch language
    Set {
        /// Language code (e.g. "en", "cs", "de") or "auto"
        code: String,
    },
}
```

**Step 2: Implement handlers**

In the main match block, add:

```rust
Commands::Language { action } => {
    let mut client = connect_to_daemon().await?;
    match action {
        LanguageAction::Get => {
            let resp = client.get_language(Empty {}).await?.into_inner();
            println!("{}", resp.language);
        }
        LanguageAction::Set { code } => {
            client
                .set_language(SetLanguageRequest { language: code.clone() })
                .await?;
            println!("Language set to: {}", code);
        }
    }
}
```

**Step 3: Build and test**

Run: `cargo build -p vcm`
Expected: Compiles.

Run: `cargo run -p vcm -- language --help`
Expected: Shows `get` and `set` subcommands.

**Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat(cli): add vcm language get/set commands"
```

---

### Task 8: Menu Bar Language Switching

**Files:**
- Modify: `menubar/src/tray.rs:8-40` (add language menu items)
- Modify: `menubar/src/bridge.rs:23-28` (add SetLanguage command)
- Modify: `menubar/src/main.rs:16-22,62-77` (handle language menu events)
- Modify: `menubar/src/state.rs` (add language tracking)

**Step 1: Add language to AppState or a separate struct**

The menu bar needs to know: (a) available languages from config, (b) current active language from daemon.

In `menubar/src/state.rs`, add:
```rust
#[derive(Debug, Clone)]
pub struct LanguageInfo {
    pub active: String,
    pub available: Vec<String>,
}
```

**Step 2: Add SetLanguage command**

In `menubar/src/bridge.rs`, extend `Command`:
```rust
pub enum Command {
    StartListening,
    StopListening,
    SetLanguage(String),
    Shutdown,
}
```

Handle it in the command loop alongside existing commands:
```rust
Command::SetLanguage(lang) => {
    let _ = grpc_client
        .set_language(SetLanguageRequest { language: lang })
        .await;
}
```

**Step 3: Fetch language info on connect**

In the bridge's connection handler (after successful daemon connect), call `GetLanguage` and send the info back via the event channel so the main loop can rebuild the menu.

**Step 4: Build language menu items**

In `menubar/src/tray.rs`, modify `build_menu` to accept `LanguageInfo` and create radio-style items:

```rust
pub struct MenuItems {
    pub toggle: MenuItem,
    pub language_items: Vec<(MenuItem, String)>, // (menu item, language code)
    pub quit: MenuItem,
}
```

Build one `MenuItem` per language in `available`, plus "Auto". Use checkmarks (`✓` prefix or enabled/disabled state) to indicate the active one.

Menu layout:
```
Status line
───────────
Toggle
───────────
Language
  ● English
  ○ Czech
  ○ Auto
───────────
Quit
```

Note: `muda` supports `CheckMenuItem` for radio-style behavior. Use that instead of regular `MenuItem`.

**Step 5: Handle language menu events**

In `menubar/src/main.rs`, extend `handle_menu_event` to check if the clicked item matches any language item, and if so send `Command::SetLanguage(code)`.

**Step 6: Rebuild menu on language change**

After a successful `SetLanguage`, the daemon should send an event (or the menu bar re-fetches language info) and rebuild the menu with the new active language.

**Step 7: Build and test manually**

Run: `cargo build -p vcm-menubar`
Expected: Compiles.

Manual test: Start daemon, start menubar, verify language items appear, click one, verify it switches.

**Step 8: Commit**

```bash
git add menubar/src/tray.rs menubar/src/bridge.rs menubar/src/main.rs menubar/src/state.rs
git commit -m "feat(menubar): language switching via menu"
```

---

### Task 9: Configuration Documentation

**Files:**
- Create: `docs/configuration.md`
- Modify: `README.md` (add link)

**Step 1: Write configuration reference**

Create `docs/configuration.md` with:

1. Config file location and creation
2. Full annotated example config (all sections, all fields, comments)
3. `[model]` section: available models table (from `SpeechModel` enum), language codes
4. `[latency]` section: mode descriptions (Fast/Balanced/Accurate), min_chunk_seconds
5. `[injection]` section: allowlist examples
6. `[logging]` section: level options
7. `[gui]` section: languages list explanation

The models table should list all variants from `daemon/src/models.rs`:
| Config Value | Size | Notes |
|---|---|---|
| whisper-tiny | ~77 MB | Fastest, least accurate |
| whisper-tiny.en | ~77 MB | English-only tiny |
| whisper-base | ~147 MB | Default, good balance |
| ...          | ...     | (see docs/configuration.md for full list) |
| whisper-large-v3-turbo | ~1.6 GB | Best speed/accuracy ratio for large |

**Step 2: Add README links**

Add a "Configuration" section to README.md pointing to `docs/configuration.md` and mentioning `cargo doc --open` for developer API docs.

**Step 3: Commit**

```bash
git add docs/configuration.md README.md
git commit -m "docs: add configuration reference"
```

---

### Task 10: App Icon Concept

**Files:**
- Create: `docs/plans/icon-prompt.md` (AI generation prompt + specs)

**Step 1: Write icon generation prompt**

Create a detailed prompt document with:
- Visual concept description (soundwave microphone)
- Color palette (hex codes)
- Style references (macOS Big Sur icon style, rounded squircle)
- Required sizes for `iconutil`: 16x16, 32x32, 128x128, 256x256, 512x512, 1024x1024
- Constraints: must be recognizable at 16x16, clean silhouette

**Step 2: Commit**

```bash
git add docs/plans/icon-prompt.md
git commit -m "docs: app icon concept and generation prompt"
```

---

## Task Order & Dependencies

```
Task 1 (rename vcmd) ──────────────────┐
Task 2 (cargo metadata) ───────────────┤
Task 3 (release workflow) ─────────────┤── Independent, can parallel
Task 10 (icon concept) ────────────────┘

Task 4 (gui config) ───┐
                       ├── Sequential chain
Task 5 (proto) ────────┤
                       │
Task 6 (engine) ───────┤ (depends on 4, 5)
                       │
Task 7 (CLI) ──────────┤ (depends on 5, 6)
                       │
Task 8 (menu bar) ─────┘ (depends on 4, 5, 6)

Task 9 (docs) ─────────── After all features implemented
```

Tasks 1-3, 10 can be done in parallel. Tasks 4-8 are a sequential chain for the language feature. Task 9 comes last.
