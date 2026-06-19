# Browser capture contract

Sensors send rendered content to `POST http://127.0.0.1:47291/capture` with the
local bearer token. The endpoint accepts up to 16 MiB, defaults ambiguous access
to `restricted`, and supports payload version `1`.

```http
Authorization: Bearer <local token>
Content-Type: application/json
```

## Article

```json
{
  "version": 1,
  "type": "article",
  "url": "https://example.com/story",
  "access": "restricted",
  "dom_extract": {
    "title": "Story title",
    "byline": "Author",
    "site": "example.com",
    "published_at": "2026-06-19",
    "text": "Clean text extracted from the rendered DOM.",
    "summary": null,
    "html_meta": {}
  },
  "tags": []
}
```

## YouTube

```json
{
  "version": 1,
  "type": "youtube",
  "url": "https://www.youtube.com/watch?v=example",
  "access": "restricted",
  "dom_extract": {
    "title": "Video title",
    "byline": "Channel",
    "site": "youtube.com",
    "text": "",
    "html_meta": {}
  },
  "transcript": [
    { "t": 12.4, "text": "Timestamped transcript text" }
  ]
}
```

Starlee renders transcript segments as `[00:12] Timestamped transcript text` in
Markdown so agents can preserve moment-level provenance. If no transcript is
available, the item is still captured with `[Transcript unavailable]`.

## Responses

- `201`: capture written to Markdown and indexed.
- `400`: malformed or unsupported payload.
- `401`: missing or invalid bearer token.
- `413`: payload exceeds 16 MiB.

`GET /health` is intentionally unauthenticated and returns no user data or
secrets. `OPTIONS` supports browser CORS preflight.

## Extension handshake

Browser extensions should announce themselves after startup or reload:

```http
POST http://127.0.0.1:47291/extension/hello
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{
  "browser": "Chromium",
  "extension_version": "0.1.0",
  "can_capture_active_tab": true
}
```

The response records local setup state only. It never returns the token.

## Menu-bar capture bridge

The macOS menu-bar app requests browser capture by creating a local pending
request:

```http
POST http://127.0.0.1:47291/capture-request
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{ "source": "menu-bar" }
```

The browser extension polls:

```http
GET http://127.0.0.1:47291/capture-request
Authorization: Bearer <local token>
```

When a request is present, the extension captures the active tab with the same
rendered-DOM payload used by the toolbar button and posts it to `/capture`.
