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
- provide an optional macOS menu-bar app over the same engine.

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
- generates an unpacked Chromium extension in `~/Starlee/sensor-extension`.

Load `~/Starlee/sensor-extension` once in `chrome://extensions` with Developer
Mode enabled. The generated extension folder includes the local-only capture
configuration, so the “Save article to Starlee” page button works without
pasting the capture token by hand. If you regenerate setup, reload the unpacked
extension in Chrome.

You can still run setup manually:

```sh
starlee setup
```

`setup` initializes `~/Starlee`, downloads the quantized local embedding model,
installs the unpacked Chromium extension into `~/Starlee/sensor-extension`, and
returns extension settings plus a personalized bookmarklet.

For a packaged CLI and optional `Starlee.app`:

```sh
make package
```

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
```

Run `starlee mcp` to start the stdio transport. The MCP tools cover setup,
capture, hybrid search, recent/get, reindex, bookmarklet generation, optional
YouTube configuration, export, and ingest.
The MCP process also serves browser capture on `http://127.0.0.1:47291` by
default. Run `starlee serve` when only the capture endpoint is needed.
Run `starlee bookmarklet` (or call the MCP `bookmarklet` tool) to generate
a personalized zero-install capture link containing the local token.

`setup` creates a random 256-bit capture token in `<home>/config.json`; on Unix,
that file is mode `0600`. Browser sensors must send it as `Authorization: Bearer
<token>`. Starlee binds only to loopback and rejects unauthenticated captures.
See [docs/capture-payload.md](docs/capture-payload.md) for the versioned contract.

Optional richer YouTube metadata uses the official Data API only:

```sh
starlee configure-youtube --api-key "$YOUTUBE_DATA_API_KEY"
```

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
