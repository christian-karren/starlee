import test from "node:test";
import assert from "node:assert/strict";
import { JSDOM } from "jsdom";
import { capturePayload } from "../src/payload.js";
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

test("does not abort on unrelated 'no language' page text before opening the panel", async () => {
  // Regression: an audio-track menu containing "No language available" used to
  // match the unavailability scan on the first loop and abort discovery in ~8ms
  // with a false transcript_language_unavailable, before "Show transcript" ran.
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="False negative demo">
    <ytd-menu-popup-renderer>Audio track: No language available</ytd-menu-popup-renderer>
    <button id="transcript">Show transcript</button>`, {
    url: "https://www.youtube.com/watch?v=falseneg123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("transcript").addEventListener("click", () => {
    dom.window.document.body.insertAdjacentHTML("beforeend", `
      <ytd-transcript-renderer>
        <ytd-transcript-segment-renderer>
          <span class="segment-timestamp">0:09</span>
          <yt-formatted-string class="segment-text">Real transcript line</yt-formatted-string>
        </ytd-transcript-segment-renderer>
      </ytd-transcript-renderer>`);
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 700,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.equal(result.transcript_reason, "rendered_transcript_segments_found");
  assert.ok(!events.some((event) => event.event === "transcript_language_unavailable"));
});

test("still reports language-unavailable when the open transcript panel says so", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Genuinely unavailable">
    <ytd-transcript-renderer>Transcript language is not available for this video.</ytd-transcript-renderer>`, {
    url: "https://www.youtube.com/watch?v=nolang123",
    pretendToBeVisual: true
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 200,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "unavailable");
  assert.equal(result.transcript_reason, "transcript_language_unavailable");
});

