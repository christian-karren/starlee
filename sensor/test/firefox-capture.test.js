import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { capturePayload } from "../src/payload.js";
import { attachSelectedText } from "../src/selection.js";

const ARTICLE_BODY = `
<article>
  <h1>A durable browser memory</h1>
  <p>People consume useful ideas every day, but those ideas disappear when they are not captured in a durable and searchable form.</p>
  <p>Starlee keeps a local Markdown record so knowledge can compound over time without sending the article to a hosted corpus.</p>
  <p>The Firefox extension observes the rendered page and the local engine owns durable storage, indexing, retrieval, and citation.</p>
  <p>This fixture is intentionally long enough for Mozilla Readability to identify it as a real article during automated tests.</p>
</article>`;

test("Firefox target reuses article extraction payload shape expected by Chrome", async () => {
  const dom = new JSDOM(`<!doctype html>
    <title>Fallback title</title>
    <meta name="author" content="Starlee Test">
    <meta property="og:title" content="A durable browser memory">
    <meta property="og:site_name" content="Starlee Fixture">
    <meta property="article:published_time" content="2026-06-19">
    <meta name="description" content="A short public summary">
    <link rel="canonical" href="https://example.com/durable-browser-memory">
    <script type="application/ld+json">{"@type":"Article","isAccessibleForFree":true}</script>
    ${ARTICLE_BODY}`, { url: "https://example.com/story?token=private" });

  const payload = await capturePayload(dom.window.document);

  assert.equal(payload.version, 1);
  assert.equal(payload.type, "article");
  assert.equal(payload.url, "https://example.com/durable-browser-memory");
  assert.equal(payload.access, "public");
  assert.equal(payload.dom_extract.title, "A durable browser memory");
  assert.equal(payload.dom_extract.byline, "Starlee Test");
  assert.equal(payload.dom_extract.site, "Starlee Fixture");
  assert.equal(payload.dom_extract.published_at, "2026-06-19");
  assert.match(payload.dom_extract.text, /local Markdown record/);
  assert.equal(payload.dom_extract.html_meta["starlee:access_reason"], "schema:isAccessibleForFree=true");
  assert.equal(payload.dom_extract.html_meta.description, "A short public summary");
  assert.deepEqual(payload.tags, []);
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("Firefox target attaches selected text only to article payloads", async () => {
  const dom = new JSDOM(`<!doctype html><title>Article</title>${ARTICLE_BODY}`, {
    url: "https://example.com/selected-text"
  });
  const article = await capturePayload(dom.window.document);
  const withSelection = attachSelectedText(article, "  selected quote from the article  ");

  assert.equal(withSelection.type, "article");
  assert.equal(withSelection.dom_extract.selected_text, "selected quote from the article");

  const youtube = await capturePayload(youtubeDocumentWithTranscript().window.document);
  const unchanged = attachSelectedText(youtube, "selected transcript text");
  assert.equal(unchanged.type, "youtube");
  assert.equal(unchanged.dom_extract.selected_text, undefined);
});

test("Firefox target reuses YouTube metadata and rendered transcript extraction", async () => {
  const payload = await capturePayload(youtubeDocumentWithTranscript().window.document);

  assert.equal(payload.version, 1);
  assert.equal(payload.type, "youtube");
  assert.equal(payload.url, "https://www.youtube.com/watch?v=ffox1234567");
  assert.equal(payload.access, "restricted");
  assert.equal(payload.dom_extract.title, "Local-first demo");
  assert.equal(payload.dom_extract.byline, "Starlee Channel");
  assert.equal(payload.dom_extract.site, "youtube.com");
  assert.equal(payload.dom_extract.text, "");
  assert.deepEqual(payload.transcript, [
    { t: 62, text: "Hello Firefox capture" },
    { t: 125, text: "Transcript segments stay timestamped" }
  ]);
  assert.equal(payload.transcript_status, "full");
  assert.equal(payload.transcript_source, "rendered_dom");
  assert.equal(payload.transcript_reason, "rendered_transcript_segments_found");
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("Firefox target preserves YouTube transcript unavailable state", async () => {
  const dom = new JSDOM(`<!doctype html>
    <title>YouTube</title>
    <meta property="og:title" content="Local-first demo">
    <h1 class="ytd-watch-metadata"><yt-formatted-string>Local-first demo</yt-formatted-string></h1>
    <ytd-watch-metadata><ytd-channel-name><a>Starlee Channel</a></ytd-channel-name></ytd-watch-metadata>`, {
    url: "https://www.youtube.com/watch?v=noTrans1234"
  });

  const payload = await capturePayload(dom.window.document);

  assert.equal(payload.type, "youtube");
  assert.equal(payload.dom_extract.title, "Local-first demo");
  assert.deepEqual(payload.transcript, []);
  assert.equal(payload.transcript_status, "unavailable");
  assert.equal(payload.transcript_source, "unavailable");
  assert.equal(payload.transcript_reason, "transcript_panel_not_rendered");
  assert.equal(payload.dom_extract.html_meta["starlee:transcript_status"], "unavailable");
});

function youtubeDocumentWithTranscript() {
  return new JSDOM(`<!doctype html>
    <title>YouTube</title>
    <meta property="og:title" content="Local-first demo">
    <h1 class="ytd-watch-metadata"><yt-formatted-string>Local-first demo</yt-formatted-string></h1>
    <ytd-watch-metadata><ytd-channel-name><a>Starlee Channel</a></ytd-channel-name></ytd-watch-metadata>
    <ytd-transcript-segment-renderer>
      <span class="segment-timestamp">1:02</span>
      <yt-formatted-string class="segment-text">Hello Firefox capture</yt-formatted-string>
    </ytd-transcript-segment-renderer>
    <ytd-transcript-segment-renderer>
      <span class="segment-timestamp">2:05</span>
      <yt-formatted-string class="segment-text">Transcript segments stay timestamped</yt-formatted-string>
    </ytd-transcript-segment-renderer>`, {
    url: "https://www.youtube.com/watch?v=ffox1234567"
  });
}
