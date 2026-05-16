# Releasing VCM

How to build, sign, notarize, and publish a VCM release.

## TL;DR

```bash
# Local build (ad-hoc signed, runs on your machine only)
./scripts/package.sh
open dist/VCM.app

# Cut a real release
git tag v0.1.0
git push origin v0.1.0          # triggers .github/workflows/release.yml
# CI builds, signs, notarizes, drafts a GitHub release
# → review the draft on GitHub, publish when ready
```

## Local development

`./scripts/package.sh` always works without any credentials. It builds
`target/aarch64-apple-darwin/release`, assembles `dist/VCM.app`, ad-hoc signs
it (`codesign -s -`), and produces an unsigned DMG.

The ad-hoc-signed `.app` opens on your own machine via `open dist/VCM.app`. It
will *not* open on other machines (Gatekeeper rejects unsigned builds). Use a
real Developer ID for distribution.

Skip the cargo build (re-package an existing build) with `VCM_SKIP_BUILD=1 ./scripts/package.sh`.

## One-time setup for signed releases

You need three Apple artifacts: a Developer ID Application certificate, an App
Store Connect API key for notarization, and your team's identifier string.

### 1. Developer ID certificate (.p12)

Get a Developer ID Application certificate (NOT Developer ID Installer). From
Apple Developer → Certificates, Identifiers & Profiles, create a Developer ID
Application certificate. Download the `.cer`, install into Keychain Access,
then export as `.p12` with a password.

Base64-encode it for the GitHub secret:

```bash
base64 -i DeveloperID.p12 -o cert.p12.base64
```

### 2. Notarization API key (.p8)

App Store Connect → Users and Access → Keys → "App Store Connect API" tab.
Generate a key with "Developer" role (sufficient for notarization). Download
the `.p8` once — you can't re-download it.

You also need the Key ID (10-character string shown next to the key) and the
Issuer ID (UUID at the top of the Keys page).

Base64-encode the `.p8`:

```bash
base64 -i AuthKey_XXXXXXXXXX.p8 -o api-key.p8.base64
```

### 3. Find your Developer ID string

```bash
security find-identity -v -p codesigning
# Look for: "Developer ID Application: Your Name (TEAMID12)"
```

The exact string between the quotes is what you set as `APPLE_DEVELOPER_ID`.

## GitHub secrets

Set in the repo's Settings → Secrets and variables → Actions:

| Secret | Value |
|---|---|
| `APPLE_DEVELOPER_ID` | `Developer ID Application: Your Name (TEAMID12)` |
| `APPLE_CERT_P12_BASE64` | contents of `cert.p12.base64` |
| `APPLE_CERT_PASSWORD` | password used when exporting the .p12 |
| `APPLE_API_KEY_BASE64` | contents of `api-key.p8.base64` |
| `APPLE_API_KEY_ID` | 10-char Key ID from App Store Connect |
| `APPLE_API_KEY_ISSUER_ID` | Issuer UUID from App Store Connect |

If any of these are missing, the release workflow still runs — it just
produces an unsigned/un-notarized DMG, which is fine for testing the pipeline.

## Local signed builds

Once your local Keychain holds the Developer ID certificate:

```bash
export APPLE_DEVELOPER_ID="Developer ID Application: Your Name (TEAMID12)"
export APPLE_API_KEY_PATH="$HOME/AppleKeys/AuthKey_XXXXXXXXXX.p8"
export APPLE_API_KEY_ID="XXXXXXXXXX"
export APPLE_API_KEY_ISSUER_ID="00000000-0000-0000-0000-000000000000"
./scripts/package.sh
```

Outputs `dist/VCM.app`, `dist/VCM-<version>-aarch64.dmg` (signed, notarized,
stapled), and `dist/VCM-<version>-aarch64.dmg.sha256`.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` under `[workspace.package]`.
2. `cargo build` to update `Cargo.lock`. Commit.
3. Open a PR, merge to main.
4. Tag from `main`:

   ```bash
   git checkout main && git pull
   git tag v0.1.0
   git push origin v0.1.0
   ```

5. CI runs `.github/workflows/release.yml`. It produces a *draft* release.
   Review the assets:

   - `VCM-0.1.0-aarch64.dmg` — signed/notarized DMG (verify via
     `xcrun stapler validate`)
   - `VCM-0.1.0-aarch64.dmg.sha256`
   - `vcm-v0.1.0-aarch64-apple-darwin.tar.gz` — bare-binaries tarball (for
     `cargo install` / Nix users)
   - `vcm-v0.1.0-aarch64-apple-darwin.tar.gz.sha256`

6. Edit release notes if needed, then publish.

## Verifying the release locally

After downloading the DMG:

```bash
# Mount, copy, unmount
hdiutil attach VCM-0.1.0-aarch64.dmg
cp -R /Volumes/VCM\ 0.1.0/VCM.app /Applications/
hdiutil detach /Volumes/VCM\ 0.1.0

# Verify Gatekeeper accepts it
spctl --assess --type exec --verbose /Applications/VCM.app
# Expected: "accepted, source=Notarized Developer ID"

# Launch
open /Applications/VCM.app

# Stream logs
log stream --predicate 'subsystem == "com.palicka.vcm"' --info
```

## Architecture notes

- v0.1 ships **aarch64-only**. CoreML acceleration is Apple Silicon only; an
  x86_64 build would be substantially slower and isn't worth the maintenance
  burden for the launch.
- Hardened runtime is on, with entitlements for microphone, JIT, library
  validation disabled (whisper.cpp needs this), and Apple Events (osascript
  reads frontmost app).
- Accessibility permission (keystroke injection) is granted by the user via
  System Settings → Privacy & Security → Accessibility. There's no entitlement
  to pre-grant this.
- The bundle ships all three binaries (`vcm`, `vcmd`, `vcmctl`). `vcm` is the
  menubar launcher. `vcmctl` is available for CLI usage; users can symlink it
  to `/usr/local/bin/vcmctl` if they want a global command.

## Common notarization failures

- **"The signature does not include a secure timestamp"** — re-sign with
  `--timestamp`. The script already does this when `APPLE_DEVELOPER_ID` is
  set.
- **"The executable does not have the hardened runtime enabled"** — re-sign
  with `--options runtime`. Same.
- **"The binary uses an SDK older than..."** — rebuild against a newer macOS
  SDK on CI. Bump `runs-on` to a more recent runner.
- **Notarization stuck "In Progress"** — Apple is slow sometimes.
  `xcrun notarytool history --key ...` to monitor.
