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

## How The Transcript Is Captured

Transcript capture has two paths, tried in order:

1. **Caption track (primary, invisible).** The extension reads the page's player
   response, picks the best caption track (preferring manual English, then
   auto-generated), and fetches the `timedtext` endpoint as `fmt=json3`. This needs
   no transcript-panel UI, captures auto-generated captions, and does not move or
   change anything on screen. Diagnostic events: `youtube_caption_tracks_found`,
   `youtube_timedtext_fetch_started`, `youtube_timedtext_fetch_succeeded` /
   `youtube_timedtext_fetch_failed`. On success `transcript_source = caption_track`.
2. **Rendered DOM (fallback).** Only if the caption track yields nothing, the
   extension opens the transcript panel once (no scrolling), waits for the rows to
   render, scrapes them, and closes the panel again. On success
   `transcript_source = rendered_dom`.

Capture still saves the canonical video record even when both paths fail, but the
trace's `recommended_next_action` will name the failing step.

## Reason Code Decision Tree

`starlee diagnostics --last-capture` reports a single `recommended_next_action`.
For a YouTube capture that ran but came back without a transcript, that action is
derived from the `transcript_reason` recorded on the final
`youtube_segments_extracted` event (the table below), instead of generic bridge
advice. Upstream failures (permission, unreachable, timed out) never reach the
extractor, so they keep the bridge-level recommendation.

| `transcript_reason` | What happened | Next action |
| --- | --- | --- |
| `rendered_transcript_segments_found` | Transcript captured. | None — success. |
| `transcript_disabled_by_video` | The video has no transcript/captions. | Metadata-only save is expected; nothing to fix. |
| `transcript_language_unavailable` | No transcript in a supported language. | Metadata-only save is expected; nothing to fix. |
| `transcript_rows_empty` | Panel opened but rendered zero lines. | Reload the tab, open the transcript once, capture again. |
| `transcript_panel_not_opened` | Control found, panel never opened. | Open the transcript once manually, then capture again. |
| `transcript_button_not_found` | No "Show transcript" control on the page. | Expand the description to reveal it, or open the transcript manually, then capture again. |
| `transcript_discovery_timed_out` | Lines did not render before the bounded timeout. | Reload the tab; capture again once fully loaded. |
| `transcript_panel_not_rendered` | Content script ran before the transcript hydrated. | Reload the tab so the content script runs after load, then capture again. |
| `youtube_metadata_unavailable` | Watch page had no usable video id/title yet. | Reload the tab and capture again. |
| `extractor_failure` | Extraction threw before reading the page. | Reload the tab and capture again. |

The same reason codes appear as discrete diagnostic events
(`transcript_button_not_found`, `transcript_panel_not_opened`,
`transcript_rows_empty`, `transcript_disabled_by_video`, …) earlier in the trace,
so you can see every discovery strategy that was attempted and why it stopped.

## One Extension, One Source Of Truth

The extension has exactly one source of truth and one installed copy:

1. Edit only `sensor/src/*.js` (modular ES modules).
2. `npm run build` (esbuild) bundles them into `sensor/dist/extension/`.
3. The CLI embeds `sensor/dist/extension/*` at compile time (`src/sensor_assets.rs`).
4. `starlee setup` writes those bytes to `~/Starlee/sensor-extension/` — the only
   folder Chrome should load.

Never hand-edit `~/Starlee/sensor-extension`; it is generated. `starlee doctor`
includes an `extension_up_to_date` check that fails when the installed copy no
longer matches the build embedded in the running binary — re-run `starlee setup`
and reload the unpacked extension when it does.

## Share-Safe Output

Share `starlee diagnostics --last-capture`, `starlee doctor`, browser name,
extension version, CLI version, and branch/commit. Do not share `~/Starlee/config.json`
or raw browser console logs unless you have checked that they contain no tokens
or captured page content.
