import { htmlMeta, pageMetadata } from "./metadata.js";

export const YOUTUBE_EXTRACTOR_VERSION = "youtube-dom-v3";

export function isYouTubeWatch(document) {
  return Boolean(videoIdFromLocation(document.location.href));
}

export async function extractYouTube(document, options = {}) {
  const result = await extractYouTubeResult(document, options);
  if (!result.ok) throw new Error(result.error);
  return {
    version: 1,
    type: "youtube",
    url: result.metadata.canonical_url,
    access: "restricted",
    dom_extract: {
      title: result.metadata.title,
      byline: result.metadata.channel,
      site: "youtube.com",
      published_at: result.metadata.published_at,
      text: "",
      html_meta: {
        ...htmlMeta(document),
        "starlee:youtube_extractor_version": result.extractor_version,
        "starlee:youtube_video_id": result.metadata.video_id,
        "starlee:transcript_status": result.transcript_status,
        "starlee:transcript_source": result.transcript_source,
        "starlee:transcript_reason": result.transcript_reason
      }
    },
    transcript: result.segments,
    transcript_status: result.transcript_status,
    transcript_source: result.transcript_source,
    transcript_reason: result.transcript_reason,
    extractor_version: result.extractor_version,
    tags: []
  };
}

export async function extractYouTubeResult(document, options = {}) {
  const startedAt = Date.now();
  emitDiagnostic(options, "youtube_extractor_started", {
    status: "started",
    safe_metadata: {
      extractor_version: YOUTUBE_EXTRACTOR_VERSION,
      host: safeHost(document),
      video_id_present: String(Boolean(videoIdFromLocation(document.location.href)))
    }
  });
  const metadata = pageMetadata(document);
  const videoId = videoIdFromLocation(document.location.href);
  const title = cleanText(text(document, "h1.ytd-watch-metadata yt-formatted-string") || metaContent(document, "property", "og:title") || metadata.title);
  if (!videoId) {
    emitDiagnostic(options, "youtube_metadata_extracted", {
      status: "failed",
      message: "Missing YouTube video id."
    });
    return failure("Missing YouTube video id.");
  }
  if (!title || title === "YouTube") {
    emitDiagnostic(options, "youtube_metadata_extracted", {
      status: "failed",
      message: "Missing YouTube video title."
    });
    return failure("Missing YouTube video title.");
  }
  const channel = cleanText(
    text(document, "ytd-watch-metadata ytd-channel-name a") ||
    text(document, "#owner ytd-channel-name a") ||
    metaContent(document, "itemprop", "name") ||
    metadata.byline ||
    ""
  );
  emitDiagnostic(options, "youtube_metadata_extracted", {
    status: "ok",
    safe_metadata: {
      has_title: "true",
      has_channel: channel ? "true" : "false",
      video_id_present: "true"
    }
  });
  const initialSegments = extractRenderedTranscriptSegments(document);
  let segments = initialSegments;
  let transcriptSource = segments.length > 0 ? "rendered_dom" : "unavailable";
  let transcriptReason = segments.length > 0
    ? "rendered_transcript_segments_found"
    : "transcript_panel_not_rendered";
  const discoveryEnabled = Boolean(options.discoverTranscript);

  if (segments.length === 0 && discoveryEnabled) {
    // PRIMARY: pull the transcript straight from the caption track in the page's
    // player response. This needs no transcript-panel UI, captures auto-generated
    // captions too, and is invisible to the viewer — so it does not depend on
    // fragile DOM selectors or the panel rendering in time.
    const api = await extractTranscriptViaApi(document, options);
    if (api.segments.length > 0) {
      segments = api.segments;
      transcriptSource = api.source;
      transcriptReason = api.reason;
    } else {
      // FALLBACK: open and scrape the rendered transcript panel (non-disruptive).
      const rowCount0 = transcriptRows(document).length;
      emitDiagnostic(options, "youtube_transcript_discovery_started", {
        status: "started",
        safe_metadata: {
          initial_segment_count: "0",
          initial_row_count: String(rowCount0),
          panel_open: String(transcriptPanelOpen(document)),
          api_reason: api.reason
        }
      });
      const discovery = await discoverTranscript(document, options);
      const domSegments = discovery.segments?.length
        ? discovery.segments
        : extractRenderedTranscriptSegments(document);
      emitDiagnostic(options, "youtube_transcript_discovery_finished", {
        status: domSegments.length > 0 ? "ok" : "unavailable",
        message: domSegments.length > 0 ? "Rendered transcript segments appeared." : "No rendered transcript segments found.",
        safe_metadata: {
          segment_count: String(domSegments.length),
          row_count: String(discovery.rowCount),
          panel_open: String(discovery.panelOpened),
          elapsed_ms: String(Date.now() - startedAt),
          reason: domSegments.length > 0 ? "rendered_transcript_segments_found" : discovery.reason
        }
      });
      if (domSegments.length > 0) {
        segments = domSegments;
        transcriptSource = "rendered_dom";
        transcriptReason = "rendered_transcript_segments_found";
      } else {
        transcriptSource = "unavailable";
        transcriptReason = discovery.reason;
      }
    }
  }

  const transcriptStatus = segments.length > 0 ? "full" : "unavailable";
  emitDiagnostic(options, "youtube_segments_extracted", {
    status: segments.length > 0 ? "ok" : "unavailable",
    message: segments.length > 0 ? "Rendered transcript segments found." : "No rendered transcript segments found.",
    safe_metadata: {
      segment_count: String(segments.length),
      transcript_status: transcriptStatus,
      transcript_source: transcriptSource,
      transcript_reason: transcriptReason
    }
  });
  return {
    ok: true,
    extractor_version: YOUTUBE_EXTRACTOR_VERSION,
    transcript_status: transcriptStatus,
    transcript_source: transcriptSource,
    transcript_reason: transcriptReason,
    metadata: {
      title,
      channel: channel || undefined,
      video_id: videoId,
      canonical_url: canonicalYouTubeUrl(videoId),
      published_at: metadata.published_at
    },
    segments
  };
}

function emitDiagnostic(options, event, detail = {}) {
  if (typeof options.onDiagnostic !== "function") return;
  options.onDiagnostic({
    component: "youtube_extractor",
    event,
    status: detail.status,
    message: detail.message,
    safe_metadata: detail.safe_metadata || {}
  });
}

