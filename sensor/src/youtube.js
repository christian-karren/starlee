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
  const discoveryAttempted = Boolean(options.discoverTranscript) && initialSegments.length === 0;
  let discovery = {
    reason: initialSegments.length > 0 ? "rendered_transcript_segments_found" : "transcript_panel_not_rendered",
    panelOpened: transcriptPanelOpen(document),
    rowCount: transcriptRows(document).length
  };
  if (discoveryAttempted) {
    emitDiagnostic(options, "youtube_transcript_discovery_started", {
      status: "started",
      safe_metadata: {
        initial_segment_count: String(initialSegments.length),
        initial_row_count: String(discovery.rowCount),
        panel_open: String(discovery.panelOpened)
      }
    });
    discovery = await discoverTranscript(document, options);
  }
  const segments = discoveryAttempted ? extractRenderedTranscriptSegments(document) : initialSegments;
  const transcriptStatus = segments.length > 0 ? "full" : "unavailable";
  const transcriptSource = segments.length > 0 ? "rendered_dom" : "unavailable";
  const transcriptReason = segments.length > 0
    ? "rendered_transcript_segments_found"
    : discoveryAttempted
      ? discovery.reason
      : "transcript_panel_not_rendered";
  if (discoveryAttempted) {
    emitDiagnostic(options, "youtube_transcript_discovery_finished", {
      status: segments.length > 0 ? "ok" : "unavailable",
      message: segments.length > 0 ? "Rendered transcript segments appeared." : "No rendered transcript segments found.",
      safe_metadata: {
        segment_count: String(segments.length),
        row_count: String(discovery.rowCount),
        panel_open: String(discovery.panelOpened),
        elapsed_ms: String(Date.now() - startedAt),
        reason: transcriptReason
      }
    });
  }
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
  let panelOpened = transcriptPanelOpen(document);
  let rowCount = transcriptRows(document).length;
  const attemptedControls = new WeakSet();
  const attemptedExpanders = new WeakSet();
  const attemptedMenus = new WeakSet();
  if (extractRenderedTranscriptSegments(document).length > 0) {
    emitTranscriptDiagnostic(options, "transcript_extraction_succeeded", {
      status: "ok",
      safe_metadata: {
        segment_count: String(extractRenderedTranscriptSegments(document).length),
        elapsed_ms: String(Date.now() - startedAt)
      }
    });
    return { reason: "rendered_transcript_segments_found", panelOpened: true, rowCount };
  }

  while (Date.now() < deadline) {
    const segments = extractRenderedTranscriptSegments(document);
    if (segments.length > 0) {
      emitTranscriptDiagnostic(options, "transcript_rows_found", {
        status: "ok",
        safe_metadata: { row_count: String(transcriptRows(document).length), segment_count: String(segments.length) }
      }, seen);
      emitTranscriptDiagnostic(options, "transcript_extraction_succeeded", {
        status: "ok",
        safe_metadata: { segment_count: String(segments.length), elapsed_ms: String(Date.now() - startedAt) }
      });
      return { reason: "rendered_transcript_segments_found", panelOpened: true, rowCount: transcriptRows(document).length };
    }

    panelOpened = transcriptPanelOpen(document);
    rowCount = transcriptRows(document).length;
    if (panelOpened) {
      emitTranscriptDiagnostic(options, "transcript_panel_opened", {
        status: "ok",
        safe_metadata: { row_count: String(rowCount), elapsed_ms: String(Date.now() - startedAt) }
      }, seen);
      if (rowCount > 0) {
        emitTranscriptDiagnostic(options, "transcript_rows_empty", {
          status: "unavailable",
          safe_metadata: { row_count: String(rowCount), segment_count: "0" }
        }, seen);
      }
    }

    // Only trust an "unavailable" verdict once the transcript panel is actually
    // open. Checking before that let stray page text (e.g. an audio-track menu
    // matching /no language/) abort discovery before "Show transcript" was ever
    // clicked, producing a false `transcript_language_unavailable` in ~8ms.
    const unavailable = panelOpened ? transcriptUnavailableReason(document) : null;
    if (unavailable) {
      emitTranscriptDiagnostic(options, unavailable.event, {
        status: "unavailable",
        safe_metadata: {
          reason: unavailable.reason,
          panel_open: String(panelOpened),
          elapsed_ms: String(Date.now() - startedAt)
        }
      }, seen);
      return { reason: unavailable.reason, panelOpened, rowCount };
    }

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
      emitTranscriptDiagnostic(options, "transcript_button_click_completed", {
        status: openedAfterClick || clickedSegments.length > 0 ? "ok" : "unavailable",
        safe_metadata: {
          ...controlMetadata(transcriptControl),
          click_method_used: "realistic_sequence",
          panel_opened_after_click: String(openedAfterClick || clickedSegments.length > 0),
          elapsed_ms: String(Date.now() - startedAt)
        }
      });
      if (clickedSegments.length > 0) {
        emitTranscriptDiagnostic(options, "transcript_panel_opened", {
          status: "ok",
          safe_metadata: { row_count: String(transcriptRows(document).length), elapsed_ms: String(Date.now() - startedAt) }
        }, seen);
        emitTranscriptDiagnostic(options, "transcript_extraction_succeeded", {
          status: "ok",
          safe_metadata: { segment_count: String(clickedSegments.length), elapsed_ms: String(Date.now() - startedAt) }
        });
        return { reason: "rendered_transcript_segments_found", panelOpened: true, rowCount: transcriptRows(document).length };
      }
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
      if (extractRenderedTranscriptSegments(document).length > 0) {
        return { reason: "rendered_transcript_segments_found", panelOpened: true, rowCount: transcriptRows(document).length };
      }
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
      if (extractRenderedTranscriptSegments(document).length > 0) {
        return { reason: "rendered_transcript_segments_found", panelOpened: true, rowCount: transcriptRows(document).length };
      }
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

  panelOpened = transcriptPanelOpen(document);
  rowCount = transcriptRows(document).length;
  if (panelOpened && rowCount === 0) {
    emitTranscriptDiagnostic(options, "transcript_panel_opened", {
      status: "ok",
      safe_metadata: { row_count: "0", elapsed_ms: String(Date.now() - startedAt) }
    }, seen);
    emitTranscriptDiagnostic(options, "transcript_rows_empty", {
      status: "unavailable",
      safe_metadata: { row_count: "0", segment_count: "0" }
    }, seen);
    return { reason: "transcript_rows_empty", panelOpened, rowCount };
  }
  if (clickAttempted && !panelOpened) {
    emitTranscriptDiagnostic(options, "transcript_panel_not_opened", {
      status: "unavailable",
      safe_metadata: { elapsed_ms: String(Date.now() - startedAt) }
    }, seen);
    return { reason: "transcript_panel_not_opened", panelOpened, rowCount };
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
    rowCount
  };
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
  node.scrollIntoView?.({ block: "center", inline: "center", behavior: "instant" });
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
