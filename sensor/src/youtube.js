import { htmlMeta, pageMetadata } from "./metadata.js";

export const YOUTUBE_EXTRACTOR_VERSION = "youtube-dom-v1";

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
  const metadata = pageMetadata(document);
  const videoId = videoIdFromLocation(document.location.href);
  const title = cleanText(text(document, "h1.ytd-watch-metadata yt-formatted-string") || metaContent(document, "property", "og:title") || metadata.title);
  if (!videoId) {
    return failure("Missing YouTube video id.");
  }
  if (!title || title === "YouTube") {
    return failure("Missing YouTube video title.");
  }
  const channel = cleanText(
    text(document, "ytd-watch-metadata ytd-channel-name a") ||
    text(document, "#owner ytd-channel-name a") ||
    metaContent(document, "itemprop", "name") ||
    metadata.byline ||
    ""
  );
  const initialSegments = extractRenderedTranscriptSegments(document);
  const discoveryAttempted = Boolean(options.discoverTranscript) && initialSegments.length === 0;
  if (discoveryAttempted) {
    await discoverTranscript(document, options);
  }
  const segments = discoveryAttempted ? extractRenderedTranscriptSegments(document) : initialSegments;
  const transcriptStatus = segments.length > 0 ? "full" : "unavailable";
  const transcriptSource = segments.length > 0 ? "rendered_dom" : "unavailable";
  const transcriptReason = segments.length > 0
    ? "rendered_transcript_segments_found"
    : discoveryAttempted
      ? "transcript_discovery_unavailable_or_timed_out"
      : "transcript_panel_not_rendered";
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
  if (extractRenderedTranscriptSegments(document).length > 0) return;

  clickFirstTranscriptControl(document);
  while (Date.now() < deadline) {
    await sleep(250);
    if (extractRenderedTranscriptSegments(document).length > 0) return;
    if (clickFirstTranscriptControl(document)) {
      await sleep(250);
      if (extractRenderedTranscriptSegments(document).length > 0) return;
    }
  }
}

function clickFirstTranscriptControl(document) {
  for (const node of candidateControls(document)) {
    if (!isVisible(node)) continue;
    const label = controlLabel(node);
    if (isTranscriptControl(label)) {
      node.click?.();
      return true;
    }
  }
  if (clickDescriptionExpander(document)) {
    return true;
  }
  return false;
}

function candidateControls(document) {
  return [
    ...document.querySelectorAll("button, [role='button'], tp-yt-paper-item, ytd-menu-service-item-renderer, ytd-button-renderer")
  ];
}

function clickDescriptionExpander(document) {
  for (const node of candidateControls(document)) {
    if (!isVisible(node)) continue;
    const label = controlLabel(node);
    if (/\bmore\b|show more|expand/i.test(label)) {
      node.click?.();
      return true;
    }
  }
  return false;
}

function isTranscriptControl(value) {
  return /show transcript|open transcript|\btranscript\b/i.test(value);
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
