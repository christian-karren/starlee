# Local Safari Web Extension Setup

Starlee's Safari path reuses the same browser sensor source as Chrome. The
extension extracts rendered article text, metadata, selected text, and YouTube
transcripts from pages you choose to save, then sends the payload only to the
local Starlee service at `http://127.0.0.1:47291`.

## What this does and does not require

For your own Mac, you can run a Safari Web Extension locally without App Store
review. Safari Web Extensions are packaged inside a small macOS app, so local
development uses Xcode and Apple's `safari-web-extension-converter`.

This repo can build the reusable WebExtension package with Node alone. To
generate and run the Safari macOS wrapper, install full Xcode and select it with
`xcode-select`; Apple's converter is not included with Command Line Tools alone.

Apple's Safari extension docs describe this architecture: web extensions from
other browsers can be converted into an Xcode project containing a macOS app and
Safari extension, and distribution for other users goes through Apple's
extension distribution paths.

## Coordination Boundaries

Safari parity uses Chrome capture behavior as the source of truth. Edit the
shared browser sensor only when a Safari Web Extension API gap cannot be handled
in Safari-specific packaging or adapter code. Do not hand-edit generated files
under `release/safari-extension`; rerun the scripts instead.

Generated Xcode wrapper ownership for local development:

- `release/safari-extension/StarleeSafari` is generated output.
- `release/safari-extension/extension` is the converter input copy used for the
  wrapper.
- Neither directory should be committed for local parity work.
- Before public distribution, the team must decide whether to keep generating the
  wrapper, check in a curated wrapper project, or maintain a template.

## Build the local Safari package

```sh
./scripts/package-safari-extension.sh
```

The script:

1. builds the shared Starlee sensor;
2. stages a clean Safari Web Extension source folder;
3. strips source maps and generated local config;
4. writes `release/safari-extension/starlee-safari-web-extension-0.1.0.zip`;
5. inspects that ZIP before any local development config is copied into the
   staged source folder;
6. if Apple's converter exists, generates an Xcode project at
   `release/safari-extension/StarleeSafari`;
7. validates the generated wrapper bundle identifiers.

Inspect the package:

```sh
./scripts/inspect-safari-extension-package.sh \
  release/safari-extension/starlee-safari-web-extension-0.1.0.zip
```

The inspection gate fails if it finds local configuration, vault data, model
files, source maps, obvious bearer tokens, or unexpected remote fetch
destinations.

The release ZIP must not contain `starlee-config.json`. For local development
only, the package script may copy
`${STARLEE_SAFARI_LOCAL_CONFIG:-$HOME/Starlee/sensor-extension/starlee-config.json}`
into the staged source folder after the ZIP has already been written and
inspected. That lets the generated wrapper connect to local Starlee without
putting the token in the release candidate.

## If the converter is missing

Install full Xcode from Apple, open it once, then select it:

```sh
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
```

Then rerun:

```sh
./scripts/package-safari-extension.sh
```

To make converter absence fail in CI or release checks:

```sh
STARLEE_REQUIRE_SAFARI_CONVERTER=1 ./scripts/package-safari-extension.sh
```

If you need to point at a specific converter binary:

```sh
SAFARI_WEB_EXTENSION_CONVERTER=/Applications/Xcode.app/Contents/Developer/usr/bin/safari-web-extension-converter \
  ./scripts/package-safari-extension.sh
```

## Run locally in Safari

The install script performs the package, Xcode build, app copy, and `pluginkit`
registration steps:

```sh
./scripts/install-safari-extension.sh
```

Expected local artifacts:

```text
release/safari-extension/StarleeSafari/Starlee Safari/Starlee Safari.xcodeproj
release/safari-extension/DerivedData/Build/Products/Release/Starlee Safari.app
~/Applications/Starlee Safari.app
~/Applications/Starlee Safari.app/Contents/PlugIns/Starlee Safari Extension.appex
```

The extension identifier is:

```text
com.starlee.capture.safari.Extension
```

