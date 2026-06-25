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
  ],
  "transcript_status": "full",
  "transcript_source": "rendered_dom",
  "transcript_reason": "rendered_transcript_segments_found",
  "extractor_version": "youtube-dom-v1"
}
```

Starlee renders transcript segments as `[00:12] Timestamped transcript text` in
Markdown so agents can preserve moment-level provenance. Timestamp-aware
indexing reads those canonical lines and stores chunk `t_start`/`t_end` ranges
when timing exists. If no transcript is available, the item is still captured
with `[Transcript unavailable]`, `transcript_status: unavailable`, and
`transcript_source: unavailable`. The optional `transcript_reason` records
whether the extension found rendered segments, did not see a transcript panel,
or attempted transcript discovery but no rendered segments appeared before the
bounded timeout.

YouTube payloads should use the canonical URL
`https://www.youtube.com/watch?v={video_id}` when the video id is known.
The backend also normalizes YouTube URLs before writing, requires a title and
video id, filters malformed or duplicate transcript segments, stores captures as
`restricted` by default, and recaptures the same canonical video in place.
Transcript text belongs only in the capture payload, Markdown vault, and local
index; bridge request status and bridge health may include safe page metadata
only.

## Responses

- `201`: capture written to Markdown and indexed.
- `400`: malformed or unsupported payload.
- `401`: missing or invalid bearer token.
- `413`: payload exceeds 16 MiB.

`GET /health` is intentionally unauthenticated and returns no user data or
secrets. `OPTIONS` supports browser CORS preflight.

`GET /bridge-health` requires the bearer token and returns sanitized browser
bridge diagnostics:

```json
{
  "bridge_health": {
    "ok": true,
    "chrome_setup": {
      "installed": true,
      "checked_in_recently": true,
      "permission_needed": false,
      "capture_test_passed": true,
      "capture_test_passed_at": "2026-06-23T05:00:02Z",
      "state": "capture_test_passed",
      "detail": "Chrome capture has completed a setup test through the local bridge.",
      "next_action": "Open an article or YouTube watch page and capture from Starlee."
    },
    "extension_setup_present": true,
    "extension_config_present": true,
    "checked_in_recently": true,
    "browser": "Chrome",
    "extension_version": "0.1.0",
    "extension_build": "main@abc123",
    "can_capture_active_tab": true,
    "last_hello_at": "2026-06-23T05:00:00Z",
    "last_request_status": "capture_saved",
    "last_failure_reason": null,
    "last_failure_message": null,
    "recommended_next_action": "Bridge is ready. Open an article or YouTube watch page and capture again.",
    "recent_diagnostics": [
      {
        "timestamp": "2026-06-23T05:00:01Z",
        "component": "browser_bridge",
        "event": "capture_request_status",
        "status": "capture_saved",
        "source": "menu-bar",
        "browser": "Safari",
        "message": "Saved to Starlee."
      }
    ]
  }
}
```

Bridge health never includes capture tokens, request IDs, article bodies,
transcripts, selected text, page metadata, or restricted content. Use
`starlee diagnostics --limit N` for the longer local rolling history, including
request IDs and sanitized page metadata for correlating one menu-bar click
across lifecycle events. Use `starlee diagnostics --last-capture` for a
chronological trace of the newest request, runtime identity, terminal status,
and recommended next action.

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
  "extension_build": "main@abc123",
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
message. Each transition is also appended to the bounded local capture
diagnostic log.

Default recovery messages are intentionally concise:

- `extension_unavailable`: load or reload the Starlee browser extension.
- `permission_denied`: grant Starlee site access in the browser, or reload the
  page.
- `unsupported_page`: the active page is not an article or YouTube watch page.
- `timed_out`: the browser did not pick up the request in time.

## Diagnostic events

The extension may post request-correlated, redacted events to:

```http
POST http://127.0.0.1:47291/capture-diagnostics/event
Authorization: Bearer <local token>
Content-Type: application/json
```

```json
{
  "timestamp": "2026-06-23T05:00:01Z",
  "component": "youtube_extractor",
  "event": "youtube_segments_extracted",
  "request_id": "<request id fingerprint>",
  "status": "unavailable",
  "browser": "Chrome",
  "message": "No rendered transcript segments found.",
  "page": {
    "title": "Video title",
    "url": "https://www.youtube.com/watch?v=example",
    "domain": "youtube.com"
  },
  "safe_metadata": {
    "segment_count": "0",
    "transcript_reason": "transcript_discovery_unavailable_or_timed_out"
  }
}
```

The endpoint requires the bearer token, stores events in the bounded local
diagnostic buffer, truncates oversized strings, and accepts only the redacted
event shape. `safe_metadata` is for structured counts, booleans, version strings,
and reason codes. It must not contain article bodies, transcript text, selected
text, capture tokens, OAuth tokens, cookies, raw HTML, embeddings, or private
file bodies.
