# Starlee Firefox Extension Release Notes

This document tracks the Firefox-specific release path for the Starlee browser extension. It is intentionally separate from Chrome and Safari packaging docs so Firefox review work does not change those targets.

## Target Decision

- Launch target: Firefox desktop WebExtension using Manifest V3.
- Package output: `release/firefox-extension/starlee-firefox-extension-<version>.zip`.
- Build output: `sensor/dist/firefox-extension`.
- Shared source: `sensor/src` remains the shared extension implementation.
- Firefox-specific source: `sensor/extension/manifest.firefox.json`, Firefox package scripts, Firefox package inspector, and Firefox target tests.

Firefox MV3 service worker behavior still needs live-browser validation for menu-bar capture polling before a listed AMO launch. The package includes the same local polling path used by the shared background runtime, but the product-safe Firefox fallback is toolbar capture until a clean Firefox profile proves menu-bar polling survives worker suspension for at least 30 minutes.

## Build and Package

```sh
cd sensor
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
- `optional_host_permissions: http://*/*, https://*/*`: allows rendered-page capture when the user grants page access.

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
