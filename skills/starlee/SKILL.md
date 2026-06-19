---
name: starlee
description: Use Starlee when the user wants to save articles, YouTube transcripts, pasted text, or notes into their local digital brain; search or retrieve their Starlee knowledge graph; export or ingest share bundles; or troubleshoot Starlee capture.
---

# Starlee

Starlee is a local-first digital brain. It stores captures as Markdown in `~/Starlee/vault`, keeps a disposable SQLite hybrid search index in `~/Starlee/index.db`, and exposes MCP tools for Codex.

Use Starlee when the user asks to:

- save or capture an article, web page, YouTube video, transcript, pasted note, or idea;
- search their local knowledge graph / digital brain;
- list recent captures or retrieve a record;
- export or ingest a `.starlee` share bundle;
- troubleshoot the browser capture button.

## Operating rules

- Prefer the Starlee MCP tools when they are available.
- Use the default home `~/Starlee` unless the user explicitly asks for another vault.
- Do not reveal or paste the capture token. If setup requires the browser extension token, tell the user to run local setup or use the generated extension folder.
- Do not commit or share vault data, config files, model caches, local databases, logs, or build artifacts.
- For paid, metered, or ambiguous web pages, use the browser sensor / extension capture path. Do not fetch and store gated content by URL-only capture.

## Common workflows

To save pasted text, use the `capture` MCP tool with `source_type: "note"` or `source_type: "article"`.

To search, use the `search` MCP tool with a concise query and cite the Starlee hit titles/URLs returned by the tool.

To help the user save the current webpage, make sure the local endpoint is running and tell them to use the page button from the unpacked browser extension at `~/Starlee/sensor-extension`.

If the page button says the local engine is unreachable, ask the user to run:

```sh
~/.local/bin/starlee serve
```

If the page button asks for setup, ask the user to run:

```sh
~/.local/bin/starlee setup
```

Then reload the unpacked extension in Chrome.
