# Release setup

One-time setup for signed, notarized, auto-updating macOS releases. The
release pipeline (`.github/workflows/release.yml`) is already in place; it
becomes operational the moment the secrets below are populated. Without
them the pipeline still runs and produces unsigned artifacts, but the
signing and notarization steps are no-ops.

References in code:

- `src/tauri/src/app/updater.rs` (header doc comment points here)
- `.github/workflows/release.yml` (consumes every secret in this file)
- `src/tauri/tauri.conf.json` (`plugins.updater.pubkey` is populated from
  the minisign keypair generated in step 1)

## 1. Minisign keypair for updater signing

The Tauri updater verifies downloaded `.app.tar.gz` archives against a
minisign public key embedded in the app at build time. Lose the private
key and we cannot ship updates. Leak it and an attacker can ship
malicious updates to every installed app.

Generate the keypair once on a trusted maintainer machine:

```bash
mkdir -p ~/.tauri
cd src/tauri
cargo tauri signer generate -w ~/.tauri/opnble.key
# Enter a strong passphrase twice. Store it in 1Password.
```

This produces two files under `~/.tauri/`:

| File | Shape | Where it goes |
|------|-------|---------------|
| `opnble.key` | 2-line ASCII, encrypted private key | GitHub Secret `TAURI_SIGNING_PRIVATE_KEY` (paste full file contents) |
| `opnble.key.pub` | 2-line ASCII, public key | `src/tauri/tauri.conf.json` `plugins.updater.pubkey` (paste full file contents, including the `untrusted comment:` line and trailing newline; JSON escapes are fine) |

The passphrase goes into the GitHub Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## 2. Apple Developer ID Application certificate

For Gatekeeper-friendly macOS distribution.

1. On a Mac, open **Keychain Access**: menu **Certificate Assistant → Request a Certificate From a Certificate Authority…**
   - Email address: your release Apple ID.
   - "CA Email" blank.
   - **Saved to disk** + **Let me specify key pair information** (2048-bit RSA).
   - Save the `.certSigningRequest` file.

2. At <https://developer.apple.com/account> → **Certificates, IDs & Profiles → Certificates → +**:
   - Type: **Developer ID Application** (NOT "Apple Distribution"; that one is for the Mac App Store).
   - Upload the CSR. Download `developerID_application.cer`.

3. Double-click the `.cer` to install. In **Keychain Access → My Certificates**, find the row labeled `Developer ID Application: <Display Name> (<TEAM_ID>)`. Right-click → **Export Items…** → format **Personal Information Exchange (.p12)** → set a strong export password.

4. Convert to single-line base64 for GitHub Secrets:

   ```bash
   openssl base64 -A -in developerID_application.p12 -out cert.b64
   pbcopy < cert.b64   # macOS: copy to clipboard
   ```

5. Find your **Team ID** at <https://developer.apple.com/account> → Membership details. 10-character alphanumeric.

## 3. App-specific password for Apple notarization

Notarization requires an Apple ID that has access to the Developer team
plus an app-specific password (not the Apple ID password).

1. At <https://appleid.apple.com> → **Sign-In and Security → App-Specific Passwords → Generate**.
2. Label it `opnble-notary-ci`. Apple shows the password once; format is `xxxx-xxxx-xxxx-xxxx`.

## 4. GitHub Secrets to populate

Repository **Settings → Secrets and variables → Actions**, or Cursor Dashboard → Cloud Agents → Secrets (the cloud agents inject them into the same place):

| Secret | Source |
|--------|--------|
| `TAURI_SIGNING_PRIVATE_KEY` | Full file contents of `~/.tauri/opnble.key` (two lines, including the `untrusted comment:` header) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The passphrase you set in step 1 |
| `APPLE_CERTIFICATE` | Single-line base64 from step 2.4 |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` export password from step 2.3 |
| `APPLE_SIGNING_IDENTITY` | Full identity string: `Developer ID Application: <Display Name> (<TEAM_ID>)` |
| `APPLE_ID` | Apple ID email used for notarization (step 3) |
| `APPLE_PASSWORD` | App-specific password from step 3 |
| `APPLE_TEAM_ID` | 10-character Team ID from step 2.5 |

The release workflow detects whether each set is populated:

- `HAS_APPLE_CERT = (APPLE_CERTIFICATE != "")` controls codesign + sanity-check.
- `HAS_APPLE_NOTARY = (APPLE_ID && APPLE_PASSWORD && APPLE_TEAM_ID)` controls notarytool + stapler.

Until both are set, the workflow runs end-to-end and produces an
unsigned `.dmg` for structural verification. The moment all eight
secrets are populated, the next tag (or `workflow_dispatch`) produces
a Gatekeeper-friendly, auto-updating release.

## 5. Verifying a signed build locally

After downloading a `.dmg` from a release run, on a Mac:

```bash
hdiutil attach -nobrowse opnble_*.dmg
APP="/Volumes/opnble/opnble.app"

