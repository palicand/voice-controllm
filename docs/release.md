# Releasing VCM

VCM's release workflow opportunistically signs and notarizes. With no Apple secrets set, the CI produces an ad-hoc-signed DMG (end users bypass Gatekeeper once on first launch). Adding the six Apple secrets later upgrades all subsequent releases to fully signed + notarized — no code change required.

## Prerequisites — always required

`assets/icon/AppIcon.icns` must exist. Generate from a 1024×1024 PNG:

```bash
scripts/build-icns.sh path/to/icon-1024.png
git add assets/icon/AppIcon.icns
git commit -m "feat: add app icon"
```

The release workflow fails fast if the icon is missing.

## Prerequisites — required for fully-signed releases (skip until you enroll)

**You need a paid Apple Developer Program membership ($99/year).** Free accounts cannot generate Developer ID Application certificates and cannot use the Notary API. Until you enroll, the workflow will fall back to ad-hoc signing — releases still work, end users just see a Gatekeeper warning.

### Apple Developer enrollment
- Active [Apple Developer Program](https://developer.apple.com/programs/enroll/) membership ($99/year, recurring).
- Create a "Developer ID Application" certificate in [Apple Developer portal → Certificates](https://developer.apple.com/account/resources/certificates).

### Local secret extraction

**Developer ID certificate (.p12):**
1. Open Keychain Access → "login" keychain → Certificates.
2. Right-click `Developer ID Application: Your Name (TEAMID)` → Export → save as `developer-id.p12` with a password.
3. Encode for GitHub:
   ```bash
   base64 -i developer-id.p12 | pbcopy
   ```

**Notarization API key (.p8):**
1. Go to [App Store Connect → Users and Access → Integrations → Team Keys](https://appstoreconnect.apple.com/access/api).
2. Click "+" and create a **Team API Key** (not Personal — Personal keys aren't eligible for the Notary API).
3. Role: **Developer** (sufficient for notarization).
4. Download the .p8 file ONCE — it cannot be re-downloaded. Note the **Key ID** (10-char string) and **Issuer ID** (UUID above the Active table).
5. Encode for GitHub:
   ```bash
   base64 -i AuthKey_XXXXXXXX.p8 | pbcopy
   ```

### GitHub repository secrets (all optional)

Set in Settings → Secrets and variables → Actions → Repository secrets:

| Secret | Value | Required for |
|---|---|---|
| `APPLE_DEVELOPER_ID` | `Developer ID Application: Your Name (TEAMID)` (from Keychain) | Signing |
| `APPLE_CERT_P12_BASE64` | base64 of the .p12 | Signing |
| `APPLE_CERT_PASSWORD` | the password used during Keychain export | Signing |
| `APPLE_API_KEY_BASE64` | base64 of the .p8 | Notarization |
| `APPLE_API_KEY_ID` | Key ID from App Store Connect | Notarization |
| `APPLE_API_KEY_ISSUER_ID` | Issuer ID from App Store Connect | Notarization |

The signing trio and the notarization trio are independent. If you have only the signing secrets you get a signed-but-not-notarized DMG (slightly better than ad-hoc — users still see the same warning but the app identity is verifiable). If you have neither set, you get ad-hoc.

## Cutting a release

1. Verify `main` is in the state you want to ship.
2. Bump `[workspace.package].version` in the root `Cargo.toml` if needed.
3. Tag and push:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
4. Watch the workflow: https://github.com/palicand/voice-controllm/actions
5. Once green, go to Releases → Draft → review the DMG + tarball + checksums.
6. Click "Publish release".

## Local DMG build (for testing — ad-hoc signed)

```bash
cargo install cargo-packager --locked
cargo build --release --target aarch64-apple-darwin
cargo packager --release --target aarch64-apple-darwin
codesign --force --deep --sign - target/aarch64-apple-darwin/release/bundle/macos/VCM.app
```

## Viewing logs

VCM logs to the macOS unified logging system under the subsystem `com.github.palicand.vcm`. View in Console.app (filter by subsystem) or from the terminal:

```bash
log stream --predicate 'subsystem == "com.github.palicand.vcm"' --info
log show --predicate 'subsystem == "com.github.palicand.vcm"' --last 5m
```

To also write logs to a rolling file (dev/troubleshooting):
```bash
VCM_LOG_FILE=1 vcm
# logs go to ~/.local/state/vcm/vcmd.log
```

## Troubleshooting

### "developer cannot be verified" Gatekeeper warning after install
Expected behavior when the release is ad-hoc-signed (no Apple Developer enrollment yet). Users right-click → Open the first time or run `xattr -d com.apple.quarantine /Applications/VCM.app`. The release notes auto-include this instruction when ad-hoc.

To eliminate the warning entirely, set up the six Apple secrets above and cut a new release.

### Notarization fails with "invalid bundle" (only relevant once you're using notarytool)
Inspect the rejection: `xcrun notarytool log <submission-id> --key ... --key-id ... --issuer ...`. The most common cause is a binary inside the bundle that's signed with a different identity or missing hardened runtime.

### Mic permission prompt doesn't appear
The first audio capture triggers the TCC prompt. Ensure `NSMicrophoneUsageDescription` is present in Info.plist (it is in this project's `assets/macos/Info.plist`).

### Accessibility permission needed
Required separately for keystroke injection. macOS will prompt the first time VCM tries to inject text, with a link to System Settings → Privacy & Security → Accessibility. There is no entitlement that grants this; users must approve it manually.
