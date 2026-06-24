# Architecture

## Invariants

1. Markdown in `vault/{year}/{id}-{slug}.md` is canonical.
2. `index.db` may always be deleted and rebuilt from the vault.
3. Captured bodies stay on the local machine.
4. Ambiguous access defaults to `restricted`.
5. Every search hit includes source metadata and a local file path.

## Data flow

```text
CLI / MCP / browser capture
      |
      v
normalize metadata -> write Markdown atomically -> content-aware chunk
                                                        |
                                                        v
                                         chunk text + timestamps -> FTS5 + sqlite-vec
                                                        |
query -> local BGE embedding + BM25 terms -> reciprocal-rank fusion -> cited result
```

## Retrieval Lifecycle

Capture and ingest paths normalize input into `CaptureInput`. Browser article
captures supply rendered text; YouTube captures may supply timestamped
transcript segments that are rendered as readable `[MM:SS]` lines. The engine
optionally enriches YouTube metadata locally from a user-configured API key,
then writes a Markdown record with YAML frontmatter through `Vault`.

Markdown is the durable memory. `Index::upsert()` reads the resulting `Record`
and creates embedding units from `Record.body`. Articles and notes prefer
paragraph and sentence boundaries, with fixed-size windowing as a fallback for
long or unstructured text. Transcript-like captures group timestamped lines
into bounded chunks and populate `chunks.t_start` and `chunks.t_end` from the
segment timestamps when available.

Each chunk is embedded through the existing local `Embedder` abstraction. The
default `FastEmbedder` runs quantized BGE-small from the local model cache. The
index stores chunk text in SQLite, mirrors it into FTS5, stores the vector in
`sqlite-vec`, records the embedding model name on each chunk, and keeps source
metadata such as URL, title, access, captured time, and consumed time.

Search and query retrieval both use local hybrid signals. BM25 matches help
exact terms, names, titles, and phrases surface; vector matches keep semantic
recall working. Candidates are fused in the index and returned with citation
metadata. The MCP query path still applies the configured relevance floor before
returning chunks to agents.

## Components

- `Engine` owns orchestration and is shared by CLI, MCP, HTTP capture, and the
  optional menu-bar shell.
- `Vault` owns the portable file contract.
- `Index` owns disposable chunking, FTS, and vector search state, including
  reciprocal-rank fusion. It chunks Markdown by source type before embedding:
  articles and notes prefer paragraph/sentence boundaries, transcript-like
  captures prefer timestamped lines when present, and fixed windows remain the
  fallback. Embeddings are recomputed from Markdown during reindex, and stale
  embedding reindex only refreshes chunks whose stored model name differs from
  the current embedder.
- Browser sensors emit a versioned payload into the engine; they never
  write vault files directly.
- YouTube capture is a browser-owned extraction path inside the same versioned
  sensor contract. The extension detects supported `youtube.com/watch` pages,
  normalizes the video URL, extracts rendered DOM transcript segments when they
  are present, makes a bounded attempt to open the rendered transcript UI,
  records `transcript_status`/`transcript_source`/`transcript_reason`, and
  saves an explicit transcript-unavailable fallback when no rendered transcript
  is available. It does not use OAuth, external transcript services, downloaded
  audio/video, or the optional YouTube Data API for transcript text.
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
- Browser bridge health is derived from local setup files, extension heartbeat
  freshness, the last hello payload, and the last request lifecycle status. It
  answers whether extension setup/config exists, whether a browser checked in
  recently, which browser checked in, whether active-tab capture is available,
  the last safe failure reason/message, the recommended next action, and a
  bounded recent diagnostic trace.
- Bridge health is observable in `starlee status`, `starlee doctor`, the
  menu-bar diagnostics summary, the extension options page, and the authenticated
  loopback `/bridge-health` endpoint. A deeper local trace is available with
  `starlee diagnostics --limit N`.
- Capture request status may include request id, source, timestamps, status,
  message, browser name, and safe page metadata such as title, URL, and domain.
  It must not include article bodies, transcripts, selected text, capture
  tokens, or restricted content.
- Capture diagnostics are a capped local ring buffer in Starlee setup state.
  The full local `starlee diagnostics` view records timestamp, component, event
  name, request id, lifecycle status, source, browser, sanitized message, and
  sanitized page metadata when available. The shorter `doctor`/bridge-health
  summary redacts request ids and page metadata.
- YouTube one-tap capture diagnostics include request-correlated extension,
  content-script, payload-builder, and YouTube extractor milestones. The
  `starlee diagnostics --last-capture` trace groups the newest request
  chronologically with runtime identity, terminal status, and a recommended
  next action.
- Bridge health is stricter than request status: it does not expose request IDs
  or page metadata, and it replaces failure messages for known browser failure
  states with concise user-facing recovery text.
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
