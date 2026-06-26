import { defineConfig } from "@playwright/test";

// Chrome extensions require a headed (non-headless) persistent context.
// CI should set DISPLAY or use xvfb-run.
export default defineConfig({
  testDir: "./test/e2e",
  timeout: 60_000,
  // Playwright workers run tests in parallel; extension contexts are isolated
  // per worker, so parallelism is safe.
  workers: 1,
  use: {
    // Extensions require headed mode — headless does not support --load-extension
    headless: false,
  },
});
