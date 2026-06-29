import test from "node:test";
import assert from "node:assert/strict";
import {
  browserNameFromUserAgent,
  createExtensionApi,
  requestTargetsBrowser
} from "../src/browser.js";

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

test("detects Firefox from user agent", () => {
  assert.equal(
    browserNameFromUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 15.5; rv:128.0) Gecko/20100101 Firefox/128.0"),
    "Firefox"
  );
});

test("capture requests are accepted only by their requested browser", () => {
  const cases = [
    ["Safari", ["Chrome", "Firefox"]],
    ["Firefox", ["Chrome", "Safari"]],
    ["Chrome", ["Safari", "Firefox"]]
  ];

  for (const [requested, rejectedBrowsers] of cases) {
    assert.equal(
      requestTargetsBrowser({ requested_browser: requested }, requested),
      true,
      `${requested} should accept its own requested_browser`
    );
    assert.equal(
      requestTargetsBrowser({ target_browser: requested }, requested),
      true,
      `${requested} should accept its own legacy target_browser`
    );
    for (const browser of rejectedBrowsers) {
      assert.equal(
        requestTargetsBrowser({ requested_browser: requested }, browser),
        false,
        `${browser} must reject ${requested}-targeted requests`
      );
    }
  }
});

test("capture requests with no browser target fail closed", () => {
  for (const browser of ["Chrome", "Safari", "Firefox"]) {
    assert.equal(requestTargetsBrowser({}, browser), false);
    assert.equal(requestTargetsBrowser({ requested_browser: null, target_browser: null }, browser), false);
    assert.equal(requestTargetsBrowser({ requested_browser: "" }, browser), false);
  }
});

test("browser adapter preserves Firefox promise-style APIs", async () => {
  const api = createExtensionApi({
    runtime: {
      getBrowserInfo: async () => ({ name: "Firefox" }),
      getURL: (path) => `moz-extension://id/${path}`,
      getManifest: () => ({ version: "0.1.0" }),
      sendMessage: async (message) => ({ echo: message.type }),
      onMessage: { addListener() {} }
    },
    storage: { local: {
      get: async (keys) => ({ keys }),
      set: async (value) => ({ stored: value })
    } },
    tabs: {
      query: async (query) => [{ id: 1, query }],
      sendMessage: async (tabId, message) => ({ tabId, message })
    },
    alarms: {
      create: async () => {},
      onAlarm: { addListener() {} }
    },
    action: {
      onClicked: { addListener() {} },
      setBadgeText: async () => {},
      setBadgeBackgroundColor: async () => {}
    }
  });

  assert.deepEqual(await api.storage.local.get(["captureToken"]), { keys: ["captureToken"] });
  assert.deepEqual(await api.runtime.sendMessage({ type: "STARLEE_STATUS" }), { echo: "STARLEE_STATUS" });
  assert.deepEqual(await api.tabs.query({ active: true }), [{ id: 1, query: { active: true } }]);
  assert.equal(api.runtime.getURL("build-info.json"), "moz-extension://id/build-info.json");
});

test("browser adapter supports Chrome callback-style APIs", async () => {
  const previousChrome = globalThis.chrome;
  globalThis.chrome = { runtime: {} };
  try {
    const api = createExtensionApi({
      runtime: {
        getURL: (path) => `chrome-extension://id/${path}`,
        getManifest: () => ({ version: "0.1.0" }),
        sendMessage: (message, callback) => callback({ echo: message.type }),
        onMessage: { addListener() {} }
      },
      storage: { local: {
        get: (keys, callback) => callback({ keys }),
        set: (_value, callback) => callback()
      } },
      tabs: {
        query: (query, callback) => callback([{ id: 2, query }]),
        sendMessage: (tabId, message, callback) => callback({ tabId, message })
      }
    });

    assert.deepEqual(await api.storage.local.get(["capturePort"]), { keys: ["capturePort"] });
    assert.deepEqual(await api.runtime.sendMessage({ type: "STARLEE_HELLO" }), { echo: "STARLEE_HELLO" });
    assert.deepEqual(await api.tabs.query({ currentWindow: true }), [{ id: 2, query: { currentWindow: true } }]);
  } finally {
    globalThis.chrome = previousChrome;
  }
});