// Pull the transcript from the page's caption track (the timedtext endpoint
// referenced by the player response). This is the reliable, invisible path: it
// covers auto-generated captions, needs no UI interaction, and does not depend on
// the transcript panel rendering. Returns { segments, reason }; never throws.
// Fetch the transcript as data, trying the most reliable source first:
//   1. innertube get_transcript — the authenticated API YouTube's own transcript
//      panel uses. Runs in the content script's real session, so the botguard/pot
//      context is present; returns clean JSON segments, fully invisible.
//   2. caption-track timedtext — best-effort; YouTube pot-gates this for some
//      (auto-caption) videos, so it can come back empty.
// Returns { segments, source, reason }; never throws. An empty result lets the
// caller fall back to scraping the rendered transcript panel.
async function extractTranscriptViaApi(document, options) {
  const context = await loadYouTubeContext(document, options);

  if (context.innertube.transcriptParams) {
    emitDiagnostic(options, "youtube_innertube_transcript_started", {
      status: "started",
      safe_metadata: { has_params: "true", player_response_source: context.source }
    });
    const transcript = await fetchInnertubeTranscript(context.innertube, document, options);
    emitDiagnostic(options, transcript.segments.length > 0 ? "youtube_innertube_transcript_succeeded" : "youtube_innertube_transcript_failed", {
      status: transcript.segments.length > 0 ? "ok" : "unavailable",
      message: transcript.segments.length > 0 ? "Fetched transcript via get_transcript." : "get_transcript returned no segments.",
      safe_metadata: {
        segment_count: String(transcript.segments.length),
        http_status: String(transcript.status),
        authorized: String(transcript.authorized)
      }
    });
    if (transcript.segments.length > 0) {
      return { segments: transcript.segments, source: "innertube_transcript", reason: "innertube_transcript_fetched" };
    }
  } else {
    emitDiagnostic(options, "youtube_innertube_transcript_failed", {
      status: "unavailable",
      message: "No transcript params present in page.",
      safe_metadata: { has_params: "false", player_response_source: context.source }
    });
  }

  const tracks = captionTracksFromPlayerResponse(context.playerResponse);
  emitDiagnostic(options, "youtube_caption_tracks_found", {
    status: tracks.length > 0 ? "ok" : "unavailable",
    message: tracks.length > 0 ? "Caption tracks present in player response." : "No caption tracks in player response.",
    safe_metadata: {
      track_count: String(tracks.length),
      languages: tracks.map((track) => track.languageCode).filter(Boolean).slice(0, 8).join(","),
      player_response_source: context.source
    }
  });
  const track = pickCaptionTrack(tracks);
  if (track) {
    emitDiagnostic(options, "youtube_timedtext_fetch_started", {
      status: "started",
      safe_metadata: { language: track.languageCode || "", kind: track.kind || "manual" }
    });
    const segments = await fetchTimedText(track, document, options);
    emitDiagnostic(options, segments.length > 0 ? "youtube_timedtext_fetch_succeeded" : "youtube_timedtext_fetch_failed", {
      status: segments.length > 0 ? "ok" : "unavailable",
      message: segments.length > 0 ? "Fetched caption-track transcript." : "Caption-track fetch returned no segments.",
      safe_metadata: { segment_count: String(segments.length), language: track.languageCode || "", kind: track.kind || "manual" }
    });
    if (segments.length > 0) {
      return { segments, source: "caption_track", reason: "caption_track_fetched" };
    }
  }
  return {
    segments: [],
    source: "unavailable",
    reason: tracks.length > 0 ? "caption_track_empty" : "transcript_api_unavailable"
  };
}

// Load the data the transcript APIs need. The content script's isolated world
// cannot read window.ytInitialPlayerResponse, and YouTube removes the inline
// bootstrap <script> from the DOM after it runs, so we fetch the watch-page HTML
// (server-rendered) which reliably carries the player response, the innertube API
// key/client version, and the get_transcript params.
async function loadYouTubeContext(document, options = {}) {
  const expectedId = videoIdFromLocation(document.location.href);
  // The DOM player response can be stale after SPA navigation (or during an ad):
  // an old ytInitialPlayerResponse for a *different* video may linger while the URL
  // already points at the current one. Only trust the DOM copy if its videoId
  // matches the URL; otherwise fall back to a fresh HTML fetch.
  let playerResponse = parsePlayerResponse(document);
  if (playerResponse && !playerResponseMatchesVideo(playerResponse, expectedId)) {
    playerResponse = null;
  }
  let source = playerResponse ? "dom" : "none";
  const html = await fetchPageHtml(document, options);
  if (!playerResponse && html) {
    const fromHtml = parsePlayerResponseFromText(html);
    if (fromHtml && playerResponseMatchesVideo(fromHtml, expectedId)) {
      playerResponse = fromHtml;
      source = "page_html";
    }
  }
  const innertube = extractInnertubeContext(html || documentHtml(document) || "");
  return { playerResponse, source, innertube };
}

function playerResponseMatchesVideo(playerResponse, expectedId) {
  // When we cannot determine the expected id, do not reject (avoid false negatives).
  if (!expectedId) return true;
  const responseId = playerResponse?.videoDetails?.videoId;
  // Some player responses omit videoDetails; accept those rather than discard a
  // possibly-valid response, but reject a clear id mismatch.
  return !responseId || responseId === expectedId;
}

function documentHtml(document) {
  try {
    return document.documentElement?.outerHTML || null;
  } catch {
    return null;
  }
}

function extractInnertubeContext(html) {
  const match = (pattern) => (html.match(pattern) || [])[1];
  return {
    apiKey: match(/"INNERTUBE_API_KEY":"([^"]+)"/),
    clientVersion: match(/"INNERTUBE_CONTEXT_CLIENT_VERSION":"([^"]+)"/) || match(/"clientVersion":"([^"]+)"/),
    transcriptParams: match(/"getTranscriptEndpoint":\{"params":"([^"]+)"/),
    visitorData: match(/"VISITOR_DATA":"([^"]+)"/) || match(/"visitorData":"([^"]+)"/)
  };
}

