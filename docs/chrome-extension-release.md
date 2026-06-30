# Starlee Chrome extension release notes

Starlee Capture is a Manifest V3 Chrome extension that acts as a local browser
sensor. It extracts the article or YouTube transcript the user chooses to save
and sends that payload only to the local Starlee service at
`http://127.0.0.1:47291`.

For v1, this Chrome extension is the only supported browser capture release
target. Firefox and Safari are future targets and must not be represented in
onboarding, store copy, or production diagnostics as available v1 capture paths.
See [`docs/chrome-capture-v1-baseline.md`](chrome-capture-v1-baseline.md).

## Permission decision

MVP keeps a persistent content script on `http://*/*` and `https://*/*` plus
`storage`, `activeTab`, `tabs`, and `alarms`.

This is intentionally broader than a pure `activeTab` extension. The reason is
the Starlee Mac menu-bar button: a click in the native menu bar is not a Chrome
toolbar click, so it does not reliably grant Chrome's temporary `activeTab`
permission. The content script keeps the page extractor ready for both capture
surfaces:

- the Chrome toolbar button;
- the in-page Starlee button;
- the Mac menu-bar "Save Current Article" action.

The privacy boundary is still narrow: the only declared network host permission
is `http://127.0.0.1/*`, and the extension package must not contain remote
telemetry, remote code, local vault data, model files, capture tokens, or local
configuration.

Future releases should revisit a hybrid mode where toolbar capture uses
programmatic injection and menu-bar capture requests prompt the user for
optional host access.

## Build the Chrome Web Store package

```sh
./scripts/package-chrome-extension.sh
```

The script:

1. generates PNG extension icons;
2. runs the sensor build;
3. stages only `sensor/dist/extension`;
4. removes source maps;
5. removes any generated `starlee-config.json`;
6. verifies source manifest, built manifest, and `sensor/package.json` versions match;
7. verifies the built extension has Manifest V3, `background.js`, `content.js`,
   `options.html`, `options.js`, and `build-info.json`;
8. includes `build-info.json` with git/build identity;
9. writes a ZIP to `release/chrome-extension/starlee-capture-<version>.zip`.

Inspect the package before upload:

```sh
./scripts/inspect-chrome-extension-package.sh \
  release/chrome-extension/starlee-capture-0.1.0.zip
```

The inspection gate fails if it finds:

- `config.json` or `starlee-config.json`;
- SQLite vault/index files;
- model files;
- `node_modules`;
- source maps;
- obvious bearer-token material;
- unexpected remote HTTP(S) URLs.
- a non-MV3 manifest;
- a package filename that does not match the manifest version;
- a changed Chrome permission or host-permission set;
- missing options page, service worker, content script, build identity, or PNG icons.

On success, the inspection command prints JSON with the package path, manifest
version, build identity, build timestamp, and file count. Keep that output with
the release notes for the Chrome Web Store candidate.

## Chrome release QA gate

`make test` is not enough for a Chrome Web Store upload candidate. It runs
`cd sensor && npm install && npm run build && npm test`, and `npm test` is
limited to `node --test 'test/*.test.js'`. That means it does not run:

- `sensor/test/e2e/extension.test.js`;
- `sensor/test/integration/handoff-loop.test.js`.

For a Chrome Web Store candidate, run the Chrome release gate:

```sh
cd sensor
npm run test:chrome-release
```

Equivalent expanded commands:

```sh
cd sensor
npm run build
npm test
npm run test:integration
npm run test:e2e
```

The E2E test loads `sensor/dist/extension` into Chromium and binds its mock
capture service on an ephemeral `127.0.0.1` port so it does not collide with a
running Starlee service on `47291` or another local test run.

Block Chrome Web Store upload if any of these commands fail:

```sh
cd sensor && npm run test:chrome-release
./scripts/package-chrome-extension.sh
./scripts/inspect-chrome-extension-package.sh \
  release/chrome-extension/starlee-capture-0.1.0.zip
```

Archive the test output and inspection JSON with the release candidate.

## Store listing draft

Short description:

> Save rendered articles and YouTube transcripts to your local Starlee brain.

Long description:

> Starlee Capture lets you save the article or YouTube transcript you are reading
> into Starlee, a local-first digital brain on your Mac. Click the Starlee
> toolbar button, the in-page save button, or the Starlee menu-bar app; the
> extension extracts readable text and metadata from the active tab and sends it
> to the Starlee app running locally on your computer.
>
> Starlee Capture does not upload article bodies, transcripts, browsing history,
> vault data, or capture tokens to Starlee servers. The extension communicates
> with `127.0.0.1`, your own computer, where the Starlee local service stores and
> indexes captures in `~/Starlee`.

Permission justification:

- `storage`: stores the local capture token, port, and redacted connection state.
- `activeTab`: captures the tab the user chooses from the toolbar.
- `tabs`: finds the active browser tab when the Mac menu-bar app requests a save.
- `alarms`: performs low-frequency local polling for menu-bar capture requests.
- `http://127.0.0.1/*`: talks to the Starlee service running on the user's Mac.
- `http://*/*` and `https://*/*` content scripts: keep the extractor available
  for the native menu-bar one-click flow.

Single-purpose statement:

> Starlee Capture saves the rendered article or YouTube transcript the user is
> viewing into the local Starlee app running on that user's Mac.

Privacy disclosure:

> Starlee Capture sends captured article text, selected text, metadata, and
> YouTube transcript data only to `127.0.0.1`, the user's own computer, where
> the Starlee app stores captures in `~/Starlee`. The extension does not sell,
> share, or upload article bodies, transcripts, browsing history, vault data, or
> capture tokens to Starlee servers.

Reviewer notes:

> This extension depends on the native Starlee app or `starlee serve` running
> locally on macOS. All capture endpoints are loopback-only and authenticated
> with a locally generated bearer token. Broad `http://*/*` and `https://*/*`
> content script access is used so the native macOS menu-bar capture button can
> save the active browser page; a native menu-bar click does not grant Chrome's
> temporary `activeTab` permission.

Required screenshots before submission:

- Chrome toolbar button on an article page.
- Extension options page showing connected local Starlee state.
- macOS menu-bar "Save Current Article" flow.
- Saved result or recent capture in Starlee.

Chrome compatibility matrix for launch:

| Browser | Launch claim | Required checks |
| --- | --- | --- |
| Chrome stable on macOS | Required | Install, options handshake, toolbar article capture, toolbar YouTube capture, menu-bar article capture, diagnostics |
| Arc | Mention only if tested | Install, toolbar article capture, menu-bar article capture |
| Brave | Mention only if tested | Install, toolbar article capture, menu-bar article capture |
| Edge | Mention only if tested | Install, toolbar article capture, menu-bar article capture |

## Manual Chrome Web Store steps

1. Register or open the Chrome Web Store Developer Dashboard.
2. Create a new extension item.
3. Upload the ZIP generated by `./scripts/package-chrome-extension.sh`.
4. Complete the privacy fields using the local-only disclosure above.
5. Add screenshots, category, support contact, and privacy policy URL/page.
6. Submit first as an unlisted beta.
7. Attach or retain the package inspection JSON and Chrome release QA output.
8. After approval, install on a clean Chrome profile and verify:
   - extension options page says connected;
   - `starlee doctor` records a recent extension handshake;
   - toolbar capture saves an article;
   - Mac menu-bar capture saves an article;
   - YouTube transcript capture saves transcript segments or the unavailable fallback.
   - `starlee diagnostics --last-capture` has terminal status and no article
     body, transcript text, selected text, full bearer token, or vault data.