codesign -dvvv --entitlements :- "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
spctl -a -t exec -vv "$APP"            # Gatekeeper assessment
xcrun stapler validate "$APP"          # Notary ticket
codesign -d --entitlements :- "$APP/Contents/MacOS/vfkit" \
  | grep com.apple.security.virtualization

hdiutil detach "$(dirname "$APP")"
```

All five commands should succeed with no warnings. The vfkit entitlement
grep in particular is the failure mode that would block VM startup even
with a passing notary submission, so it's worth running explicitly.

## 6. Key rotation

The minisign keypair is the one piece of long-lived state with no
escape hatch: clients trust whichever pubkey was embedded in the
version they have installed. To rotate:

1. Generate a new keypair (`cargo tauri signer generate -w ~/.tauri/opnble-vN.key`).
2. Ship a release built with the **new** pubkey in `tauri.conf.json`,
   still signed by the **old** private key in `TAURI_SIGNING_PRIVATE_KEY`.
3. Wait for adoption (watch the Cloudflare worker logs for hits to
   `/updates/latest.json` from older versions).
4. Once adoption is high enough, swap `TAURI_SIGNING_PRIVATE_KEY` to the
   new private key. Old-version clients can no longer be updated; they
   reinstall from a fresh `.dmg`.

The Apple Developer ID certificate has a 5-year expiration and is
swappable: generate a new cert, export, update `APPLE_CERTIFICATE` +
`APPLE_CERTIFICATE_PASSWORD` + `APPLE_SIGNING_IDENTITY`. The next
release picks up the new identity automatically.

## 7. VM image hosting (Phase 2)

The macOS app needs a ~4 GiB Alpine VM image at runtime, which is too
large for GitHub release assets (2 GiB cap). Phase 2 hosts the image
on Cloudflare R2 and serves it through the worker at `openable.dev/vm/`.

One-time bucket setup:

```bash
wrangler r2 bucket create opnble-vm
```

After `wrangler deploy` picks up the `[[r2_buckets]]` binding in
`src/worker/wrangler.toml`, the worker exposes:

- `GET /vm/manifest.json` -> small JSON describing the current image.
- `GET /vm/<file>.img.zst` -> the compressed image payload.

Both 404 until the bucket has objects.

To publish a new image:

```bash
make vm-image                                       # build alpine-base.img locally
./scripts/upload-vm-image.sh 2026-06-02-alpine-3.20   # compress + upload + manifest
curl -s https://openable.dev/vm/manifest.json | jq .  # sanity-check
```

`scripts/upload-vm-image.sh` compresses with zstd, hashes both the
compressed and decompressed forms, uploads the image and a manifest to
R2 via `wrangler r2 object put`. The manifest schema is:

```json
{
  "schemaVersion": 1,
  "imageVersion": "<slug>",
  "url": "https://openable.dev/vm/alpine-base-<slug>.img.zst",
  "compression": "zstd",
  "sha256": "<sha of compressed>",
  "uncompressedSha256": "<sha of raw image>",
  "compressedSize": 0,
  "uncompressedSize": 0,
  "minAppVersion": "0.2.0",
  "notes": ""
}
```

The host crate (Phase 2 Rust client, follow-up PR) reads
`/vm/manifest.json` at first launch, compares `imageVersion` to the
locally installed image, and either skips the download or streams it
with progress + SHA verification before unpacking to
`~/.opnble/vm/alpine-base.img`.

## 8. Beta-channel rollout

When Phase 4 (`/updates/beta/latest.json` worker route) lands, the same
secrets cover both channels. A maintainer chooses which channel to
publish by the tag they push:

- `v0.2.0` → stable channel.
- `v0.2.1-beta.1` → beta channel. The workflow auto-flags the GitHub
  release as a prerelease (Phase 0); the worker route forwards beta
  clients to that release.

No second keypair, no second cert. The trust root is the same.
