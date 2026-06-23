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
The menu-bar bridge treats the handshake as fresh for a short window; current
extensions refresh it while polling so a loaded extension is not blocked.

## Menu-bar capture bridge

The macOS menu-bar app requests browser capture by creating a local pending
request. If no extension has checked in recently, the service returns
`409` with `extension_unavailable` and does not queue a request:

```http
POST http://127.0.0.1:47291/capture-request
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{ "source": "menu-bar" }
```

Successful responses are enveloped as `{"request": ...}` and include only
protocol metadata:

```json
{
  "request": {
    "id": "<request id>",
    "source": "menu-bar",
    "requested_at": "2026-06-23T05:00:00Z",
    "picked_up_at": null,
    "completed_at": null,
    "status": "queued",
    "message": "Capture request queued for the browser extension.",
    "browser": "Chrome",
    "page": null
  }
}
```

The browser extension polls:

```http
GET http://127.0.0.1:47291/capture-request
Authorization: Bearer <local token>
```

When a request is present, the service atomically removes it from the pending
slot and marks it `picked_up`, preventing duplicate extension pickups. Requests
older than 10 seconds become `timed_out` and are not served to the extension.

When a request is present, the extension captures the active tab with the same
rendered-DOM payload used by the toolbar button and posts it to `/capture`. It
also records intermediate lifecycle updates:

- `extracting`: the extension is asking the content script to read the active
  tab.
- `posted`: the extension posted a capture payload to the local service.

The extension then records the final result for the original request:

```http
POST http://127.0.0.1:47291/capture-request/result
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{
  "id": "<request id>",
  "status": "capture_saved",
  "message": "Saved to Starlee.",
  "page": {
    "title": "Example article",
    "url": "https://example.com/article",
    "domain": "example.com"
  }
}
```

`page` is optional and must stay safe: title, URL, and domain only. Article
bodies, transcripts, selected text, capture tokens, and restricted content must
not be written to request status metadata.

The menu-bar app polls request status while its icon is in the loading state:

```http
GET http://127.0.0.1:47291/capture-request/status?id=<request id>
Authorization: Bearer <local token>
```

Lifecycle states are:

- `queued`
- `picked_up`
- `extracting`
- `posted`
- `capture_saved`
- `capture_failed`
- `permission_denied`
- `unsupported_page`
- `extension_unavailable`
- `timed_out`

Success feedback in the macOS menu bar is reserved for `capture_saved`.
`queued`, `picked_up`, `extracting`, and `posted` stay in loading state.
All other terminal states return a distinct error state with an actionable
message.