test("waits for lazy-rendered rows after the panel opens instead of re-clicking", async () => {
  // Reproduces the live trace: the panel opens with 0 rows, then YouTube renders
  // the lines a beat later. Discovery must poll (not click again, which would
  // toggle the panel shut) and still capture the transcript.
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Lazy rows demo">
    <button id="transcript">Show transcript</button>`, {
    url: "https://www.youtube.com/watch?v=lazyrows123",
    pretendToBeVisual: true
  });
  const doc = dom.window.document;
  let clicks = 0;
  doc.getElementById("transcript").addEventListener("click", () => {
    clicks += 1;
    // First click opens an empty panel; rows arrive ~250ms later.
    if (clicks === 1) {
      doc.body.insertAdjacentHTML("beforeend", `<ytd-transcript-renderer></ytd-transcript-renderer>`);
      setTimeout(() => {
        doc.querySelector("ytd-transcript-renderer").insertAdjacentHTML("beforeend", `
          <ytd-transcript-segment-renderer>
            <span class="segment-timestamp">0:04</span>
            <yt-formatted-string class="segment-text">Lazy line</yt-formatted-string>
          </ytd-transcript-segment-renderer>`);
      }, 250);
    }
  });

  const result = await extractYouTubeResult(doc, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 1500,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.deepEqual(result.segments, [{ t: 4, text: "Lazy line" }]);
  assert.equal(clicks, 1, "should open the panel exactly once and then poll");
});

test("clicks actionable button ancestor for nested transcript label", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Nested label demo">
    <button id="outer"><span aria-label="Show transcript">Transcript</span></button>`, {
    url: "https://www.youtube.com/watch?v=nested123",
    pretendToBeVisual: true
  });
  let spanClicked = false;
  dom.window.document.querySelector("span").addEventListener("click", () => {
    spanClicked = true;
  });
  dom.window.document.getElementById("outer").addEventListener("click", () => {
    dom.window.document.body.insertAdjacentHTML("beforeend", `
      <ytd-transcript-renderer>
        <ytd-transcript-segment-renderer>
          <span class="segment-timestamp">0:11</span>
          <yt-formatted-string class="segment-text">Nested transcript</yt-formatted-string>
        </ytd-transcript-segment-renderer>
      </ytd-transcript-renderer>`);
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 700,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.equal(spanClicked, false);
  const click = events.find((event) => event.event === "transcript_button_click_completed");
  assert.equal(click.safe_metadata.nearest_actionable_ancestor_tag, "button");
  assert.equal(click.safe_metadata.click_method_used, "realistic_sequence");
  assert.equal(click.safe_metadata.panel_opened_after_click, "true");
});

test("clicks YouTube menu item control when transcript label is in renderer", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Menu item demo">
    <ytd-menu-service-item-renderer id="item" role="button">
      <span>Show transcript</span>
    </ytd-menu-service-item-renderer>`, {
    url: "https://www.youtube.com/watch?v=menuitem123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("item").addEventListener("click", () => {
    dom.window.document.body.insertAdjacentHTML("beforeend", `
      <ytd-transcript-renderer>
        <ytd-transcript-segment-renderer>
          <span class="segment-timestamp">0:13</span>
          <yt-formatted-string class="segment-text">Menu transcript</yt-formatted-string>
        </ytd-transcript-segment-renderer>
      </ytd-transcript-renderer>`);
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 700,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  const click = events.find((event) => event.event === "transcript_button_click_completed");
  assert.equal(click.safe_metadata.nearest_actionable_ancestor_tag, "ytd-menu-service-item-renderer");
  assert.equal(click.safe_metadata.nearest_actionable_ancestor_role, "button");
  assert.equal(click.safe_metadata.selector_strategy_used, "youtube_menu_item");
});

test("does not directly click inert transcript-labeled nodes", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Inert label demo">
    <span id="label" aria-label="Show transcript">Transcript</span>`, {
    url: "https://www.youtube.com/watch?v=inert123",
    pretendToBeVisual: true
  });
  let clicked = false;
  dom.window.document.getElementById("label").addEventListener("click", () => {
    clicked = true;
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 40,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(clicked, false);
  assert.equal(result.transcript_status, "unavailable");
  assert.ok(events.some((event) => event.event === "transcript_control_not_actionable"));
  const notActionable = events.find((event) => event.event === "transcript_control_not_actionable");
  assert.equal(notActionable.safe_metadata.candidate_tag_name, "span");
  assert.equal(notActionable.safe_metadata.nearest_actionable_ancestor_tag, "");
});

test("discovers transcript button behind expanded description", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Description demo">
    <button id="more">Show more</button>`, {
    url: "https://www.youtube.com/watch?v=description123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("more").addEventListener("click", () => {
    if (dom.window.document.getElementById("transcript")) return;
    dom.window.document.body.insertAdjacentHTML("beforeend", `<button id="transcript">Show transcript</button>`);
    dom.window.document.getElementById("transcript").addEventListener("click", () => {
      dom.window.document.body.insertAdjacentHTML("beforeend", `
        <ytd-transcript-renderer>
          <ytd-transcript-segment-renderer>
            <span class="segment-timestamp">0:09</span>
            <yt-formatted-string class="segment-text">Expanded transcript</yt-formatted-string>
          </ytd-transcript-segment-renderer>
        </ytd-transcript-renderer>`);
    });
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 900,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.deepEqual(result.segments, [{ t: 9, text: "Expanded transcript" }]);
  assert.ok(events.some((event) => event.event === "transcript_button_found"));
  assert.ok(events.some((event) => event.event === "transcript_extraction_succeeded"));
});

test("tries description expansion after transcript-labeled controls do not open panel", async () => {
  const events = [];
  const inertTranscriptButtons = Array.from({ length: 4 }, (_, index) => (
    `<button id="decoy-${index}">Transcript</button>`
  )).join("");
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Description fallback demo">
    ${inertTranscriptButtons}
    <button id="more">Show more</button>`, {
    url: "https://www.youtube.com/watch?v=descriptionfallback123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("more").addEventListener("click", () => {
    if (dom.window.document.getElementById("real-transcript")) return;
    dom.window.document.body.insertAdjacentHTML("beforeend", `<button id="real-transcript">Show transcript</button>`);
    dom.window.document.getElementById("real-transcript").addEventListener("click", () => {
      dom.window.document.body.insertAdjacentHTML("beforeend", `
        <ytd-transcript-renderer>
          <ytd-transcript-segment-renderer>
            <span class="segment-timestamp">0:21</span>
            <yt-formatted-string class="segment-text">Recovered through description</yt-formatted-string>
          </ytd-transcript-segment-renderer>
        </ytd-transcript-renderer>`);
    });
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 1200,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.deepEqual(result.segments, [{ t: 21, text: "Recovered through description" }]);
  assert.ok(events.some((event) => event.event === "transcript_button_click_completed"));
  assert.ok(events.some((event) => event.event === "transcript_description_expand_attempted"));
  assert.ok(events.some((event) => event.event === "transcript_extraction_succeeded"));
});

test("tries overflow menu after transcript-labeled controls do not open panel", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Menu fallback demo">
    <button id="decoy">Transcript</button>
    <button id="menu" aria-label="More actions">More actions</button>`, {
    url: "https://www.youtube.com/watch?v=menufallback123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("menu").addEventListener("click", () => {
    if (dom.window.document.getElementById("menu-transcript")) return;
    dom.window.document.body.insertAdjacentHTML("beforeend", `
      <ytd-menu-service-item-renderer id="menu-transcript" role="button">
        <span>Show transcript</span>
      </ytd-menu-service-item-renderer>`);
    dom.window.document.getElementById("menu-transcript").addEventListener("click", () => {
      dom.window.document.body.insertAdjacentHTML("beforeend", `
        <ytd-transcript-renderer>
          <ytd-transcript-segment-renderer>
            <span class="segment-timestamp">0:34</span>
            <yt-formatted-string class="segment-text">Recovered through menu</yt-formatted-string>
          </ytd-transcript-segment-renderer>
        </ytd-transcript-renderer>`);
    });
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 1200,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "full");
  assert.deepEqual(result.segments, [{ t: 34, text: "Recovered through menu" }]);
  assert.ok(events.some((event) => event.event === "transcript_menu_open_attempted"));
  assert.ok(events.some((event) => event.event === "transcript_extraction_succeeded"));
});

test("records precise reason when no transcript button is available", async () => {
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="No captions demo">
    <main>No private body should appear in diagnostics.</main>`, {
    url: "https://www.youtube.com/watch?v=nocaptions123",
    pretendToBeVisual: true
  });
  const events = [];

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 25,
    onDiagnostic: (event) => events.push(event)
  });

  assert.deepEqual(result.segments, []);
  assert.equal(result.transcript_status, "unavailable");
  assert.equal(result.transcript_reason, "transcript_button_not_found");
  assert.ok(events.some((event) => event.event === "transcript_button_not_found"));
  assert.equal(JSON.stringify(events).includes("private body"), false);
});

