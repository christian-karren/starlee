import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { classifyAccess } from "../src/access.js";

test("schema.org public signal wins", () => {
  const dom = new JSDOM(`<script type="application/ld+json">{"@type":"Article","isAccessibleForFree":true}</script>`, { url: "https://example.com/story" });
  assert.equal(classifyAccess(dom.window.document).access, "public");
});

test("known paid domains and ambiguity fail closed", () => {
  const paid = new JSDOM(`<article>Story</article>`, { url: "https://www.nytimes.com/story" });
  const ambiguous = new JSDOM(`<article>Story</article>`, { url: "https://example.com/story" });
  assert.equal(classifyAccess(paid.window.document).access, "restricted");
  assert.equal(classifyAccess(ambiguous.window.document).access, "restricted");
});

