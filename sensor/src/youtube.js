import { htmlMeta, pageMetadata } from "./metadata.js";

export function isYouTubeWatch(document) {
  const url = new URL(document.location.href);
  return /(^|\.)youtube\.com$/.test(url.hostname) && url.pathname === "/watch" && url.searchParams.has("v");
}

export function extractYouTube(document) {
  const metadata = pageMetadata(document);
  const title = text(document, "h1.ytd-watch-metadata yt-formatted-string") || metadata.title;
  const channel = text(document, "ytd-watch-metadata ytd-channel-name a") || metadata.byline;
  const transcript = [...document.querySelectorAll("ytd-transcript-segment-renderer, ytd-transcript-segment-list-renderer [class*='segment']")]
    .map((node) => {
      const timestampText = text(node, ".segment-timestamp") || text(node, "[class*='timestamp']");
      const segmentText = text(node, ".segment-text") || text(node, "yt-formatted-string") || "";
      return { t: parseTimestamp(timestampText), text: segmentText };
    })
    .filter((segment) => Number.isFinite(segment.t) && segment.text);
  return {
    version: 1,
    type: "youtube",
    url: metadata.canonical || document.location.href,
    access: "restricted",
    dom_extract: {
      title,
      byline: channel,
      site: "youtube.com",
      published_at: metadata.published_at,
      text: "",
      html_meta: htmlMeta(document)
    },
    transcript,
    tags: []
  };
}

export function parseTimestamp(value = "") {
  const parts = value.trim().split(":").map(Number);
  if (!parts.length || parts.some(Number.isNaN)) return Number.NaN;
  return parts.reduce((total, part) => total * 60 + part, 0);
}

function text(root, selector) { return root.querySelector(selector)?.textContent?.trim(); }