test("records precise reason when transcript button never opens panel", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Stuck panel demo">
    <button id="transcript">Show transcript</button>`, {
    url: "https://www.youtube.com/watch?v=stuck123",
    pretendToBeVisual: true
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 40,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "unavailable");
  assert.equal(result.transcript_reason, "transcript_panel_not_opened");
  assert.ok(events.some((event) => event.event === "transcript_button_found"));
  assert.ok(events.some((event) => event.event === "transcript_button_click_attempted"));
  assert.ok(events.some((event) => event.event === "transcript_panel_not_opened"));
});

test("records precise reason when transcript panel opens with empty rows", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Empty panel demo">
    <button id="transcript">Show transcript</button>`, {
    url: "https://www.youtube.com/watch?v=emptyrows123",
    pretendToBeVisual: true
  });
  dom.window.document.getElementById("transcript").addEventListener("click", () => {
    dom.window.document.body.insertAdjacentHTML("beforeend", `<ytd-transcript-renderer></ytd-transcript-renderer>`);
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 40,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.transcript_status, "unavailable");
  assert.equal(result.transcript_reason, "transcript_rows_empty");
  assert.ok(events.some((event) => event.event === "transcript_panel_opened"));
  assert.ok(events.some((event) => event.event === "transcript_rows_empty"));
});

test("emits redacted YouTube diagnostic events", async () => {
  const events = [];
  const dom = new JSDOM(`<title>Video</title>
    <meta property="og:title" content="Diagnostic demo">
    <button>More</button>`, {
    url: "https://www.youtube.com/watch?v=diagnostic123",
    pretendToBeVisual: true
  });

  const result = await extractYouTubeResult(dom.window.document, {
    discoverTranscript: true,
    transcriptDiscoveryTimeoutMs: 1,
    onDiagnostic: (event) => events.push(event)
  });

  assert.equal(result.ok, true);
  const eventNames = events.map((event) => event.event);
  assert.ok(eventNames.includes("youtube_extractor_started"));
  assert.ok(eventNames.includes("youtube_metadata_extracted"));
  assert.ok(eventNames.includes("youtube_transcript_discovery_started"));
  assert.ok(eventNames.includes("youtube_segments_extracted"));
  assert.equal(events.at(-1).safe_metadata.segment_count, "0");
  assert.equal(JSON.stringify(events).includes("Diagnostic demo"), false);
});

test("payload diagnostics map unsupported pages without private content", async () => {
  const events = [];
  const dom = new JSDOM(`<main>Private page body that must not appear.</main>`, {
    url: "https://example.com/not-readerable"
  });

  await assert.rejects(
    capturePayload(dom.window.document, {
      onDiagnostic: (event) => events.push(event)
    }),
    /does not look like/
  );

  assert.equal(events.length, 1);
  assert.equal(events[0].event, "payload_page_type_detected");
  assert.equal(events[0].status, "unsupported");
  assert.equal(JSON.stringify(events).includes("Private page body"), false);
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
