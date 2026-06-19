# Release checklist

## Implemented gates

- Markdown vault is canonical and fully reindexable.
- Quantized BGE-small embeddings run locally; no inference API is present.
- Hybrid sqlite-vec + FTS5 retrieval returns source paths, URLs, and snippets.
- MCP stdio uses newline-delimited JSON-RPC and negotiates stable protocol versions.
- Capture endpoint binds to `127.0.0.1` and requires a random 256-bit bearer token.
- Article extraction runs in the rendered browser DOM through Mozilla Readability.
- Access classification uses `isAccessibleForFree`, domain/marker heuristics, and fails closed.
- YouTube transcripts come only from rendered DOM segments and retain timestamps.
- Optional YouTube metadata uses official Data API `videos.list` only.
- URL-only server capture requires an explicit public schema signal.
- Recaptured canonical URLs update in place.
- Share export strips all restricted bodies and blocks output on audit failure.
- Borrowed bundles open read-only and return summary/citation for `get`.
- Setup installs the model, extension assets, bookmarklet, token, and example prompts.
- Optional macOS menu-bar app supports status, recent items, search, pasted capture, vault access, and endpoint control.

## Validation commands

```sh
make test
./scripts/legal-invariants.sh
make package
```

Before a commercial public release, run the maintained 50-site extraction corpus
against current publisher pages and obtain counsel review for publisher-specific
terms and restricted-text embeddings. Those are operational release activities,
not hidden runtime dependencies.