async function fetchInnertubeTranscript(innertube, document, options = {}) {
  const { apiKey, clientVersion, transcriptParams, visitorData } = innertube;
  if (!apiKey || !clientVersion || !transcriptParams) {
    return { segments: [], status: 0, authorized: false };
  }
  const view = document.defaultView;
  const fetchFn = view?.fetch || (typeof fetch === "function" ? fetch : null);
  if (!fetchFn) return { segments: [], status: 0, authorized: false };
  const client = { clientName: "WEB", clientVersion, hl: "en", gl: "US" };
  if (visitorData) client.visitorData = visitorData;
  const headers = {
    "Content-Type": "application/json",
    "X-Youtube-Client-Name": "1",
    "X-Youtube-Client-Version": clientVersion
  };
  if (visitorData) headers["X-Goog-Visitor-Id"] = visitorData;
  // Authenticated innertube calls need a SAPISIDHASH Authorization header; cookies
  // alone yield FAILED_PRECONDITION. The page's own JS adds this; we replicate it.
  const auth = await sapisidHashAuthorization(document);
  if (auth) {
    headers["Authorization"] = auth;
    headers["X-Origin"] = "https://www.youtube.com";
  }
  let response;
  try {
    response = await fetchWithTimeout(fetchFn, `https://www.youtube.com/youtubei/v1/get_transcript?key=${encodeURIComponent(apiKey)}&prettyPrint=false`, {
      method: "POST",
      credentials: "include",
      headers,
      body: JSON.stringify({ context: { client }, params: transcriptParams })
    }, view, options.transcriptApiTimeoutMs);
  } catch {
    return { segments: [], status: -1, authorized: Boolean(auth) };
  }
  if (!response?.ok) {
    return { segments: [], status: response?.status ?? 0, authorized: Boolean(auth) };
  }
  let data;
  try {
    data = await response.json();
  } catch {
    return { segments: [], status: response.status, authorized: Boolean(auth) };
  }
  return { segments: parseTranscriptSegments(data), status: response.status, authorized: Boolean(auth) };
}

// Compute the SAPISIDHASH used by YouTube's web client for authenticated innertube
// requests: SHA1("<unix_ts> <SAPISID> <origin>"). Reads SAPISID from document.cookie
// (the page's own JS reads it the same way, so it is not httpOnly for this origin).
// Returns null when unavailable so the request degrades to credentials-only.
async function sapisidHashAuthorization(document) {
  try {
    const view = document.defaultView;
    const subtle = view?.crypto?.subtle || (typeof crypto !== "undefined" ? crypto.subtle : null);
    if (!subtle) return null;
    const cookies = document.cookie || "";
    const sapisid = cookieValue(cookies, "SAPISID") || cookieValue(cookies, "__Secure-3PAPISID");
    if (!sapisid) return null;
    const origin = "https://www.youtube.com";
    const timestamp = Math.floor(Date.now() / 1000);
    const bytes = new TextEncoder().encode(`${timestamp} ${sapisid} ${origin}`);
    const digest = await subtle.digest("SHA-1", bytes);
    const hex = [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    return `SAPISIDHASH ${timestamp}_${hex}`;
  } catch {
    return null;
  }
}

function cookieValue(cookies, name) {
  const match = cookies.match(new RegExp(`(?:^|;\\s*)${name}=([^;]+)`));
  return match ? match[1] : null;
}

// Walk the get_transcript response for transcriptSegmentRenderer nodes, which
// carry startMs and a snippet of runs. Order by start time and de-duplicate.
function parseTranscriptSegments(data) {
  const seen = new Set();
  const segments = [];
  const walk = (node) => {
    if (Array.isArray(node)) {
      node.forEach(walk);
      return;
    }
    if (!node || typeof node !== "object") return;
    const renderer = node.transcriptSegmentRenderer;
    if (renderer) {
      const t = Number(renderer.startMs) / 1000;
      const runs = renderer.snippet?.runs || [];
      const segmentText = cleanText(runs.map((run) => run.text || "").join(""));
      if (Number.isFinite(t) && segmentText) {
        const key = `${Math.floor(t * 1000)}:${segmentText}`;
        if (!seen.has(key)) {
          seen.add(key);
          segments.push({ t, text: segmentText });
        }
      }
    }
    for (const value of Object.values(node)) walk(value);
  };
  walk(data);
  segments.sort((left, right) => left.t - right.t);
  return segments;
}

async function fetchPageHtml(document, options = {}) {
  const view = document.defaultView;
  const fetchFn = view?.fetch || (typeof fetch === "function" ? fetch : null);
  const url = document.location?.href;
  if (!fetchFn || !url) return null;
  try {
    const response = await fetchWithTimeout(fetchFn, url, { credentials: "include" }, view, options.transcriptApiTimeoutMs);
    if (!response?.ok) return null;
    return await response.text();
  } catch {
    return null;
  }
}

function parsePlayerResponse(document) {
  for (const script of document.querySelectorAll("script")) {
    const found = parsePlayerResponseFromText(script.textContent || "");
    if (found) return found;
  }
  return null;
}

function parsePlayerResponseFromText(text) {
  const marker = text.indexOf("ytInitialPlayerResponse");
  if (marker === -1) return null;
  const braceStart = text.indexOf("{", marker);
  if (braceStart === -1) return null;
  const json = balancedJsonAt(text, braceStart);
  if (!json) return null;
  try {
    return JSON.parse(json);
  } catch {
    return null;
  }
}

function balancedJsonAt(text, start) {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = start; index < text.length; index += 1) {
    const character = text[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === "\"") inString = false;
      continue;
    }
    if (character === "\"") inString = true;
    else if (character === "{") depth += 1;
    else if (character === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(start, index + 1);
    }
  }
  return null;
}

function captionTracksFromPlayerResponse(playerResponse) {
  const tracks = playerResponse?.captions?.playerCaptionsTracklistRenderer?.captionTracks;
  // baseUrl comes from page-controlled player-response JSON and is fetched with
  // credentials, so drop any track whose URL is not a trusted YouTube origin —
  // otherwise a malicious page could exfiltrate cookies / trigger SSRF.
  return Array.isArray(tracks)
    ? tracks.filter((track) => track && isTrustedYouTubeUrl(track.baseUrl))
    : [];
}

function isTrustedYouTubeUrl(rawUrl) {
  try {
    const url = new URL(rawUrl, "https://www.youtube.com");
    return url.protocol === "https:" && /(^|\.)youtube\.com$/.test(url.hostname);
  } catch {
    return false;
  }
}

