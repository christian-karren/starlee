import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { extractYouTube, parseTimestamp } from "../src/youtube.js";

test("parses transcript timestamps", () => {
  assert.equal(parseTimestamp("1:02"), 62);
  assert.equal(parseTimestamp("1:02:03"), 3723);
});

test("extracts rendered transcript segments", () => {
  const dom = new JSDOM(`<title>Video</title><ytd-transcript-segment-renderer><span class="segment-timestamp">1:02</span><yt-formatted-string class="segment-text">Hello brain</yt-formatted-string></ytd-transcript-segment-renderer>`, { url: "https://www.youtube.com/watch?v=test" });
  const payload = extractYouTube(dom.window.document);
  assert.deepEqual(payload.transcript, [{ t: 62, text: "Hello brain" }]);
  assert.equal(payload.access, "restricted");
});
