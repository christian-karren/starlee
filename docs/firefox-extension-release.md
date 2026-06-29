# Starlee Firefox Extension Release Notes

This document tracks the Firefox-specific release path for the Starlee browser extension. It is intentionally separate from Chrome and Safari packaging docs so Firefox review work does not change those targets.

## Target Decision

- Launch target: Firefox desktop WebExtension using Manifest V3.
- Package output: `release/firefox-extension/starlee-firefox-extension-<version>.zip`.
- Build output: `sensor/dist/firefox-extension`.
- Shared source: `sensor/src` remains the shared extension implementation.
- Firefox-specific source: `sensor/extension/manifest.firefox.json`, Firefox package scripts, Firefox package inspector, and Firefox target tests.

Firefox MV3 service worker behavior still needs live-browser validation for menu-bar capture polling before a listed AMO launch. The package includes the same local polling path used by the shared background runtime, but the product-safe Firefox fallback is toolbar capture until a clean Firefox profile proves menu-bar polling survives worker suspension for at least 30 minutes.

## Merge Readiness

Current recommendation: merge after manual Firefox smoke.

Automated coverage is in place for the Firefox build target, package hygiene,
article payload shape, selected-text attachment, YouTube metadata, rendered
transcript segments, transcript-unavailable state, browser adapter behavior, and
local bridge error mapping. This makes the branch suitable for a human Firefox
smoke pass, not a claim that AMO/listed production launch is complete.

What works by automated verification:

- Chrome remains the default build target.
- Firefox build writes to `sensor/dist/firefox-extension`.
- Firefox package uses `sensor/extension/manifest.firefox.json`.
- Article extraction reuses the shared Readability path and emits the same
  payload fields as Chrome.
- Selected text is attached only to article payloads.
- YouTube capture reuses the shared metadata/transcript path and preserves
  transcript segment shape `{ t: seconds, text: string }`.
- Local bridge helpers return `token_missing`, `token_invalid`,
  `service_down`, and `payload_too_large` without leaking page bodies or tokens.

What still requires manual Firefox testing:

- Temporary add-on install in a clean Firefox desktop profile.
- Toolbar article capture posting to a local Starlee service.
- Toolbar selected-text capture posting to a local Starlee service.
- Toolbar YouTube transcript capture against a real video page.
- Toolbar YouTube transcript-unavailable behavior against a real or controlled
  no-transcript page.
- A 30-minute MV3 lifecycle run proving menu-bar polling still receives
  `/capture-request` after worker suspension.
- Runtime network inspection confirming captured content only posts to
  `127.0.0.1`.

## Build and Package

```sh
cd sensor
npm test
npm run test:integration
npm run build
node scripts/build.mjs --target firefox

cd ..
./scripts/package-firefox-extension.sh
./scripts/inspect-firefox-extension-package.sh \
  release/firefox-extension/starlee-firefox-extension-0.1.0.zip
```

The package inspector verifies:

- Required extension files are present.
- `build-info.json` identifies the `firefox` target.
- `host_permissions` are limited to `http://127.0.0.1/*`.
- Static content-script matches include `http://*/*`, `https://*/*`, and
  YouTube watch-page hosts so rendered article and transcript extraction can run.
- No `starlee-config.json`, capture token, vault data, SQLite file, model file, sourcemap, or `node_modules` directory is bundled.
- No bundled source contains non-local `fetch("http...")` destinations.

## AMO Review Notes

Single purpose:

Starlee saves rendered articles and YouTube transcript metadata from the active browser tab into the user's local Starlee vault.

Data handling:

Captured article text, transcript text, page metadata, URLs, and the capture token are sent only to the local Starlee service at `http://127.0.0.1:47291`. The extension does not send captured content to Starlee servers or third-party services.

Permissions:

- `storage`: stores the local capture token, port, and non-sensitive status codes.
- `activeTab`: captures the current tab after user interaction.
- `tabs`: locates the active tab for toolbar and menu-bar initiated capture.
- `alarms`: wakes the background worker to check for pending local menu-bar capture requests.
- `host_permissions: http://127.0.0.1/*`: communicates with the local Starlee service.
- Static `content_scripts` on `http://*/*`, `https://*/*`, and YouTube hosts:
  lets Starlee read the rendered page when the user invokes capture. This page
  access is required for article extraction, selected-text capture, and rendered
  transcript capture. Captured content is still posted only to the local Starlee
  service.

## Manual QA

Run before AMO submission:

- Clean Firefox profile installs the signed or temporary package.
- Options page accepts token and port without displaying the stored token.
- `POST /extension/hello` records browser name `Firefox`.
- Service-down and token-invalid states are visible in options.
- Toolbar article capture saves a local record.
- Toolbar selected-text article capture includes selected text.
- Toolbar YouTube capture saves transcript segments when rendered.
- Menu-bar request pickup succeeds for a 30-minute Firefox MV3 lifecycle run, or release notes mark menu-bar capture as unavailable for Firefox with toolbar fallback.
- Extension update preserves stored token and port.
- Runtime network inspection shows no captured content leaving `127.0.0.1`.
