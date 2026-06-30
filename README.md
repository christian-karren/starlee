# Starlee

Starlee is a local-first, shareable digital brain. Markdown files are the source
of truth; the SQLite search index is a disposable cache.

Starlee includes:

- initialize a local Starlee home;
- capture pasted text as human-readable Markdown with YAML frontmatter;
- chunk captures on article and transcript boundaries, then index them with
  SQLite FTS5 and sqlite-vec;
- generate 384-dimensional embeddings locally with quantized BGE-small;
- search, list recent captures, get a record, and rebuild the index;
- expose capture, retrieval, setup, and sharing through an MCP stdio server;
- accept authenticated browser captures on a loopback-only HTTP endpoint.
- extract rendered articles with Mozilla Readability and rendered YouTube transcripts;
- export audited, restricted-body-free share bundles and mount them read-only;
- install as a Codex Plugin with bundled Starlee MCP tools and workflow guidance;
- expose Spotify sync diagnostics and Spotify episode vault schema, while
  clearly reporting Spotify's current podcast-history API limitation;
- provide a Dock-visible macOS app with a desktop status window and a persistent
  menu-bar capture icon that can request capture from a browser extension after
  the user has loaded or installed that extension.

## V1 browser baseline

Starlee v1 is Chrome-only for browser capture. The supported production path is:

1. install Starlee;
2. load or install the Chrome extension;
3. capture rendered articles and YouTube transcripts from Chrome through the
   Chrome toolbar, in-page save button, or Starlee macOS menu-bar icon.

Firefox and Safari are future browser targets. They must not affect v1
onboarding, diagnostics, release readiness, or capture routing. See
[docs/chrome-capture-v1-baseline.md](docs/chrome-capture-v1-baseline.md) for
the source-of-truth contract and the known-good manual QA record.

## Download

