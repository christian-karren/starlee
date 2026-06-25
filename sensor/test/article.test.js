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

test("article extraction emits safe diagnostics without article body", () => {
  const diagnostics = [];
  const dom = new JSDOM(`<!doctype html>
    <title>Fallback title</title>
    <meta name="author" content="Starlee Test">
    <link rel="canonical" href="https://example.com/durable-browser-memory">
    <script type="application/ld+json">{"@type":"Article","isAccessibleForFree":true}</script>
    ${BODY}`, { url: "https://example.com/story?token=secret" });

  const payload = extractArticle(dom.window.document, {
    onDiagnostic: (event) => diagnostics.push(event)
  });

  assert.equal(payload.type, "article");
  assert.deepEqual(diagnostics.map((event) => event.event), [
    "article_extraction_started",
    "article_extraction_succeeded"
  ]);
  assert.equal(diagnostics.at(-1).safe_metadata.access, "public");
  assert.match(diagnostics.at(-1).safe_metadata.word_count, /^\d+$/);
  const serialized = JSON.stringify(diagnostics);
  assert.equal(serialized.includes("local Markdown record"), false);
  assert.equal(serialized.includes("secret"), false);
});

test("payload builder emits safe article counts", async () => {
  const diagnostics = [];
  const dom = new JSDOM(`<!doctype html><title>Fallback title</title>${BODY}`, { url: "https://example.com/story" });
  const payload = await capturePayload(dom.window.document, {
    onDiagnostic: (event) => diagnostics.push(event)
  });

  assert.equal(payload.type, "article");
  assert.ok(diagnostics.some((event) => event.event === "payload_built"));
  const built = diagnostics.find((event) => event.event === "payload_built");
  assert.equal(built.safe_metadata.payload_type, "article");
  assert.match(built.safe_metadata.text_char_count, /^\d+$/);
  assert.equal(JSON.stringify(diagnostics).includes("Starlee keeps a local Markdown record"), false);
});

test("routes YouTube before article detection", async () => {
  const dom = new JSDOM(`<title>Video</title>${BODY}`, { url: "https://www.youtube.com/watch?v=test" });
  const payload = await capturePayload(dom.window.document);
  assert.equal(payload.type, "youtube");
  assert.equal(payload.dom_extract.site, "youtube.com");
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("capture payload includes consumed_at engagement timestamp", async () => {
  const dom = new JSDOM(`<!doctype html><title>Fallback title</title>${BODY}`, { url: "https://example.com/story" });
  const payload = await capturePayload(dom.window.document);
  assert.equal(payload.type, "article");
  assert.match(payload.consumed_at, /^\d{4}-\d{2}-\d{2}T/);
});

test("capture payload rejects unsupported pages before posting", async () => {
  const dom = new JSDOM(`<!doctype html><title>Settings</title><main><button>Save</button></main>`, { url: "https://example.com/settings" });
  await assert.rejects(
    () => capturePayload(dom.window.document),
    /does not look like an article or YouTube video/
  );
});
