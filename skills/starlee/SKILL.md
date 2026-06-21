---
name: starlee
description: >
  Query the user's personal Starlee knowledge corpus — their captured articles,
  YouTube transcripts, and notes. Use when the user asks what they know about a
  topic, wants to recall something they read, asks about their reading history,
  wants to find connections between ideas, asks "what have I learned about X",
  wants to save content into Starlee, or needs Starlee setup/capture troubleshooting.
---

# Starlee

Starlee is the user's local-first digital brain. It stores captures as Markdown
in `~/Starlee/vault`, keeps a disposable SQLite hybrid/vector search index in
`~/Starlee/index.db`, and exposes MCP tools for Codex.

Use Starlee for:

- asking questions about the user's saved articles, YouTube transcripts, notes,
  and reading history;
- finding connections between a new URL/title/idea and the existing library;
- saving or capturing articles, web pages, YouTube transcripts, pasted notes, or
  ideas;
- listing recent captures or retrieving a complete record;
- exporting or ingesting `.starlee` share bundles;
- troubleshooting browser capture.

Do not use Starlee for coding, file management, or topics unrelated to the
user's personal knowledge base.

## Operating rules

- Prefer Starlee MCP tools when they are available.
- Use the default home `~/Starlee` unless the user explicitly asks for another
  vault.
- Do not reveal or paste the capture token.
- Do not commit or share vault data, config files, model caches, local
  databases, logs, or build artifacts.
- For paid, metered, or ambiguous web pages, use the browser sensor/extension
  capture path. Do not fetch and store gated content by URL-only capture.
- Starlee never performs synthesis with an external inference API. Codex does
  the reasoning; Starlee provides retrieval and citations.

## Session start

When a new Starlee-focused session starts, call `starlee_corpus_overview` first
if the tool is available. Use the result to:

- tell the user how many captures are in their brain and the date range;
- suggest 3 query topics based on `top_topics`;
- note if the corpus is sparse, especially with fewer than 10 captures, and
  suggest capturing more.

Skip the overview only when the user is clearly asking for setup/install,
troubleshooting, code work, or a non-Starlee task.

## Answering questions about the corpus

When the user asks what they know about a topic, asks to recall something, asks
about their reading history, or asks "what have I learned about X":

1. Call `starlee_query` with `question` set to the user's query.
2. If `relevance_floor_hit` is true and `chunks` is empty, tell the user their
   corpus does not have enough on this topic yet. Suggest related topics from
   the last corpus overview when available. Do not synthesize from training
   data.
3. If chunks are returned, synthesize a prose answer from only those chunks.
   Cite factual claims with inline markers like `[1]`.
4. At the end, include:

   ```text
   Sources:
   [1] {title} — {url or domain} — captured {captured_at}
   [2] ...
   ```

5. Never make a claim that is not supported by a returned chunk. If uncertain,
   say so and reference the specific chunk.

Tone: respond as if you are the user's own memory — confident, direct, and
cited. Prefer "you've been reading about..." over "the search returned...".

## Gap detection

If `starlee_query` returns sparse results or `relevance_floor_hit: true`, be
explicit:

- "I don't see enough in your Starlee corpus to answer that yet."
- "The closest saved item is..."
- "Topics that are better represented in your library are..."

Never fill gaps with general model knowledge unless the user explicitly asks
for a broader answer outside Starlee.

## "What connects to this?" pattern

When the user pastes a URL, title, or short passage and asks what it connects to
in their library:

1. Call `starlee_query` with `question` describing the connection task and
   `context` set to the pasted URL/title/text.
2. Synthesize the top 3-5 related captures in prose.
3. Explain the connection; do not merely list results.
4. Cite sources with the same inline marker and Sources section format.

## Recent reading

When the user asks what they have read lately or recently:

1. Prefer `recent` for a compact list of current captures, or call
   `starlee_query` with `question` set to "recent captures last 14 days" when a
   thematic synthesis is requested.
2. Group the response by theme rather than mechanically by date.
3. Cite specific captures.

## Setup and install

When the user asks "set yourself up", "install Starlee", or similar:

- Treat this as a setup task, not a query task.
- Direct fresh installs to run `./scripts/install.sh` from the Starlee repo.
- If the CLI is already installed, `starlee setup` initializes the local vault.
- After setup, run `starlee doctor` and make sure all gates are green.
- Do not call `starlee_query` or `starlee_corpus_overview` until setup is
  complete and `starlee doctor` is healthy.

## Capture workflows

To save pasted text, use the `capture` MCP tool with `source_type: "note"` or
`source_type: "article"`.

To save the current webpage, prefer the user's installed browser sensor:

- The macOS menu-bar item `★ Starlee` should create a local capture request.
- The Safari or Chromium extension should extract the visible page and send it
  only to `http://127.0.0.1:47291`.
- If capture fails, run `doctor` and check local service, extension assets,
  Safari extension installation, and extension handshake.

If the page button says the local engine is unreachable, ask the user to run:

```sh
~/.local/bin/starlee serve
```

If the page button asks for setup, ask the user to run:

```sh
~/.local/bin/starlee setup
```
