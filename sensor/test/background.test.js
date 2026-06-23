import test from "node:test";
import assert from "node:assert/strict";
import { browserNameFromUserAgent } from "../src/browser.js";

test("detects Safari instead of falling back to Chrome", () => {
  assert.equal(
    browserNameFromUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_5) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15"),
    "Safari"
  );
  assert.equal(
    browserNameFromUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"),
    "Chrome"
  );
});
