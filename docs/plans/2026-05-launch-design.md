# VCM v0.1 Launch — macOS .app, DMG, Release, oslog, Platform Abstraction

Design doc for the first official VCM release. Converts the current CLI/menubar
binary set into a signable, notarizable, distributable macOS application.

## Goals

1. Distributable `VCM.app` bundle launchable from `/Applications`, no Dock icon.
2. Signed + notarized DMG, conditional on credentials (local builds work unsigned).
3. GitHub release workflow on `v*` tags producing the DMG + checksum.
4. Logs flow through macOS unified logging (`os_log`) so users see them in
   Console.app.
5. Trait boundaries for the two macOS-only branches in the daemon so future
   Linux/Windows work doesn't require a refactor.

## Non-goals

- Sparkle / auto-update (future work).
- Windows/Linux implementations — only trait boundaries with macOS impl.
- App Store distribution — Developer ID for direct distribution only.
- Universal binary — aarch64 only for v0.1 (CoreML is Apple Silicon).

## Bundle layout

```
VCM.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   ├── vcm        # launcher (menubar) — Info.plist CFBundleExecutable
│   │   ├── vcmd       # daemon, spawned by vcm
│   │   └── vcmctl     # CLI, available for users who want it
│   └── Resources/
│       └── AppIcon.icns
```

The menubar binary `vcm` already locates `vcmd` via
`current_exe().parent().join("vcmd")` (see `menubar/src/bridge.rs`), which works
unchanged inside the bundle.

`vcmctl` is shipped inside the bundle for power users; the README documents
`ln -s /Applications/VCM.app/Contents/MacOS/vcmctl /usr/local/bin/vcmctl` for
PATH access.

### Info.plist key choices

| Key | Value | Why |
|---|---|---|
| `CFBundleIdentifier` | `com.palicka.vcm` | Stable across the app + logging subsystem |
| `CFBundleExecutable` | `vcm` | Existing menubar binary |
| `CFBundleVersion` / `CFBundleShortVersionString` | from `workspace.package.version` | Single source of truth |
| `LSUIElement` | `true` | Menu-bar-only, no Dock icon |
| `LSMinimumSystemVersion` | `13.0` | macOS Ventura; matches CoreML feature support tier |
| `NSMicrophoneUsageDescription` | "VCM uses the microphone to transcribe your speech offline." | Required for mic access |
| `NSAppleEventsUsageDescription` | (for AppleScript frontmost app query) | Required when sending events to System Events |

Accessibility (keystroke injection) prompt is triggered at runtime by `enigo` —
no Info.plist key reliably gates it; users grant in System Settings. The README
explains.

## Bundling approach: hand-rolled `scripts/package.sh`

Considered:
- **cargo-bundle**: unmaintained (last release 2022), known issues with
  workspaces and tray-icon crates.
- **cargo-packager**: heavier dependency, opinionated about manifests; we want
  full control over multi-binary layout.
- **Hand-rolled shell**: ~100 lines, easy to debug, no third-party tool to
  break in CI.

Choosing hand-rolled. The script:

1. `cargo build --release --target aarch64-apple-darwin` (workspace).
2. `mkdir -p VCM.app/Contents/{MacOS,Resources}`.
3. Copy `vcm`, `vcmd`, `vcmctl` to `Contents/MacOS/`.
4. Copy `Info.plist` (template with version substitution) and `AppIcon.icns`
   to `Contents/`.
5. Run `codesign` (conditional).
6. Build DMG via `hdiutil` with a `/Applications` symlink for drag-to-install.
7. Notarize + staple (conditional).

Outputs into `dist/`:
- `dist/VCM.app`
- `dist/VCM-<version>-aarch64.dmg`
- `dist/VCM-<version>-aarch64.dmg.sha256`

## Code signing — conditional

Local-friendly: if no signing identity is in env, fall back to ad-hoc
(`codesign -s -`) so the app runs on the developer's own machine without
Gatekeeper grief.

`scripts/sign.sh`:

```bash
if [[ -n "${APPLE_DEVELOPER_ID:-}" ]]; then
  codesign --force --options runtime --timestamp \
    --entitlements scripts/entitlements.plist \
    --sign "$APPLE_DEVELOPER_ID" \
    "$APP"
else
  codesign --force --sign - --entitlements scripts/entitlements.plist "$APP"
fi
```

### Entitlements (`scripts/entitlements.plist`)

| Entitlement | Why |
|---|---|
| `com.apple.security.device.audio-input` | Microphone |
| `com.apple.security.cs.disable-library-validation` | whisper.cpp loads CoreML model bundles at runtime |
| `com.apple.security.cs.allow-jit` | Some GGML/whisper.cpp paths use JIT |
| `com.apple.security.cs.allow-unsigned-executable-memory` | Same reason |
| `com.apple.security.automation.apple-events` | osascript frontmost-app query |

Hardened runtime is on (`--options runtime`).

Accessibility (CGEvent posting via enigo) is granted by the user, not by
entitlement.

## Notarization — conditional

