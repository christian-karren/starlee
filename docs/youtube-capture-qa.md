# YouTube Transcript Capture — QA & Hardening

This records the adversarial QA pass on YouTube transcript extraction: what was
reviewed, what was fixed, what is covered by tests, and the known remaining gaps
with their rationale. Goal: capture the transcript on every YouTube video, every
time — and when that's impossible, fail loudly and diagnosably, never silently.

## How capture works (two-layer, most-reliable-first)

1. **innertube `get_transcript`** — the authenticated API the transcript panel uses.
   Best-effort; currently returns HTTP 400 for many videos even with a valid
   SAPISIDHASH, so it is not relied upon. `transcript_source = innertube_transcript`.
2. **caption-track `timedtext` (`fmt=json3`)** — best-effort; YouTube pot-gates this
   for auto-captions, so it often returns empty. `transcript_source = caption_track`.
3. **Rendered transcript panel scrape** — the reliable path. Opens the panel once
   (no scrolling), waits out the loading spinner, reads `transcript-segment-view-model`
   rows, then closes the panel. `transcript_source = rendered_dom`.

Every step emits a redacted diagnostic, so `starlee diagnostics --last-capture`
names exactly which path produced the transcript or why each failed.

## Review pass: issues found and fixed

Three independent adversarial reviews (correctness, edge-case coverage, privacy/
security) were run against the source. Fixed:

| Severity | Issue | Fix |
| --- | --- | --- |
| Critical (security) | `captionTracks[].baseUrl` is page-controlled but was fetched with `credentials: include` and no origin check → cookie-exfil / SSRF | `isTrustedYouTubeUrl` allowlist (`https`, `*.youtube.com`); untrusted tracks dropped; rechecked in `fetchTimedText` |
| Critical (correctness) | Stale DOM player response after SPA navigation could capture the **wrong** video's transcript | `loadYouTubeContext` cross-checks `videoDetails.videoId` against the URL; on mismatch it discards the DOM copy and uses a fresh HTML fetch |
| High | View-model parser used `String.replace`, corrupting lines whose text contains/looks like a timestamp | Split on the **leading** stamp only via anchored regex |
| High | API/HTML fetches had no timeout — a hung connection stalled capture unboundedly | `fetchWithTimeout` (AbortController, 8s default, configurable via `transcriptApiTimeoutMs`) on all three fetches |
| High (coverage) | Shorts, `/live/`, and `youtu.be` were rejected at the gate even though they have transcripts | `isYouTubeWatch` now delegates to a unified `videoIdFromLocation` that parses `/watch`, `/shorts/`, `/live/`, `/embed/`, and `youtu.be`; canonicalizes to a watch URL |
| High | `cleanVideoId(null)` returned the literal `"null"` (fake id for `/watch` with no `v`) | Guard `String(value ?? "")` + bounded charset `{3,64}` |
| Medium | Panel left open on the lazy-open path (violates "leave the screen unchanged") | Track `panelWasOpenInitially`; set `openedByUs` whenever the panel opens after our click |
| Medium | `parseTimestamp` accepted fractional/oversized parts (`1.5:30`, `999999:00`) | Require integer 1–3 digit components |
| Low | `timedTextJson3Url` wouldn't override a pre-pinned `fmt` | Strip any existing `fmt` and force `json3` |
| Low (privacy, defense-in-depth) | Rust sanitizer denylisted metadata **keys** only, not values/message | Drop credential-shaped values (SAPISIDHASH / `Bearer ` / 40+ hex run) and redact such messages; broadened the key denylist |

Confirmed clean by review (no change needed): the `panel_tags` fingerprint is
tag-names only; SAPISID/SAPISIDHASH/Authorization are never logged and only sent to
hard-coded `youtube.com`; the loopback capture token never reaches the page; no
transcript text is inserted into the DOM (no XSS sink).

## Test coverage

`sensor/test/youtube.test.js` (33 tests) covers: legacy + view-model markup,
caption-track and innertube primary paths, HTML-refetch recovery, DOM fallback,
all transcript-panel reason codes (button-not-found, panel-not-opened, rows-empty,
language-unavailable, spinner/lazy rows), dedup/blank/malformed filtering, the
control-discovery strategies (button/menu/description-expander), and the new
regression tests: URL-gate matrix (Shorts/live/youtu.be/playlist/spoofed hosts),
timestamp-looking caption text, Shorts capture + canonicalization, SSRF baseUrl
rejection, SPA stale-player-response mismatch, abort-timeout on a hung fetch, and
`parseTimestamp` bounds. Rust side adds a sanitizer redaction test.

## Known gaps (deferred, with rationale)

These do not block "works on normal videos" but are tracked for full coverage:

- **DOM virtualization on very long transcripts.** If both API paths fail on a
  multi-hour video, the panel scrape reads only the rendered window. Mitigated
  because the panel usually loads fully and the row count is in the trace; full fix
  is to scroll the transcript container during discovery.
- **Age-restricted / members-only / consent-wall (EU).** These fail as an
  undifferentiated `unavailable`. They often work for a logged-in/entitled user via
  the credentialed refetch, but there's no distinct reason code yet. Fix: detect
  `playabilityStatus` / consent-page markers and emit a specific reason.
- **Live streams / premieres.** No transcript exists until VOD processing; outcome is
  correct (`unavailable`) but lacks a distinct `live_no_transcript` reason.
- **Language selection.** Always prefers English when present; no user override and
  no auto-translate fallback.
- **Title gate.** A video literally titled "YouTube" (or a slow SPA where the title
  hasn't updated) hard-fails extraction; should fall back to
  `videoDetails.title` before declaring failure.
- **MV3 pickup under full worker eviction.** The keep-alive port covers idle
  eviction; instant wake from a fully-evicted worker needs native messaging, planned
  for the Web Store build (stable extension ID + installer-registered host).

## When it legitimately can't get a transcript

Some videos have no transcript at all (captions disabled, live, members-only for a
non-member). In those cases metadata-only is the correct outcome; the trace's
`recommended_next_action` and reason code explain why, and `panel_tags` shows the
panel state. The harness is built so any future YouTube DOM/markup change surfaces
immediately in the trace (row count + `panel_tags`) rather than silently saving
empty.
