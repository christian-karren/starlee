# Architecture

## Invariants

1. Markdown in `vault/{year}/{id}-{slug}.md` is canonical.
2. `index.db` may always be deleted and rebuilt from the vault.
3. Captured bodies stay on the local machine.
4. Ambiguous access defaults to `restricted`.
5. Every search hit includes source metadata and a local file path.

## Data flow

```text
CLI / MCP capture
      |
      v
normalize metadata -> write Markdown atomically -> content-aware chunk -> FTS5 + sqlite-vec
                                                        |
query -> local BGE embedding -> reciprocal-rank fusion -> cited result
```

## Components

- `Engine` owns orchestration and is shared by CLI, MCP, HTTP capture, and the
  optional menu-bar shell.
- `Vault` owns the portable file contract.
- `Index` owns disposable FTS and vector search state, including reciprocal-rank
  fusion. It chunks Markdown by source type before embedding: articles and
  notes prefer paragraph/sentence boundaries, transcript-like captures prefer
  timestamped lines when present, and fixed windows remain the fallback.
  Embeddings are recomputed from Markdown during reindex.
- Browser sensors emit a versioned payload into the engine; they never
  write vault files directly.
- The MCP process co-hosts a bearer-authenticated capture endpoint bound to
  `127.0.0.1`; the token lives only in the local mode-`0600` config file.
- The macOS menu-bar app does not read browser DOM directly. A normal click
  creates a local pending capture request; the browser extension polls the
  loopback service, extracts the active tab, posts the rendered payload back to
  `/capture`, then records the request result. The menu-bar icon only plays
  success feedback after the request reaches `capture_saved`.
- The menu-bar capture bridge is an observable local protocol, not a direct
  native-to-browser handoff. Request states are `queued`, `picked_up`,
  `extracting`, `posted`, `capture_saved`, `capture_failed`,
  `permission_denied`, `unsupported_page`, `extension_unavailable`, and
  `timed_out`. Pending requests expire after a short TTL so an old click cannot
  be captured later by accident.
- Extension freshness comes from the local `/extension/hello` heartbeat. If no
  extension has checked in recently, menu-bar capture returns
  `extension_unavailable` without queueing work. The extension owns active-tab
  extraction and refreshes its heartbeat while polling.
- Capture request status may include request id, source, timestamps, status,
  message, browser name, and safe page metadata such as title, URL, and domain.
  It must not include article bodies, transcripts, selected text, capture
  tokens, or restricted content.
- Generated extension assets are not the same as an installed browser
  extension. `starlee doctor` treats extension assets and extension handshake as
  separate checks.
- Chunk rows store the embedding model that produced each vector. `starlee
  reindex --stale-embeddings-only` refreshes only sources whose chunk model is
  missing or different from the current local embedder, so model upgrades do not
  require deleting the whole index.
- Share bundles are standalone SQLite files containing metadata, summaries, and
  vectors. Restricted chunk text is always `NULL`, enforced by a pre-write audit.
- Borrowed bundles are opened read-only and searched without copying them into
  the owner vault.

## Recovery

The vault is the only irreplaceable component. `starlee reindex` removes the
SQLite cache and recomputes chunks and vectors from Markdown. Stale-only
reindexing keeps the cache and replaces only sources with missing or outdated
embedding provenance. Share bundles and borrowed-base configuration never modify
the owner vault.
