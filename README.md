# Starlee

Starlee is a local-first, shareable digital brain. Markdown files are the source
of truth; the SQLite search index is a disposable cache.

Starlee includes:

- initialize a local Starlee home;
- capture pasted text as human-readable Markdown with YAML frontmatter;
- chunk and index captures with SQLite FTS5 and sqlite-vec;
- generate 384-dimensional embeddings locally with quantized BGE-small;
- search, list recent captures, get a record, and rebuild the index;
- expose capture, retrieval, setup, and sharing through an MCP stdio server;
- accept authenticated browser captures on a loopback-only HTTP endpoint.
- extract rendered articles with Mozilla Readability and rendered YouTube transcripts;
- export audited, restricted-body-free share bundles and mount them read-only;
- install as a Codex Plugin with bundled Starlee MCP tools and workflow guidance;
- expose Spotify sync diagnostics and Spotify episode vault schema, while
  clearly reporting Spotify's current podcast-history API limitation;
- provide a macOS menu-bar/floating-button app that can request capture from a
  browser extension after the user has loaded or installed that extension.

## Install

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
the extension, `starlee doctor` should show `extension_handshake: true`.

The generated extension folder includes the local-only capture configuration,
so the “Save article to Starlee” page button and the Starlee menu-bar capture
action work without pasting the capture token by hand. If you regenerate setup,
reload the unpacked extension in Chrome.

Open the menu-bar app:

```sh
open ~/Applications/Starlee.app
```

The app appears as a menu-bar icon. Click it once to capture the current
browser article or YouTube watch page. The icon only shows the success pulse
after the browser extension reports that the capture was saved; request pickup
and extraction remain in the loading state, and failures resolve to the error
state. Option-click the icon to open management tools for Recent Captures,
Browser Setup, diagnostics, vault access, capture-service controls, and Quit.

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

For a local Safari Web Extension source package and, when full Xcode is
installed, an Xcode wrapper app:

```sh
make package-safari
./scripts/inspect-safari-extension-package.sh release/safari-extension/starlee-safari-web-extension-0.1.0.zip
```

See [docs/safari-web-extension-local.md](docs/safari-web-extension-local.md) for
the local Safari run flow.

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
capture, hybrid search, recent/get, reindex, bookmarklet generation, optional
YouTube configuration, export, and ingest.
The MCP process also serves browser capture on `http://127.0.0.1:47291` by
default. Run `starlee serve` when only the capture endpoint is needed.
Run `starlee bookmarklet` (or call the MCP `bookmarklet` tool) only when you
explicitly want to generate a personalized zero-install capture link containing
the local token.
Run `starlee doctor` for redacted setup diagnostics; it reports token
fingerprints instead of token values.

`setup` creates a random 256-bit capture token in `<home>/config.json`; on Unix,
that file is mode `0600`. Browser sensors must send it as `Authorization: Bearer
<token>`. Starlee binds only to loopback and rejects unauthenticated captures.
See [docs/capture-payload.md](docs/capture-payload.md) for the versioned contract.

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
