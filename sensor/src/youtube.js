import { htmlMeta, pageMetadata } from "./metadata.js";

export const YOUTUBE_EXTRACTOR_VERSION = "youtube-dom-v3";

export function isYouTubeWatch(document) {
  const url = new URL(document.location.href);
  return /(^|\.)youtube\.com$/.test(url.hostname) && url.pathname === "/watch" && Boolean(url.searchParams.get("v"));
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
    const caption = await extractTranscriptViaCaptionTrack(document, options);
    if (caption.segments.length > 0) {
      segments = caption.segments;
      transcriptSource = "caption_track";
      transcriptReason = "caption_track_fetched";
    } else {
      // FALLBACK: open and scrape the rendered transcript panel (non-disruptive).
      const rowCount0 = transcriptRows(document).length;
      emitDiagnostic(options, "youtube_transcript_discovery_started", {
        status: "started",
        safe_metadata: {
          initial_segment_count: "0",
          initial_row_count: String(rowCount0),
          panel_open: String(transcriptPanelOpen(document)),
          caption_track_reason: caption.reason
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
async function extractTranscriptViaCaptionTrack(document, options) {
  const { playerResponse, source } = await loadPlayerResponse(document);
  if (!playerResponse) {
    emitDiagnostic(options, "youtube_caption_tracks_found", {
      status: "unavailable",
      message: "No player response found in page or page HTML.",
      safe_metadata: { track_count: "0", player_response_source: source }
    });
    return { segments: [], reason: "player_response_unavailable" };
  }
  const tracks = captionTracksFromPlayerResponse(playerResponse);
  emitDiagnostic(options, "youtube_caption_tracks_found", {
    status: tracks.length > 0 ? "ok" : "unavailable",
    message: tracks.length > 0 ? "Caption tracks present in player response." : "No caption tracks in player response.",
    safe_metadata: {
      track_count: String(tracks.length),
      languages: tracks.map((track) => track.languageCode).filter(Boolean).slice(0, 8).join(","),
      player_response_source: source
    }
  });
  const track = pickCaptionTrack(tracks);
  if (!track) {
    return { segments: [], reason: "caption_track_unavailable" };
  }
  emitDiagnostic(options, "youtube_timedtext_fetch_started", {
    status: "started",
    safe_metadata: { language: track.languageCode || "", kind: track.kind || "manual" }
  });
  const segments = await fetchTimedText(track, document);
  emitDiagnostic(options, segments.length > 0 ? "youtube_timedtext_fetch_succeeded" : "youtube_timedtext_fetch_failed", {
    status: segments.length > 0 ? "ok" : "unavailable",
    message: segments.length > 0 ? "Fetched caption-track transcript." : "Caption-track fetch returned no segments.",
    safe_metadata: {
      segment_count: String(segments.length),
      language: track.languageCode || "",
      kind: track.kind || "manual"
    }
  });
  return {
    segments,
    reason: segments.length > 0 ? "caption_track_fetched" : "caption_track_empty"
  };
}

// Obtain the player response (which carries the caption tracks). The content
// script runs in an isolated world, so it cannot read window.ytInitialPlayerResponse,
// and YouTube often removes the inline bootstrap <script> from the DOM after it
// runs. So: try the live DOM first (fast, no network), then fall back to fetching
// the watch page HTML, whose server-rendered ytInitialPlayerResponse reliably
// includes the caption tracks.
async function loadPlayerResponse(document) {
  const fromDom = parsePlayerResponse(document);
  if (fromDom) return { playerResponse: fromDom, source: "dom" };
  const html = await fetchPageHtml(document);
  if (html) {
    const fromHtml = parsePlayerResponseFromText(html);
    if (fromHtml) return { playerResponse: fromHtml, source: "page_html" };
  }
  return { playerResponse: null, source: "none" };
}

async function fetchPageHtml(document) {
  const view = document.defaultView;
  const fetchFn = view?.fetch || (typeof fetch === "function" ? fetch : null);
  const url = document.location?.href;
  if (!fetchFn || !url) return null;
  try {
    const response = await fetchFn(url, { credentials: "include" });
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
  return Array.isArray(tracks) ? tracks.filter((track) => track && track.baseUrl) : [];
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

async function fetchTimedText(track, document) {
  const view = document.defaultView;
  const fetchFn = view?.fetch || (typeof fetch === "function" ? fetch : null);
  if (!fetchFn || !track?.baseUrl) return [];
  let response;
  try {
    response = await fetchFn(timedTextJson3Url(track.baseUrl), { credentials: "include" });
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
  if (/[?&]fmt=/.test(baseUrl)) return baseUrl;
  return `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}fmt=json3`;
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
  if (parts.some((part) => !/^\d+(?:\.\d+)?$/.test(part.trim()))) return Number.NaN;
  const numbers = parts.map(Number);
  if (numbers.some((part) => !Number.isFinite(part))) return Number.NaN;
  return numbers.reduce((total, part) => total * 60 + part, 0);
}

function extractRenderedTranscriptSegments(document) {
  const seen = new Set();
  const segments = [];
  for (const node of document.querySelectorAll("ytd-transcript-segment-renderer, ytd-transcript-segment-list-renderer [class*='segment']")) {
    const timestampText = text(node, ".segment-timestamp") || text(node, "[class*='timestamp']");
    const segmentText = cleanText(text(node, ".segment-text") || text(node, "yt-formatted-string") || "");
    const t = parseTimestamp(timestampText);
    if (!Number.isFinite(t) || !segmentText) continue;
    const key = `${Math.floor(t * 1000)}:${segmentText}`;
    if (seen.has(key)) continue;
    seen.add(key);
    segments.push({ t, text: segmentText });
  }
  return segments;
}

async function discoverTranscript(document, options = {}) {
  const timeoutMs = options.transcriptDiscoveryTimeoutMs ?? 5000;
  const deadline = Date.now() + timeoutMs;
  const startedAt = Date.now();
  const seen = new Set();
  let buttonFound = false;
  let clickAttempted = false;
  // Tracks whether *we* opened the transcript panel, so we can close it again
  // afterward and leave the viewer's screen exactly as we found it.
  let openedByUs = false;
  let panelOpened = transcriptPanelOpen(document);
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
    // We already have the segment data in hand, so closing the panel (which
    // removes the rendered rows from the DOM) does not cost us anything.
    if (openedByUs) closeTranscriptPanel(document);
    return {
      reason: "rendered_transcript_segments_found",
      panelOpened: true,
      rowCount: segments.length,
      segments
    };
  };

  // Already rendered (e.g. the user opened the transcript themselves). Read it
  // without clicking anything, and leave their open panel as-is.
  const existing = extractRenderedTranscriptSegments(document);
  if (existing.length > 0) {
    return succeed(existing);
  }

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
        if (openedByUs) closeTranscriptPanel(document);
        return { reason: unavailable.reason, panelOpened, rowCount, segments: [] };
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
      // If the panel opened, hand off to the next iteration's panel-open branch
      // (so we never click again and toggle it shut). If it did NOT open, fall
      // through to try description expansion / the overflow menu this round.
      if (opened) continue;
    }

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
  if (openedByUs) closeTranscriptPanel(document);
  if (panelOpened) {
    emitTranscriptDiagnostic(options, "transcript_panel_opened", {
      status: "ok",
      safe_metadata: { row_count: String(rowCount), elapsed_ms: String(Date.now() - startedAt) }
    }, seen);
    emitTranscriptDiagnostic(options, "transcript_rows_empty", {
      status: "unavailable",
      safe_metadata: { row_count: String(rowCount), segment_count: "0" }
    }, seen);
    return { reason: "transcript_rows_empty", panelOpened, rowCount, segments: [] };
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
      "ytd-transcript-segment-list-renderer [class*='segment']",
      "ytd-transcript-renderer [class*='segment']",
      "[data-purpose='transcript-segment']"
    ].join(", "))
  ];
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
    if (/(^|\.)youtube\.com$/.test(url.hostname) && url.pathname === "/watch") {
      return cleanVideoId(url.searchParams.get("v"));
    }
  } catch {
    return "";
  }
  return "";
}

function canonicalYouTubeUrl(videoId) {
  return `https://www.youtube.com/watch?v=${encodeURIComponent(videoId)}`;
}

function cleanVideoId(value = "") {
  const cleaned = String(value).trim();
  return /^[A-Za-z0-9_-]{3,}$/.test(cleaned) ? cleaned : "";
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
