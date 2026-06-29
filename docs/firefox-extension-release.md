# Starlee Firefox Extension Release Notes

This document tracks the Firefox-specific release path for the Starlee browser extension. It is intentionally separate from Chrome and Safari packaging docs so Firefox review work does not change those targets.

## Target Decision

- Launch target: Firefox desktop WebExtension using Manifest V3.
- Package output: `release/firefox-extension/starlee-firefox-extension-<version>.zip`.
- Build output: `sensor/dist/firefox-extension`.
- Shared source: `sensor/src` remains the shared extension implementation.
- Firefox-specific source: `sensor/extension/manifest.firefox.json`, Firefox package scripts, Firefox package inspector, and Firefox target tests.

Firefox MV3 does not accept a service-worker-only background in the tested Firefox runtime. The Firefox target therefore uses a Gecko MV3 event-page background (`background.scripts`) while Chrome keeps its service-worker manifest. Immediate menu-bar polling is smoke-tested in Firefox, but a listed AMO launch still needs a long-lifecycle run proving polling survives background suspension for at least 30 minutes. The product-safe Firefox fallback is toolbar capture.

## Merge Readiness

Current recommendation: merge-ready as an additive Firefox target, not AMO launch-ready.

Automated coverage is in place for the Firefox build target, package hygiene,
article payload shape, selected-text attachment, YouTube metadata, rendered
transcript segments, transcript-unavailable state, browser adapter behavior, and
local bridge error mapping. A real Firefox temporary-add-on smoke also proves
rendered article extraction, selected-text article capture, toolbar/in-page
capture, immediate menu-bar request polling, local bridge posting, and diagnostic
redaction. This is enough to merge the additive Firefox target if code review is
comfortable with the documented launch blockers below.

What works by automated verification:

- Chrome remains the default build target.
- Firefox build writes to `sensor/dist/firefox-extension`.
- Firefox package uses `sensor/extension/manifest.firefox.json`.
- Firefox manifest uses `background.scripts` for Gecko MV3 compatibility while
  Chrome keeps `background.service_worker`.
- Article extraction reuses the shared Readability path and emits the same
  payload fields as Chrome.
- Selected text is attached only to article payloads.
- YouTube capture reuses the shared metadata/transcript path and preserves
  transcript segment shape `{ t: seconds, text: string }`.
- Local bridge helpers return `token_missing`, `token_invalid`,
  `service_down`, and `payload_too_large` without leaking page bodies or tokens.
- A live Firefox smoke with a temporary add-on posts article captures to a mock
  `127.0.0.1` bridge and verifies diagnostics omit article body, selected text,
  and token material.

What still requires manual Firefox testing before AMO/listed production launch:

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
FIREFOX_BIN=/path/to/firefox node scripts/firefox-smoke.mjs

cd ..
./scripts/package-firefox-extension.sh
./scripts/inspect-firefox-extension-package.sh \
  release/firefox-extension/starlee-firefox-extension-0.1.0.zip
```

The package inspector verifies:

- Required extension files are present.
- `build-info.json` identifies the `firefox` target.
- `background.scripts` references `background.js` for Gecko MV3 compatibility.
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
- `alarms`: wakes the Firefox background context to check for pending local menu-bar capture requests.
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