function pickCaptionTrack(tracks) {
  if (!tracks.length) return null;
  // Prefer a manual English track, then auto English, then any manual, then any.
  const score = (track) => {
    const language = String(track.languageCode || "").toLowerCase();
    const isEnglish = language === "en" || language.startsWith("en");
    const isAuto = track.kind === "asr";
    return (isEnglish ? 0 : 2) + (isAuto ? 1 : 0);
  };
  return [...tracks].sort((left, right) => score(left) - score(right))[0];
}

async function fetchTimedText(track, document, options = {}) {
  const view = document.defaultView;
  const fetchFn = view?.fetch || (typeof fetch === "function" ? fetch : null);
  if (!fetchFn || !track?.baseUrl || !isTrustedYouTubeUrl(track.baseUrl)) return [];
  let response;
  try {
    response = await fetchWithTimeout(fetchFn, timedTextJson3Url(track.baseUrl), { credentials: "include" }, view, options.transcriptApiTimeoutMs);
  } catch {
    return [];
  }
  if (!response?.ok) return [];
  let data;
  try {
    data = await response.json();
  } catch {
    return [];
  }
  return parseJson3Transcript(data);
}

function timedTextJson3Url(baseUrl) {
  // Force json3, replacing any other format YouTube may have pinned on the URL.
  const stripped = baseUrl.replace(/([?&])fmt=[^&]*/g, "$1").replace(/[?&]$/, "");
  return `${stripped}${stripped.includes("?") ? "&" : "?"}fmt=json3`;
}

