# Chunking and Embedding

Starlee stores Markdown captures in the vault as the canonical source of truth.
`index.db` is a disposable retrieval cache built from those Markdown files.
During capture or reindex, each record is chunked, embedded locally, and stored
in SQLite FTS5 plus `sqlite-vec`.

## Source Text Lifecycle

1. CLI and MCP captures create `CaptureInput` directly. Browser captures send a
   versioned `CapturePayload`, which is normalized into `CaptureInput`.
2. `Vault` writes the capture to Markdown with YAML frontmatter. The Markdown
   body is the only text source used for indexing.
3. `Index::upsert` chooses a chunking strategy from the record source type,
   embeds each chunk through the local `Embedder` trait, and writes chunk rows
   plus vector rows in one transaction.
4. Search and query use the chunk rows for FTS snippets and the vector rows for
   local semantic retrieval.

## Chunking Strategies

Articles and notes use text-aware chunking. Paragraphs are kept intact when they
fit within the configured maximum chunk size. Paragraphs that are too large are
split on sentence boundaries when possible. If a sentence or text span is still
too large, Starlee falls back to fixed-size windows with overlap.

Transcript-like captures, currently YouTube and Spotify episode records, first
look for timestamped lines in the canonical Markdown form:

```text
[00:12] Timestamped transcript text
```

When timestamped lines are present, chunks are packed from those transcript
units and chunk rows store the covered `t_start` and `t_end` range. If timing is
missing, the same text-aware path used by notes is used, without failing
ingestion.

Fixed-window chunking remains available as the final fallback for unusual text,
very long unbroken spans, and legacy-shaped captures. Chunking is deterministic:
given the same Markdown body, source type, and limits, it produces the same
chunk boundaries.

## Stored Chunk Metadata

Each chunk row stores:

- source id and ordinal;
- character start and end offsets into the canonical Markdown body;
- optional `t_start` and `t_end` seconds for timestamped media;
- access level;
- chunk text;
- `embedding_model`, the local embedder name that produced the vector.

The current embedder is the quantized local BGE-small model exposed through the
existing `Embedder` trait. Starlee does not call hosted embedding APIs.

## Stale Re-Embedding

`starlee reindex` rebuilds the whole disposable index from Markdown. `starlee
reindex --stale-embeddings-only` keeps the index and selects only sources whose
chunks have a missing or non-current `embedding_model`. Those sources are
upserted again, replacing their chunks and vectors with embeddings from the
current local model.

This lets a future model upgrade refresh stale vectors without requiring a full
corpus rebuild as the only available path.

## Privacy Constraints

Chunking and embedding run locally. Article bodies, transcripts, restricted
content, embeddings, and capture tokens must not be written to logs or config.
The vault remains the canonical local record, and `index.db` remains rebuildable
from local Markdown.
