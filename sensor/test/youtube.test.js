import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { extractYouTube, extractYouTubeResult, isYouTubeWatch, parseTimestamp, YOUTUBE_EXTRACTOR_VERSION } from "../src/youtube.js";

test("parses transcript timestamps", () => {
  assert.equal(parseTimestamp("1:02"), 62);
  assert.equal(parseTimestamp("1:02:03"), 3723);
  assert.equal(parseTimestamp("01:02:03"), 3723);
  assert.equal(Number.isNaN(parseTimestamp("hello")), true);
  assert.equal(Number.isNaN(parseTimestamp("1")), true);
});

test("extracts rendered transcript segments", async () => {
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Local-first demo">
    <ytd-watch-metadata><ytd-channel-name><a>Starlee Channel</a></ytd-channel-name></ytd-watch-metadata>
    <ytd-transcript-segment-renderer><span class="segment-timestamp">1:02</span><yt-formatted-string class="segment-text">Hello brain</yt-formatted-string></ytd-transcript-segment-renderer>`, { url: "https://www.youtube.com/watch?v=test_id" });
  const payload = await extractYouTube(dom.window.document);
  assert.deepEqual(payload.transcript, [{ t: 62, text: "Hello brain" }]);
  assert.equal(payload.access, "restricted");
  assert.equal(payload.url, "https://www.youtube.com/watch?v=test_id");
  assert.equal(payload.dom_extract.title, "Local-first demo");
  assert.equal(payload.dom_extract.byline, "Starlee Channel");
  assert.equal(payload.transcript_status, "full");
  assert.equal(payload.transcript_source, "rendered_dom");
  assert.equal(payload.transcript_reason, "rendered_transcript_segments_found");
  assert.equal(payload.extractor_version, YOUTUBE_EXTRACTOR_VERSION);
});

test("captures useful YouTube metadata when transcript is unavailable", async () => {
  const dom = new JSDOM(`<title>Video</title><h1 class="ytd-watch-metadata"><yt-formatted-string>Local-first demo</yt-formatted-string></h1>`, { url: "https://www.youtube.com/watch?v=test_id" });
  const payload = await extractYouTube(dom.window.document);
  assert.equal(payload.type, "youtube");
  assert.equal(payload.dom_extract.title, "Local-first demo");
  assert.deepEqual(payload.transcript, []);
  assert.equal(payload.transcript_status, "unavailable");
  assert.equal(payload.transcript_source, "unavailable");
  assert.equal(payload.transcript_reason, "transcript_panel_not_rendered");
});

test("detects only supported YouTube watch pages", () => {
  assert.equal(isYouTubeWatch(new JSDOM("", { url: "https://www.youtube.com/watch?v=abc123" }).window.document), true);
  assert.equal(isYouTubeWatch(new JSDOM("", { url: "https://music.youtube.com/watch?v=abc123" }).window.document), true);
  assert.equal(isYouTubeWatch(new JSDOM("", { url: "https://www.youtube.com/shorts/abc123" }).window.document), false);
  assert.equal(isYouTubeWatch(new JSDOM("", { url: "https://www.youtube.com/watch" }).window.document), false);
});

test("filters malformed and duplicate transcript segments", async () => {
  const dom = new JSDOM(`<meta property="og:title" content="Transcript fixture">
    <ytd-transcript-segment-renderer><span class="segment-timestamp">bad</span><yt-formatted-string class="segment-text">Skip me</yt-formatted-string></ytd-transcript-segment-renderer>
    <ytd-transcript-segment-renderer><span class="segment-timestamp">00:12</span><yt-formatted-string class="segment-text">Keep me</yt-formatted-string></ytd-transcript-segment-renderer>
    <ytd-transcript-segment-renderer><span class="segment-timestamp">00:12</span><yt-formatted-string class="segment-text">Keep me</yt-formatted-string></ytd-transcript-segment-renderer>
    <ytd-transcript-segment-renderer><span class="segment-timestamp">00:18</span><yt-formatted-string class="segment-text">   </yt-formatted-string></ytd-transcript-segment-renderer>`, { url: "https://www.youtube.com/watch?v=fixture123" });

  const result = await extractYouTubeResult(dom.window.document);

  assert.equal(result.ok, true);
  assert.deepEqual(result.segments, [{ t: 12, text: "Keep me" }]);
});

test("opens transcript controls when discovery is enabled", async () => {
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Discovery demo">
    <button id="transcript">Show transcript</button>`, {
    url: "https://www.youtube.com/watch?v=discover123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("transcript").addEventListener("click", () => {
    dom.window.document.body.insertAdjacentHTML("beforeend", `
      <ytd-transcript-segment-renderer>
        <span class="segment-timestamp">0:07</span>
        <yt-formatted-string class="segment-text">Found after click</yt-formatted-string>
      </ytd-transcript-segment-renderer>`);
  });

  const payload = await extractYouTube(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 50
  });

  assert.deepEqual(payload.transcript, [{ t: 7, text: "Found after click" }]);
  assert.equal(payload.transcript_status, "full");
  assert.equal(payload.transcript_reason, "rendered_transcript_segments_found");
});

test("records a discovery timeout reason when no transcript appears", async () => {
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="No captions demo">
    <button>More</button>`, {
    url: "https://www.youtube.com/watch?v=nocaptions123",
    pretendToBeVisual: true
  });

  const payload = await extractYouTube(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 1
  });

  assert.deepEqual(payload.transcript, []);
  assert.equal(payload.transcript_status, "unavailable");
  assert.equal(payload.transcript_reason, "transcript_discovery_unavailable_or_timed_out");
});

test("handles long transcript fixtures without losing order", async () => {
  const transcript = Array.from({ length: 80 }, (_, index) => `
    <ytd-transcript-segment-renderer>
      <span class="segment-timestamp">${Math.floor(index / 60)}:${String(index % 60).padStart(2, "0")}</span>
      <yt-formatted-string class="segment-text">Segment ${index}</yt-formatted-string>
    </ytd-transcript-segment-renderer>
  `).join("");
  const dom = new JSDOM(`<meta property="og:title" content="Long lecture">${transcript}`, { url: "https://www.youtube.com/watch?v=long123" });
  const payload = await extractYouTube(dom.window.document);

  assert.equal(payload.transcript.length, 80);
  assert.deepEqual(payload.transcript.at(0), { t: 0, text: "Segment 0" });
  assert.deepEqual(payload.transcript.at(-1), { t: 79, text: "Segment 79" });
});

test("returns explicit extractor failure for malformed watch pages", async () => {
  const missingTitle = new JSDOM(`<title>YouTube</title>`, { url: "https://www.youtube.com/watch?v=abc123" });
  const result = await extractYouTubeResult(missingTitle.window.document);

  assert.equal(result.ok, false);
  assert.equal(result.transcript_status, "unavailable");
  assert.equal(result.transcript_reason, "extractor_failure");
  assert.match(result.error, /title/i);
});