`scripts/notarize.sh` runs only when `APPLE_API_KEY_PATH`, `APPLE_API_KEY_ID`,
and `APPLE_API_KEY_ISSUER_ID` are all set. Uses `notarytool` with API key.

If env vars are absent, the script prints a warning and exits 0 — the DMG is
still produced, just unstapled. This keeps local builds frictionless.

## GitHub release workflow

`.github/workflows/release.yml` triggers on `push: tags: ['v*']`. Runs on
`macos-14` (Apple Silicon runner).

Secrets:
- `APPLE_DEVELOPER_ID` — e.g., `Developer ID Application: Name (TEAMID)`
- `APPLE_CERT_P12_BASE64` — Developer ID certificate, base64-encoded p12
- `APPLE_CERT_PASSWORD` — p12 password
- `APPLE_API_KEY_BASE64` — base64-encoded .p8 file
- `APPLE_API_KEY_ID`
- `APPLE_API_KEY_ISSUER_ID`

Steps:
1. Checkout, install Rust + protoc.
2. If `APPLE_CERT_P12_BASE64` is set: decode p12, import into a temporary
   keychain.
3. If `APPLE_API_KEY_BASE64` is set: decode p8 to a temp file, export
   `APPLE_API_KEY_PATH` to `$GITHUB_ENV`.
4. Run `scripts/package.sh` — produces signed DMG, notarizes inline if creds
   present.
5. Build the bare-binaries tarball (for cargo-install / Nix users — preserves
   the existing path).
6. Upload DMG + SHA256 + tarball + SHA256 as **draft** release assets. User
   reviews and publishes manually.
7. Cleanup the temporary keychain.

## macOS unified logging

Replace `tracing_appender::rolling` in `daemon/src/lib.rs` with an oslog layer.

Crate: `tracing-oslog` v0.3.0. Subsystem string: `com.palicka.vcm` (matches
bundle id). Category `daemon` for the daemon process, `menubar` for the menu
bar app.

```rust
let oslog = tracing_oslog::OsLogger::new("com.palicka.vcm", "daemon");
tracing_subscriber::registry().with(filter).with(oslog).init();
```

File appender is gated by `VCM_LOG_FILE=1` so power users can still get a flat
log file at `~/.local/state/vcm/daemon.log` when debugging.

Whisper logs already route through tracing via
`whisper_rs::install_logging_hooks()`, so they'll flow through oslog
automatically once tracing is re-wired.

Users see logs via:

```bash
log show --predicate 'subsystem == "com.palicka.vcm"' --last 5m
log stream --predicate 'subsystem == "com.palicka.vcm"' --info
```

Or open Console.app and filter by "Subsystem: com.palicka.vcm".

## Platform abstraction

`daemon/src/platform/mod.rs` defines:

```rust
pub trait FrontmostApp { fn name() -> Result<String>; }
pub trait PlatformLogging {
    fn init(subsystem: &str, category: &str, filter: EnvFilter) -> Result<()>;
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{MacOsFrontmostApp as Frontmost, MacOsLogging as Logging};
```

The AppleScript `get_frontmost_app` body moves from `daemon/src/inject.rs` into
`daemon/src/platform/macos.rs::MacOsFrontmostApp::name`. The existing
`#[cfg(not(target_os = "macos"))]` stub that returned empty strings is dropped;
on a non-macOS build the missing impl is a loud compile error (correct
behavior — we don't ship binaries for those platforms yet).

`KeystrokeInjector` stays as-is — `enigo` is the abstraction layer.

What we are NOT abstracting (YAGNI):
- Autostart / launch-at-login (not implemented today).
- App data dirs — `xdg` is already cross-platform via vcm-common.
- Mic permission prompt — triggered implicitly by first audio access; no API
  call to abstract.
- Bundle assembly — bash script, not Rust code.

## Icon pipeline

Icon assets live at `assets/icon/`. Workflow:

1. User provides a 1024x1024 `AppIcon-1024.png` (via the AI prompt in
   `docs/plans/icon-prompt.md`).
2. `scripts/build-icns.sh` generates the iconset and runs `iconutil -c icns`.

For v0.1: if a real icon isn't ready by tag time,
`scripts/make-placeholder-icon.sh` generates a pure-Python placeholder
(squircle gradient + white mic glyph). It's good enough to ship.

## Documentation

New file: `docs/release.md`. Covers Developer ID setup, GitHub secrets,
cutting a release, verifying notarization locally, common failures.

README updates:
- "Install from DMG" section above the "From source" section.
- `vcmctl` symlink for PATH access.
- Console.app / `log stream` log viewing.

## Backwards compatibility

- `cargo install --path .` continues to work — bundle script is additive.
- Existing release tarball workflow is **kept** alongside the DMG. Users on
  Nix install via `cargo install`, not via release artifacts, so no regression.

## Implementation order (followed)

1. Worktree + branch `feat/launch-v0.1.0`.
2. Platform abstraction.
3. oslog wiring (daemon, then menubar).
4. Bundle scripts.
5. Release workflow.
6. Documentation.
7. Verification.
8. PR.