**[⬇ Download the latest Starlee for macOS](https://github.com/christian-karren/starlee/releases/latest)**

1. Open the downloaded `Starlee-*.dmg` and drag **Starlee** into **Applications**.
2. Launch Starlee — its icon appears in the macOS menu bar.

> First launch only: this build is not yet notarized by Apple, so macOS shows a
> "cannot check it for malicious software" prompt. Right-click the app →
> **Open** once to trust it (or run
> `xattr -dr com.apple.quarantine /Applications/Starlee.app`).

Your Markdown vault, search index, and config live in `~/Starlee`.

## Install from source

Build and install the full local experience:

```sh
./scripts/install.sh
```

The installer:

- builds and installs the CLI to `~/.local/bin/starlee`;
- initializes `~/Starlee`;
- installs Starlee as a local Codex Plugin from the personal marketplace;
- starts the loopback capture service with a macOS LaunchAgent;
- installs `Starlee.app` to `~/Applications/Starlee.app`;
- generates unpacked Chromium extension assets in `~/Starlee/sensor-extension`;
- prints a redacted `starlee doctor` report describing what is installed,
  running, and still missing.

Important: the installer generates extension assets, but Chrome does not treat
that as an installed extension. Load `~/Starlee/sensor-extension` once in
`chrome://extensions` with Developer Mode enabled. After Chrome loads or reloads
the extension, `starlee doctor` should show a healthy `browser_bridge` check.
The diagnostics include whether extension setup/config exists, which browser
checked in, whether the heartbeat is fresh, the last capture request status, the
last safe failure reason, the next recovery action, and a bounded redacted trace
of recent menu-bar capture events. For deeper local debugging, run
`starlee diagnostics --limit 50`; for the most recent one-tap request, run
`starlee diagnostics --last-capture`.

The generated extension folder includes the local-only capture configuration,
so the “Save article to Starlee” page button and the Starlee menu-bar capture
action work without pasting the capture token by hand. If you regenerate setup,
reload the unpacked extension in Chrome.

The extension has a single source of truth. Edit only `sensor/src/*.js`; `npm run
build` bundles them into `sensor/dist/extension/`, the CLI embeds that build at
compile time, and `starlee setup` writes it to `~/Starlee/sensor-extension`. Never
hand-edit `~/Starlee/sensor-extension` — it is generated. `starlee doctor` runs an
`extension_up_to_date` check that fails when the loaded extension drifts from the
build embedded in your installed binary; re-run `starlee setup` and reload the
unpacked extension if it does.

Open the menu-bar app:

```sh
open ~/Applications/Starlee.app
```

The app appears in the Dock and opens a desktop status window for the local
capture system, browser bridge, and loopback endpoint. It also keeps the
Starlee menu-bar icon available for one-tap capture. Click the menu-bar icon
once to capture the current browser article or YouTube watch page. The icon only
shows the success pulse after the browser extension reports that the capture was
saved; request pickup and extraction remain in the loading state, stale requests
time out quickly, and failures distinguish extension availability, page
permission, unsupported pages, and capture errors. Option-click the icon to open
management tools for Recent Captures, Browser Setup, diagnostics, vault access,
capture-service controls, and Quit.

Closing the desktop window leaves the menu-bar capture icon running. Reopening
Starlee from Finder or the Dock brings the window back instead of creating a
second menu-bar icon. Quit Starlee from the app menu or the menu-bar management
menu to stop the desktop app process.
For YouTube watch pages, Starlee stores a restricted canonical video record with
title, channel when available, video id, consumed time, transcript
status/source/reason, and either timestamped transcript lines from the rendered
page or an explicit `[Transcript unavailable]` fallback. The extension makes a
bounded attempt to open the rendered transcript UI before falling back.
Transcript capture does not require OAuth, the YouTube Data API, audio/video
download, or any external transcript service.

You can still run setup manually:

```sh
starlee setup
```

`setup` initializes `~/Starlee`, downloads the quantized local embedding model,
generates unpacked Chromium extension assets in `~/Starlee/sensor-extension`,
and returns redacted extension settings. It does not print the capture token.

For a packaged CLI and optional `Starlee.app`:

```sh
make package
```

For a Chrome Web Store-ready extension ZIP:

```sh
make package-chrome
./scripts/inspect-chrome-extension-package.sh release/chrome-extension/starlee-capture-0.1.0.zip
```

The Chrome package is local-only by design: the extension declares loopback
network access to `127.0.0.1`, strips generated `starlee-config.json`, and does
not include vault data, local config, model files, source maps, or build caches.
See [docs/chrome-extension-release.md](docs/chrome-extension-release.md) for the
permission rationale, store listing draft, and manual Chrome Web Store steps.

Safari and Firefox package scripts are retained for future work, but they are
not part of the v1 production release path.

## Try it

```sh
starlee setup
starlee capture-text \
  --title "A useful idea" \
  --text "Durable knowledge compounds when it remains searchable." \
  --url "https://example.com/idea" \
  --access public
starlee search "durable knowledge"
starlee status
starlee doctor
```

Run `starlee mcp` to start the stdio transport. The MCP tools cover setup,
capture, hybrid search, citation-ready hybrid query retrieval, recent/get,
reindex, bookmarklet generation, optional YouTube configuration, export, and
ingest.
The MCP process also serves browser capture on `http://127.0.0.1:47291` by
default. Run `starlee serve` when only the capture endpoint is needed.
Run `starlee bookmarklet` (or call the MCP `bookmarklet` tool) only when you
explicitly want to generate a personalized zero-install capture link containing
the local token.
Run `starlee doctor` for redacted setup diagnostics; it reports token
fingerprints instead of token values. Browser bridge health is also redacted:
it does not include request IDs, capture tokens, article bodies, transcripts,
selected text, or restricted content. Common recovery actions are to load or
reload the extension, grant site access and reload the page, open an article or
YouTube watch page, or retry after the browser picks up timed-out requests.

`setup` creates a random 256-bit capture token in `<home>/config.json`; on Unix,
that file is mode `0600`. Browser sensors must send it as `Authorization: Bearer
<token>`. Starlee binds only to loopback and rejects unauthenticated captures.
See [docs/capture-payload.md](docs/capture-payload.md) for the versioned contract.
For failed YouTube one-tap captures, see
[docs/youtube-capture-debugging.md](docs/youtube-capture-debugging.md).

Optional richer YouTube metadata uses the official Data API only:

```sh
starlee configure-youtube --api-key "$YOUTUBE_DATA_API_KEY"
```

Spotify passive podcast sync is partially scaffolded but not claimed as
complete. Spotify's recently played endpoint currently does not support podcast
episodes, so Starlee exposes honest diagnostics instead of pretending hourly
history polling works:

```sh
starlee sync-status
starlee sync-spotify
```

See [docs/spotify-passive-sync.md](docs/spotify-passive-sync.md) for the product
tradeoff and viable paths.

Share bundles exclude every restricted body and are audited before being
written. Public bodies are also excluded unless explicitly requested:

```sh
starlee export ./brain.starlee
starlee ingest ./friends-brain.starlee
starlee search --scope borrowed "what did they learn about memory?"
```

## Privacy boundary

Captured content and inference stay on the device. URL-only fetching is allowed
only for pages explicitly marked `isAccessibleForFree=true`; ambiguous and known
metered pages are refused and routed to the browser sensor. Share bundles always
strip restricted bodies and fail export if the audit detects a leak.

Search uses reciprocal-rank fusion over local semantic vectors and FTS5 keyword
matches without changing the Markdown contract. Recapturing a canonical URL
updates the existing Markdown record rather than creating a duplicate.

The quantized `BAAI/bge-small-en-v1.5` model downloads into `<home>/models` on
first setup. After that, capture and search inference run locally without an API
key or inference service.

See [docs/architecture.md](docs/architecture.md) and
[docs/release-checklist.md](docs/release-checklist.md).

## Tests

Run the full local test suite:

```sh
make test
```

Run only the menu-bar/browser bridge smoke test:

```sh
./scripts/bridge-smoke-test.sh
```

The smoke test starts the real loopback capture service on a temporary Starlee
home, drives a menu-bar-style `/capture-request`, uses the browser sensor's
article extraction modules against a deterministic local fixture, posts the
result to `/capture`, records `/capture-request/result`, and asserts that one
Markdown vault entry was created. It also checks duplicate pickup handling and
that terminal request metadata contains only sanitized status and record
identity fields, not article bodies, selected text, transcripts, or tokens.

This is a bridge harness, not full browser UI automation. It does not launch
Chrome or exercise extension permissions.
