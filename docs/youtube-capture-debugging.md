# YouTube Capture Debugging

Starlee's YouTube one-tap capture path is local-only:

1. The menu-bar app creates a `/capture-request`.
2. The browser extension polls and picks up the request.
3. The active tab content script builds a rendered-DOM payload.
4. The YouTube extractor records metadata and rendered transcript segment counts.
5. The extension posts `/capture`.
6. The extension records `/capture-request/result`.
7. The menu-bar app shows success only after `capture_saved`.

## Verify What Is Running

Check the source branch and commit:

```sh
git branch --show-current
git rev-parse --short HEAD
```

Check the installed CLI and runtime identity:

```sh
starlee --version
starlee diagnostics --last-capture
```

The trace reports the Starlee version, browser name, extension version, current
git commit when available, source repo path when available, installed app path
when present, and the binary path of the CLI that produced the trace.

Check the installed menu-bar app path:

```sh
ls -la ~/Applications/Starlee.app
```

Check the installed/generated extension version:

```sh
cat ~/Starlee/sensor-extension/manifest.json
```

Then reload the unpacked extension in the browser and run:

```sh
starlee doctor
```

## Inspect The Last Capture

Run:

```sh
starlee diagnostics --last-capture
```

The output is safe to share. It includes request lifecycle transitions, extension
and content-script events, YouTube extractor milestones, segment counts, final
terminal status, and a recommended next action. It must not include capture
tokens, OAuth tokens, article bodies, transcript text, selected text, raw HTML,
cookies, embeddings, or private file bodies.

## Common Failure Points

- `extension_unavailable`: load or reload `~/Starlee/sensor-extension`, then retry.
- `permission_denied`: grant page access to the Starlee extension and reload the tab.
- `unsupported_page`: open an article or `youtube.com/watch?v=...` page.
- `content_script_capture_failed`: the content script ran but could not build a payload.
- `youtube_metadata_extracted` with `failed`: the page did not expose a usable video id or title.
- `youtube_transcript_discovery_finished` with `unavailable`: the transcript panel did not render before the bounded timeout.
- `youtube_segments_extracted` with `segment_count: "0"`: the extractor ran, but no rendered transcript segments were found.
- `capture_failed` after `capture_payload_posted`: the backend rejected the payload or failed while writing/indexing.
- `vault` or `index` failures in `starlee doctor`: the local vault or disposable index needs repair.

## Share-Safe Output

Share `starlee diagnostics --last-capture`, `starlee doctor`, browser name,
extension version, CLI version, and branch/commit. Do not share `~/Starlee/config.json`
or raw browser console logs unless you have checked that they contain no tokens
or captured page content.
