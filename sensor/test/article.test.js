import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { extractArticle } from "../src/article.js";
import { capturePayload } from "../src/payload.js";

const BODY = `
<article>
  <h1>A durable browser memory</h1>
  <p>People consume useful ideas every day, but those ideas disappear when they are not captured in a durable and searchable form.</p>
  <p>Starlee keeps a local Markdown record so knowledge can compound over time without sending the article to a hosted corpus.</p>
  <p>The browser extension observes the rendered page and the local engine owns durable storage, indexing, retrieval, and citation.</p>
  <p>This fixture is intentionally long enough for Mozilla Readability to identify it as a real article during automated tests.</p>
</article>`;

test("extracts normalized article metadata and public access signal", () => {
  const dom = new JSDOM(`<!doctype html>
    <title>Fallback title</title>
    <meta name="author" content="Starlee Test">
    <meta property="og:title" content="A durable browser memory">
    <meta property="og:site_name" content="Starlee Fixture">
    <meta property="article:published_time" content="2026-06-19">
    <link rel="canonical" href="https://example.com/durable-browser-memory">
    <script type="application/ld+json">{"@type":"Article","isAccessibleForFree":true}</script>
    ${BODY}`, { url: "https://example.com/story" });

  const payload = extractArticle(dom.window.document);
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
});

test("routes YouTube before article detection", () => {
  const dom = new JSDOM(`<title>Video</title>${BODY}`, { url: "https://www.youtube.com/watch?v=test" });
  const payload = capturePayload(dom.window.document);
  assert.equal(payload.type, "youtube");
  assert.equal(payload.dom_extract.site, "youtube.com");
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("capture payload includes consumed_at engagement timestamp", () => {
  const dom = new JSDOM(`<!doctype html><title>Fallback title</title>${BODY}`, { url: "https://example.com/story" });
  const payload = capturePayload(dom.window.document);
  assert.equal(payload.type, "article");
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("capture payload rejects unsupported pages before posting", () => {
  const dom = new JSDOM(`<!doctype html><title>Settings</title><main><button>Save</button></main>`, { url: "https://example.com/settings" });
  assert.throws(
    () => capturePayload(dom.window.document),
    /does not look like an article or YouTube video/
  );
});