// Wraps fetch with an abort-based timeout so a hung connection can never stall the
// whole capture (the DOM-discovery branch is time-boxed; the API fetches were not).
async function fetchWithTimeout(fetchFn, url, options, view, timeoutMs) {
  const effectiveTimeout = Number.isFinite(timeoutMs) ? timeoutMs : 8000;
  const Controller = view?.AbortController || (typeof AbortController === "function" ? AbortController : null);
  if (!Controller) return fetchFn(url, options);
  const controller = new Controller();
  const timer = setTimeout(() => controller.abort(), effectiveTimeout);
  try {
    return await fetchFn(url, { ...options, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

function parseJson3Transcript(data) {
  const events = Array.isArray(data?.events) ? data.events : [];
  const seen = new Set();
  const segments = [];
  for (const event of events) {
    if (!Array.isArray(event.segs)) continue;
    const segmentText = cleanText(event.segs.map((seg) => seg.utf8 || "").join(""));
    if (!segmentText) continue;
    const t = Number(event.tStartMs) / 1000;
    if (!Number.isFinite(t)) continue;
    const key = `${Math.floor(t * 1000)}:${segmentText}`;
    if (seen.has(key)) continue;
    seen.add(key);
    segments.push({ t, text: segmentText });
  }
  return segments;
}

export function parseTimestamp(value = "") {
  const parts = value.trim().split(":");
  if (parts.length < 2 || parts.length > 3) return Number.NaN;
  // Each component is an integer (1-3 digits); reject fractional or oversized parts
  // so "1.5:30" or "999999:00" don't produce mis-timed segments.
  if (parts.some((part) => !/^\d{1,3}$/.test(part.trim()))) return Number.NaN;
  const numbers = parts.map(Number);
  if (numbers.some((part) => !Number.isFinite(part))) return Number.NaN;
  return numbers.reduce((total, part) => total * 60 + part, 0);
}

function extractRenderedTranscriptSegments(document) {
  const seen = new Set();
  const segments = [];
  for (const node of document.querySelectorAll(TRANSCRIPT_SEGMENT_SELECTOR)) {
    const { t, text: segmentText } = readTranscriptSegment(node);
    if (!Number.isFinite(t) || !segmentText) continue;
    const key = `${Math.floor(t * 1000)}:${segmentText}`;
    if (seen.has(key)) continue;
    seen.add(key);
    segments.push({ t, text: segmentText });
  }
  return segments;
}

// Matches both the legacy renderer and YouTube's newer "view-model" transcript
// markup. `transcript-segment-view-model` replaced `ytd-transcript-segment-renderer`.
const TRANSCRIPT_SEGMENT_SELECTOR = [
  "ytd-transcript-segment-renderer",
  "transcript-segment-view-model",
  "[class*='segment-text']",
  "ytd-transcript-segment-list-renderer [class*='segment']"
].join(", ");

function readTranscriptSegment(node) {
  // Prefer explicit timestamp/text containers (legacy + current class names).
  const timestampText =
    text(node, ".segment-timestamp") ||
    text(node, "[class*='timestamp']") ||
    text(node, "[class*='time']");
  const segmentText = cleanText(
    text(node, ".segment-text") ||
    text(node, "[class*='segment-text']") ||
    text(node, "yt-formatted-string") ||
    ""
  );
  if (Number.isFinite(parseTimestamp(timestampText)) && segmentText) {
    return { t: parseTimestamp(timestampText), text: segmentText };
  }
  // Fallback for the view-model markup: timestamp and text are not in known
  // containers, but the rendered row always leads with the "M:SS" stamp. Split on
  // the LEADING stamp only — never a String.replace of an arbitrary occurrence,
  // which would corrupt lines whose own text contains or looks like a timestamp.
  const full = cleanText(node.textContent || "");
  const match = full.match(/^(\d{1,3}:\d{2}(?::\d{2})?)\s*([\s\S]+)$/);
  if (match) {
    const t = parseTimestamp(match[1]);
    return { t, text: cleanText(stripDurationLabel(match[2], t)) };
  }
  return { t: Number.NaN, text: "" };
}

// The view-model timestamp element also renders a screen-reader duration label
// ("1 minute, 8 seconds") right after the visible stamp, which concatenates into
// the segment text. Strip that leading label, but ONLY when it exactly equals the
// segment's timestamp, so a spoken line that genuinely starts with a duration is
// never truncated.
function stripDurationLabel(text, seconds) {
  const label = text.match(/^(?:\d+\s*hours?\s*,?\s*)?(?:\d+\s*minutes?\s*,?\s*)?(?:\d+\s*seconds?)?/i);
  if (label && label[0] && durationLabelToSeconds(label[0]) === seconds) {
    return text.slice(label[0].length);
  }
  return text;
}

function durationLabelToSeconds(label) {
  const hours = label.match(/(\d+)\s*hours?/i);
  const minutes = label.match(/(\d+)\s*minutes?/i);
  const secs = label.match(/(\d+)\s*seconds?/i);
  return (hours ? Number(hours[1]) * 3600 : 0) +
    (minutes ? Number(minutes[1]) * 60 : 0) +
    (secs ? Number(secs[1]) : 0);
}

async function discoverTranscript(document, options = {}) {
  // The transcript panel fetches its rows asynchronously and can show a loading
  // spinner for several seconds; wait long enough for slow loads to resolve.
  const timeoutMs = options.transcriptDiscoveryTimeoutMs ?? 12000;
  const deadline = Date.now() + timeoutMs;
  const startedAt = Date.now();
  const seen = new Set();
  let buttonFound = false;
  let clickAttempted = false;
  // Tracks whether *we* opened the transcript panel, so we can close it again
  // afterward and leave the viewer's screen exactly as we found it.
  let openedByUs = false;
  const panelWasOpenInitially = transcriptPanelOpen(document);
  let panelOpened = panelWasOpenInitially;
  let rowCount = transcriptRows(document).length;
  const attemptedControls = new WeakSet();
  const attemptedExpanders = new WeakSet();
  const attemptedMenus = new WeakSet();

  const succeed = (segments) => {
    emitTranscriptDiagnostic(options, "transcript_extraction_succeeded", {
      status: "ok",
      safe_metadata: {
        segment_count: String(segments.length),
        elapsed_ms: String(Date.now() - startedAt)
      }
    });
    return {
      reason: "rendered_transcript_segments_found",
      panelOpened: true,
      rowCount: segments.length,
      segments
    };
  };

  // Already rendered (e.g. the user opened the transcript themselves). Read it
  // without clicking anything, and leave their open panel exactly as-is.
  const existing = extractRenderedTranscriptSegments(document);
  if (existing.length > 0) {
    return succeed(existing);
  }

  // Cloak the transcript panel so opening/closing it is invisible to the viewer:
  // off-screen positioning means no layout shift, opacity 0 means nothing is seen.
  // We close it and remove the cloak in `finally`, leaving the screen unchanged.
  const removeCloak = installTranscriptCloak(document);
  try {
    return await runTranscriptDiscovery();
  } finally {
    if (openedByUs) closeTranscriptPanel(document);
    removeCloak();
  }

  async function runTranscriptDiscovery() {
  while (Date.now() < deadline) {
    const segments = extractRenderedTranscriptSegments(document);
    if (segments.length > 0) {
      emitTranscriptDiagnostic(options, "transcript_rows_found", {
        status: "ok",
        safe_metadata: { row_count: String(transcriptRows(document).length), segment_count: String(segments.length) }
      }, seen);
      return succeed(segments);
    }

    panelOpened = transcriptPanelOpen(document);
    rowCount = transcriptRows(document).length;
    if (panelOpened) {
      // If the panel became open after our click attempts (it was not open when we
      // started), we are responsible for closing it again — even on the lazy-open
      // path where the synchronous post-click check missed it.
      if (clickAttempted && !panelWasOpenInitially) openedByUs = true;
      // The panel is open. Do NOT click anything else: clicking other
      // transcript-labeled controls toggles the panel shut and moves the page.
      // Detect a genuine "unavailable" message; otherwise just wait for the
      // lazy-rendered transcript rows to appear.
      emitTranscriptDiagnostic(options, "transcript_panel_opened", {
        status: "ok",
        safe_metadata: { row_count: String(rowCount), elapsed_ms: String(Date.now() - startedAt) }
      }, seen);
      const unavailable = transcriptUnavailableReason(document);
      if (unavailable) {
        emitTranscriptDiagnostic(options, unavailable.event, {
          status: "unavailable",
          safe_metadata: {
            reason: unavailable.reason,
            panel_open: "true",
            elapsed_ms: String(Date.now() - startedAt)
          }
        }, seen);
        return { reason: unavailable.reason, panelOpened, rowCount, segments: [] };
      }
      if (transcriptPanelLoading(document)) {
        emitTranscriptDiagnostic(options, "transcript_panel_loading", {
          status: "loading",
          safe_metadata: { elapsed_ms: String(Date.now() - startedAt) }
        }, seen);
      }
      await sleep(150);
      continue;
    }

    // The panel is not open yet. Try to open it with a single, non-scrolling
    // activation of the transcript control (falling back to description
    // expansion and the overflow menu only when no control is present).
    const transcriptControl = firstTranscriptControl(document, attemptedControls);
    if (transcriptControl) {
      buttonFound = true;
      emitTranscriptDiagnostic(options, "transcript_button_found", {
        status: "ok",
        safe_metadata: controlMetadata(transcriptControl)
      }, seen);
      if (!transcriptControl.actionable) {
        emitTranscriptDiagnostic(options, "transcript_control_not_actionable", {
          status: "unavailable",
          safe_metadata: controlMetadata(transcriptControl)
        });
        attemptedControls.add(transcriptControl.node);
        await sleep(100);
        continue;
      }
      attemptedControls.add(transcriptControl.actionable);
      attemptedControls.add(transcriptControl.node);
      clickAttempted = true;
      emitTranscriptDiagnostic(options, "transcript_button_click_attempted", {
        status: "started",
        safe_metadata: {
          ...controlMetadata(transcriptControl),
          click_method_used: "realistic_sequence"
        }
      });
      activateControl(transcriptControl.actionable);
      await sleep(300);
      const clickedSegments = extractRenderedTranscriptSegments(document);
      const openedAfterClick = transcriptPanelOpen(document);
      const opened = openedAfterClick || clickedSegments.length > 0;
      if (openedAfterClick) openedByUs = true;
      emitTranscriptDiagnostic(options, "transcript_button_click_completed", {
        status: opened ? "ok" : "unavailable",
        safe_metadata: {
          ...controlMetadata(transcriptControl),
          click_method_used: "realistic_sequence",
          panel_opened_after_click: String(opened),
          elapsed_ms: String(Date.now() - startedAt)
        }
      });
      // Always hand off to the next iteration: the panel-open branch reads the
      // (cloaked) panel once it opens, and we never click another control that
      // could toggle it shut.
      void opened;
      continue;
    }

    // Description-expansion and the overflow ("...") menu are VISIBLE actions
    // (the description grows; a Save/Report popup flashes), so they are disabled
    // by default to keep capture invisible. They run only when a caller explicitly
    // opts into visible discovery (used by tests).
    if (options.allowVisibleFallbacks) {
      const expander = firstDescriptionExpander(document, attemptedExpanders);
      if (expander) {
        attemptedExpanders.add(expander.node);
        emitTranscriptDiagnostic(options, "transcript_description_expand_attempted", {
          status: "started",
          safe_metadata: {
            control_category: expander.category,
            label_hint: transcriptLabelHint(expander.label),
            elapsed_ms: String(Date.now() - startedAt)
          }
        }, seen);
        activateControl(expander.node);
        await sleep(250);
        continue;
      }
      const menu = firstTranscriptMenuOpener(document, attemptedMenus);
      if (menu) {
        attemptedMenus.add(menu.node);
        emitTranscriptDiagnostic(options, "transcript_menu_open_attempted", {
          status: "started",
          safe_metadata: {
            control_category: menu.category,
            label_hint: transcriptLabelHint(menu.label),
            elapsed_ms: String(Date.now() - startedAt)
          }
        }, seen);
        activateControl(menu.node);
        await sleep(250);
        continue;
      }
    }

    // We already clicked the transcript control; just wait for the cloaked panel
    // to open rather than touching any other (visible) control.
    if (clickAttempted) {
      await sleep(200);
      continue;
    }

    emitTranscriptDiagnostic(options, "transcript_button_not_found", {
      status: "not_found",
      safe_metadata: {
        control_count: String(candidateControls(document).length),
        panel_open: String(panelOpened),
        elapsed_ms: String(Date.now() - startedAt)
      }
    }, seen);
    await sleep(250);
  }

  // Deadline reached. One last read in case rows rendered on the final tick.
  const finalSegments = extractRenderedTranscriptSegments(document);
  if (finalSegments.length > 0) {
    return succeed(finalSegments);
  }
  panelOpened = transcriptPanelOpen(document);
  rowCount = transcriptRows(document).length;
  // Capture the panel's element-tag fingerprint (no text) BEFORE closing it, so a
  // panel that renders rows we failed to match reveals which selectors drifted.
  const panelTags = panelOpened ? transcriptPanelFingerprint(document) : "";
  const stillLoading = panelOpened ? transcriptPanelLoading(document) : false;
  if (panelOpened) {
    emitTranscriptDiagnostic(options, "transcript_panel_opened", {
      status: "ok",
      safe_metadata: { row_count: String(rowCount), elapsed_ms: String(Date.now() - startedAt) }
    }, seen);
    emitTranscriptDiagnostic(options, "transcript_rows_empty", {
      status: "unavailable",
      safe_metadata: {
        row_count: String(rowCount),
        segment_count: "0",
        panel_tags: panelTags,
        is_loading: String(stillLoading)
      }
    }, seen);
    return {
      reason: stillLoading ? "transcript_panel_still_loading" : "transcript_rows_empty",
      panelOpened,
      rowCount,
      segments: []
    };
  }
  if (clickAttempted) {
    emitTranscriptDiagnostic(options, "transcript_panel_not_opened", {
      status: "unavailable",
      safe_metadata: { elapsed_ms: String(Date.now() - startedAt) }
    }, seen);
    return { reason: "transcript_panel_not_opened", panelOpened, rowCount, segments: [] };
  }
  emitTranscriptDiagnostic(options, "transcript_discovery_timed_out", {
    status: "unavailable",
    safe_metadata: {
      button_found: String(buttonFound),
      click_attempted: String(clickAttempted),
      panel_open: String(panelOpened),
      row_count: String(rowCount),
      elapsed_ms: String(Date.now() - startedAt)
    }
  });
  return {
    reason: buttonFound ? "transcript_discovery_timed_out" : "transcript_button_not_found",
    panelOpened,
    rowCount,
    segments: []
  };
  }
}

// Inject a stylesheet that renders the transcript engagement panel off-screen and
// fully transparent. The panel still populates its rows (it is in the viewport and
// in the DOM), but the viewer sees nothing and the page layout never shifts because
// the panel is taken out of normal flow. Returns a cleanup function.
function installTranscriptCloak(document) {
  try {
    const head = document.head || document.documentElement;
    if (!head || typeof document.createElement !== "function") return () => {};
    const style = document.createElement("style");
    style.id = "starlee-transcript-cloak";
    style.textContent = [
      "ytd-engagement-panel-section-list-renderer[target-id*='transcript'],",
      "ytd-engagement-panel-section-list-renderer[target-id*='transcript'] * {",
      "  transition: none !important; animation: none !important;",
      "}",
      // position:fixed takes the panel out of normal flow (no layout shift);
      // opacity:0 + a deep negative z-index make it invisible. It stays inside the
      // viewport (top/left 0) with real dimensions so YouTube still renders its rows.
      "ytd-engagement-panel-section-list-renderer[target-id*='transcript'] {",
      "  position: fixed !important; top: 0 !important; left: 0 !important;",
      "  width: 420px !important; height: 70vh !important;",
      "  opacity: 0 !important; pointer-events: none !important;",
      "  z-index: -2147483647 !important;",
      "}"
    ].join("\n");
    head.appendChild(style);
    return () => {
      try {
        style.remove();
      } catch {
        // ignore
      }
    };
  } catch {
    return () => {};
  }
}

// Best-effort close of the transcript engagement panel so capturing leaves the
// page exactly as the viewer had it. Never throws; leaving it open is harmless.
function closeTranscriptPanel(document) {
  try {
    const panel = document.querySelector(
      "ytd-engagement-panel-section-list-renderer[target-id*='transcript']"
    );
    if (!panel) return;
    const closeButton = panel.querySelector([
      "#visibility-button button",
      "#visibility-button",
      "button[aria-label*='close' i]",
      "[aria-label*='close transcript' i]"
    ].join(", "));
    if (closeButton) activateControl(closeButton);
  } catch {
    // best effort
  }
}

function firstTranscriptControl(document, attemptedControls = new WeakSet()) {
  const candidates = candidateControls(document)
    .map((node, index) => ({ node, index, label: controlLabel(node) }))
    .sort((left, right) => transcriptControlPriority(left.label) - transcriptControlPriority(right.label));
  for (const candidate of candidates) {
    const node = candidate.node;
    if (!isVisible(node)) continue;
    const label = candidate.label;
    if (!isTranscriptControl(label)) continue;
    const actionable = nearestActionableControl(node);
    if (attemptedControls.has(node) || (actionable && attemptedControls.has(actionable))) continue;
    return {
      node,
      label,
      category: controlCategory(node),
      candidateIndex: candidate.index,
      actionable,
      selectorStrategy: selectorStrategy(node)
    };
  }
  return null;
}

function candidateControls(document) {
  return [
    ...document.querySelectorAll([
      "button",
      "[role='button']",
      "tp-yt-paper-item",
      "ytd-menu-service-item-renderer",
      "ytd-button-renderer",
      "yt-button-shape",
      "ytd-video-description-transcript-section-renderer button",
      "ytd-transcript-renderer button",
      "[aria-label*='transcript' i]",
      "[title*='transcript' i]"
    ].join(", "))
  ];
}

function firstDescriptionExpander(document, attemptedExpanders = new WeakSet()) {
  for (const node of candidateControls(document)) {
    if (attemptedExpanders.has(node)) continue;
    if (!isVisible(node)) continue;
    const label = controlLabel(node);
    if (/more actions|action menu|options|menu/i.test(label)) continue;
    if (/\bmore\b|show more|expand/i.test(label)) {
      return { node, label, category: controlCategory(node) };
    }
  }
  return null;
}

function firstTranscriptMenuOpener(document, attemptedMenus = new WeakSet()) {
  for (const node of candidateControls(document)) {
    if (attemptedMenus.has(node)) continue;
    if (!isVisible(node)) continue;
    const label = controlLabel(node);
    if (/more actions|action menu|options|menu/i.test(label)) {
      return { node, label, category: controlCategory(node) };
    }
  }
  return null;
}

function isTranscriptControl(value) {
  return /show transcript|open transcript|view transcript|\btranscript\b/i.test(value);
}

function transcriptControlPriority(value) {
  if (/show transcript/i.test(value)) return 0;
  if (/open transcript|view transcript/i.test(value)) return 1;
  if (/\btranscript\b/i.test(value)) return 2;
  return 3;
}

function controlLabel(node) {
  return cleanText([
    node.getAttribute?.("aria-label"),
    node.getAttribute?.("title"),
    node.textContent
  ].filter(Boolean).join(" "));
}

function isVisible(node) {
  const view = node.ownerDocument?.defaultView;
  if (!view?.getComputedStyle) return true;
  const style = view.getComputedStyle(node);
  const opacity = Number.parseFloat(style.opacity);
  if (style.display === "none" || style.visibility === "hidden" || opacity === 0) return false;
  if (node.closest?.("[hidden], [aria-hidden='true']")) return false;
  return true;
}

function isDisabled(node) {
  return Boolean(
    node.disabled ||
    node.getAttribute?.("aria-disabled") === "true" ||
    node.hasAttribute?.("disabled")
  );
}

function hasBoundingBox(node) {
  const rect = node.getBoundingClientRect?.();
  if (!rect) return false;
  return rect.width > 0 && rect.height > 0;
}

function nearestActionableControl(node) {
  const actionable = node.closest?.([
    "button",
    "[role='button']",
    "tp-yt-paper-item",
    "ytd-menu-service-item-renderer",
    "ytd-button-renderer",
    "yt-button-shape",
    "a[href]"
  ].join(", "));
  if (!actionable || !isVisible(actionable) || isDisabled(actionable)) return null;
  return actionable;
}

function activateControl(node) {
  // Deliberately no scrollIntoView: it yanked the viewport to each control and
  // made the page visibly jump around. Synthetic pointer/click events do not
  // require the element to be scrolled into view.
  node.focus?.({ preventScroll: true });
  dispatchPointerMouseSequence(node);
  node.click?.();
  if (isButtonLike(node) && node.ownerDocument?.activeElement === node) {
    dispatchKeyboardActivation(node, "Enter");
    dispatchKeyboardActivation(node, " ");
  }
}

function dispatchPointerMouseSequence(node) {
  const view = node.ownerDocument?.defaultView;
  if (!view) return;
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) {
    const EventClass = type.startsWith("pointer") && typeof view.PointerEvent === "function"
      ? view.PointerEvent
      : view.MouseEvent;
    node.dispatchEvent(new EventClass(type, {
      bubbles: true,
      cancelable: true,
      composed: true,
      pointerType: "mouse",
      button: 0,
      buttons: type.endsWith("down") ? 1 : 0
    }));
  }
}

function dispatchKeyboardActivation(node, key) {
  const view = node.ownerDocument?.defaultView;
  if (!view?.KeyboardEvent) return;
  for (const type of ["keydown", "keyup"]) {
    node.dispatchEvent(new view.KeyboardEvent(type, {
      key,
      code: key === " " ? "Space" : key,
      bubbles: true,
      cancelable: true,
      composed: true
    }));
  }
}

function isButtonLike(node) {
  const tag = tagName(node);
  return tag === "button" || node.getAttribute?.("role") === "button";
}

function transcriptPanelOpen(document) {
  return Boolean(document.querySelector([
    "ytd-transcript-renderer",
    "ytd-transcript-search-panel-renderer",
    "ytd-engagement-panel-section-list-renderer[target-id*='transcript']:not([visibility='ENGAGEMENT_PANEL_VISIBILITY_HIDDEN'])",
    "ytd-engagement-panel-section-list-renderer[visibility='ENGAGEMENT_PANEL_VISIBILITY_EXPANDED'] ytd-transcript-segment-list-renderer"
  ].join(", ")));
}

function transcriptRows(document) {
  return [
    ...document.querySelectorAll([
      "ytd-transcript-segment-renderer",
      "transcript-segment-view-model",
      "ytd-transcript-segment-list-renderer [class*='segment']",
      "ytd-transcript-renderer [class*='segment']",
      "[data-purpose='transcript-segment']"
    ].join(", "))
  ];
}

// True when the transcript panel is showing a loading spinner — i.e. YouTube is
// still fetching the rows, so we should keep waiting rather than conclude empty.
function transcriptPanelLoading(document) {
  const panel = document.querySelector([
    "ytd-transcript-renderer",
    "ytd-transcript-search-panel-renderer",
    "ytd-engagement-panel-section-list-renderer[target-id*='transcript']"
  ].join(", "));
  if (!panel) return false;
  return Boolean(panel.querySelector("tp-yt-paper-spinner, yt-content-loading-renderer, [class*='spinner'], [class*='loading']"));
}

// Redacted structural fingerprint of the transcript panel: the most common
// element tag names and their counts (NO text content). Used to detect selector
// drift when the panel visibly renders rows we failed to scrape.
function transcriptPanelFingerprint(document) {
  const panel = document.querySelector([
    "ytd-transcript-renderer",
    "ytd-transcript-search-panel-renderer",
    "ytd-engagement-panel-section-list-renderer[target-id*='transcript']"
  ].join(", "));
  if (!panel) return "no_panel";
  const counts = new Map();
  for (const element of panel.querySelectorAll("*")) {
    const tag = (element.localName || "").toLowerCase();
    if (!tag) continue;
    if (!(tag.includes("-") || tag === "div" || tag === "button" || tag === "span")) continue;
    counts.set(tag, (counts.get(tag) || 0) + 1);
  }
  return [...counts.entries()]
    .sort((left, right) => right[1] - left[1])
    .slice(0, 12)
    .map(([tag, count]) => `${tag}:${count}`)
    .join(",") || "empty";
}

function transcriptUnavailableReason(document) {
  // Scan only the transcript panel itself. Scanning generic menus/dialogs/all
  // engagement panels matched unrelated UI (audio-track menus, settings popups)
  // and aborted discovery with a false "unavailable" verdict.
  const text = cleanText([
    ...document.querySelectorAll([
      "ytd-transcript-renderer",
      "ytd-transcript-search-panel-renderer",
      "ytd-engagement-panel-section-list-renderer[target-id*='transcript']"
    ].join(", "))
  ].map((node) => node.textContent || "").join(" "));
  if (!text) return null;
  if (/transcript (is )?not available|no transcript|transcript unavailable|captions unavailable/i.test(text)) {
    return { event: "transcript_disabled_by_video", reason: "transcript_disabled_by_video" };
  }
  // Keep this tied to the transcript wording so it cannot match an audio-track list.
  if (/transcript[^.]*language[^.]*not available|language[^.]*not available[^.]*transcript/i.test(text)) {
    return { event: "transcript_language_unavailable", reason: "transcript_language_unavailable" };
  }
  return null;
}

function emitTranscriptDiagnostic(options, event, detail = {}, seen) {
  if (seen?.has(event)) return;
  seen?.add(event);
  emitDiagnostic(options, event, detail);
}

function controlMetadata(control) {
  const actionable = control.actionable || null;
  return {
    candidate_index: String(control.candidateIndex ?? ""),
    candidate_role: control.node.getAttribute?.("role") || "",
    candidate_tag_name: tagName(control.node),
    candidate_aria_label_present: String(Boolean(control.node.getAttribute?.("aria-label"))),
    candidate_text_category: transcriptLabelHint(control.label),
    nearest_actionable_ancestor_tag: actionable ? tagName(actionable) : "",
    nearest_actionable_ancestor_role: actionable?.getAttribute?.("role") || "",
    selector_strategy_used: control.selectorStrategy || "unknown",
    visible: String(isVisible(control.node)),
    disabled: String(isDisabled(actionable || control.node)),
    bounding_box_present: String(hasBoundingBox(actionable || control.node)),
    control_category: control.category || "unknown",
    label_hint: transcriptLabelHint(control.label)
  };
}

function controlCategory(node) {
  const name = node.localName || node.tagName || "unknown";
  if (name === "button") return "button";
  if (name === "tp-yt-paper-item" || name === "ytd-menu-service-item-renderer") return "menu_item";
  if (name === "ytd-button-renderer" || name === "yt-button-shape") return "youtube_button";
  if (node.getAttribute?.("role") === "button") return "role_button";
  return String(name).toLowerCase().slice(0, 48);
}

function transcriptLabelHint(value = "") {
  if (/show transcript/i.test(value)) return "show_transcript";
  if (/open transcript/i.test(value)) return "open_transcript";
  if (/view transcript/i.test(value)) return "view_transcript";
  if (/transcript/i.test(value)) return "transcript";
  if (/more actions|action menu|options|menu/i.test(value)) return "menu";
  if (/\bmore\b|show more|expand/i.test(value)) return "expand";
  return "unknown";
}

function selectorStrategy(node) {
  const tag = tagName(node);
  if (tag === "button") return "button";
  if (tag === "tp-yt-paper-item" || tag === "ytd-menu-service-item-renderer") return "youtube_menu_item";
  if (tag === "ytd-button-renderer" || tag === "yt-button-shape") return "youtube_button_renderer";
  if (node.getAttribute?.("role") === "button") return "role_button";
  return "generic_control";
}

function tagName(node) {
  return String(node.localName || node.tagName || "").toLowerCase();
}

function safeHost(document) {
  try {
    return new URL(document.location.href).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function videoIdFromLocation(value) {
  try {
    const url = new URL(value);
    if (/(^|\.)youtu\.be$/.test(url.hostname)) {
      return cleanVideoId(url.pathname.split("/").filter(Boolean)[0]);
    }
    if (!/(^|\.)youtube\.com$/.test(url.hostname)) return "";
    if (url.pathname === "/watch") {
      return cleanVideoId(url.searchParams.get("v"));
    }
    // Shorts, live permalinks, and embeds carry the id as the path segment after
    // the type prefix; all have transcripts and are valid one-tap capture targets.
    const segments = url.pathname.split("/").filter(Boolean);
    if (segments.length === 2 && ["shorts", "live", "embed", "v"].includes(segments[0])) {
      return cleanVideoId(segments[1]);
    }
  } catch {
    return "";
  }
  return "";
}

function canonicalYouTubeUrl(videoId) {
  return `https://www.youtube.com/watch?v=${encodeURIComponent(videoId)}`;
}

function cleanVideoId(value) {
  // Guard against null/undefined (e.g. a missing `v` param): String(null) is the
  // literal "null", which would otherwise pass the charset check as a fake id.
  const cleaned = String(value ?? "").trim();
  return /^[A-Za-z0-9_-]{3,64}$/.test(cleaned) ? cleaned : "";
}

function cleanText(value = "") {
  return String(value).replace(/\s+/g, " ").trim();
}

function metaContent(document, attribute, value) {
  return document.querySelector(`meta[${attribute}="${value}"]`)?.getAttribute("content")?.trim();
}

function failure(error) {
  return {
    ok: false,
    extractor_version: YOUTUBE_EXTRACTOR_VERSION,
    transcript_status: "unavailable",
    transcript_source: "unavailable",
    transcript_reason: "extractor_failure",
    error
  };
}

function text(root, selector) {
  return root.querySelector(selector)?.textContent?.trim();
}