Confirm registration:

```sh
pluginkit -m -A -D -i com.starlee.capture.safari.Extension
```

Then finish the Safari user-approval steps:

1. Open Safari.
2. Open Safari Settings > Extensions.
3. Enable Starlee.
4. Grant Starlee access on the sites you want to save.
5. Reload the active tab after granting access.
6. Make sure Starlee's local service is running:

   ```sh
   starlee serve
   ```

7. Open an article in Safari and use the Starlee extension button, the in-page
   Starlee button, or the Starlee macOS menu-bar icon.

When using the macOS menu-bar icon, Safari must already have a fresh extension
check-in with the local Starlee service. The extension polls
`http://127.0.0.1:47291/capture-request`, looks up Safari's active tab, probes
the page content script, extracts the payload, posts `/capture`, then posts
`/capture-request/result`. If Safari has not checked in recently, cannot expose
the active tab, or cannot message the content script, Starlee should show an
actionable "needs attention" message instead of a generic red X.

Verify from Starlee:

```sh
starlee doctor
starlee recent
starlee search "words from the article you saved"
```

## Smoke Checklist

Run these before treating the local Safari path as working:

- [ ] `./scripts/package-safari-extension.sh` exits 0.
- [ ] `./scripts/inspect-safari-extension-package.sh release/safari-extension/starlee-safari-web-extension-0.1.0.zip` exits 0.
- [ ] `./scripts/install-safari-extension.sh` exits 0 on a Mac with full Xcode.
- [ ] `pluginkit -m -A -D -i com.starlee.capture.safari.Extension` lists the extension.
- [ ] Safari Settings > Extensions shows Starlee enabled.
- [ ] Safari prompts for site access on a test article page, and capture succeeds after access is granted and the page is reloaded.
- [ ] A normal article capture appears in `starlee recent`.
- [ ] Selected text on an article is saved in the capture payload.
- [ ] A YouTube watch page with captions saves transcript segments.
- [ ] A YouTube watch page without captions saves a metadata-only record with explicit transcript status/reason.
- [ ] Denying site access produces `permission_denied` or `content_script_unreachable` with a next action to grant site access and reload.
- [ ] Clicking the macOS menu-bar icon on a Safari article either saves the page or shows a non-generic setup/site-access/check-in message.
- [ ] Stopping `starlee serve` produces `service_down` or local-service recovery text.
- [ ] `starlee diagnostics --last-capture` contains browser `Safari` and does not contain capture tokens, article bodies, transcript text, selected text, raw HTML, cookies, embeddings, or vault file bodies.

## Uninstall Local Wrapper

For a clean reinstall:

```sh
pkill -f "$HOME/Applications/Starlee Safari.app/Contents/MacOS/Starlee Safari" || true
pluginkit -r "$HOME/Applications/Starlee Safari.app/Contents/PlugIns/Starlee Safari Extension.appex" || true
rm -rf "$HOME/Applications/Starlee Safari.app"
```

Then rerun:

```sh
./scripts/install-safari-extension.sh
```

## Distribution Implications

Local Safari parity is not the same as public distribution.

- Local development: full Xcode, generated wrapper, manual Safari Settings
  enablement, no App Store review.
- Signed/notarized direct distribution: Developer ID signing, hardened runtime
  decision, notarization, stapling, and Gatekeeper verification.
- Mac App Store distribution: Apple Developer account, App Sandbox and network
  client entitlement review, Safari Web Extension capability, bundle ID
  ownership, provisioning profile, privacy labels, screenshots, review copy, and
  App Store review.

Before public distribution, audit the generated wrapper entitlements and signing
settings after conversion. The release copy must state that Starlee reads only
pages the user chooses to save and sends captures to the local Starlee app on the
user's Mac.

## Product tradeoff

Safari is a good fit for Starlee's Mac-first identity, but the extension is
still a browser permission surface. The advantage is that it shares most of the
same JavaScript sensor code as Chrome, so Starlee does not need separate article
extraction engines per browser.
