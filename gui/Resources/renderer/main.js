const state = {
  captures: [],
  monthLabel: "Library",
  totalCount: 0,
  readiness: "",
  backgroundSettings: window.starleeDefaultPixelDitherSettings,
  editMode: false,
  filters: { type: "", author: "", topic: "", from: "", to: "" },
};

const elements = {
  search: document.querySelector("#search-input"),
  row: document.querySelector("#card-row"),
  empty: document.querySelector("#empty-state"),
  background: document.querySelector("#pixel-dither-background"),
  editToggle: document.querySelector("#edit-toggle"),
  uploadButton: document.querySelector("#upload-button"),
  filterToggle: document.querySelector("#filter-toggle"),
  filterPanel: document.querySelector("#filter-panel"),
  filterType: document.querySelector("#filter-type"),
  filterAuthor: document.querySelector("#filter-author"),
  filterTopic: document.querySelector("#filter-topic"),
  filterFrom: document.querySelector("#filter-from"),
  filterTo: document.querySelector("#filter-to"),
  filterClear: document.querySelector("#filter-clear"),
  filterCount: document.querySelector("#filter-count"),
  reader: document.querySelector("#reader"),
  readerTitle: document.querySelector("#reader-title"),
  readerMeta: document.querySelector("#reader-meta"),
  readerTopics: document.querySelector("#reader-topics"),
  readerActions: document.querySelector("#reader-actions"),
  readerBody: document.querySelector("#reader-body"),
};

const TYPE_LABELS = { article: "Article", youtube: "YouTube", note: "Note", spotify_episode: "Spotify", document: "Document" };

const pixelBackground = window.createStarleeBackground(
  elements.background,
  state.backgroundSettings
);

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

// Bridge to the native host (Swift WKScriptMessageHandler named "starlee").
function postToHost(message) {
  try {
    window.webkit?.messageHandlers?.starlee?.postMessage(message);
  } catch (_error) {
    /* not running inside the host webview */
  }
}

function matchesQuery(capture, query) {
  if (!query) return true;
  return [capture.title, capture.source, capture.type, capture.snippet, (capture.topics || []).join(" ")]
    .join(" ")
    .toLowerCase()
    .includes(query);
}

function dayOf(capture) {
  return String(capture.capturedAt || "").slice(0, 10); // YYYY-MM-DD (ISO sorts lexically)
}

function matchesFilters(capture) {
  const f = state.filters;
  if (f.type && capture.type !== f.type) return false;
  if (f.author && (capture.author || "") !== f.author) return false;
  if (f.topic && !(capture.topics || []).includes(f.topic)) return false;
  const day = dayOf(capture);
  if (f.from && (!day || day < f.from)) return false;
  if (f.to && (!day || day > f.to)) return false;
  return true;
}

function activeFilterCount() {
  const f = state.filters;
  return ["type", "author", "topic", "from", "to"].filter((key) => f[key]).length;
}

function fillSelect(select, values, currentValue) {
  if (!select) return;
  const placeholder = select.querySelector('option[value=""]');
  select.innerHTML = "";
  if (placeholder) select.appendChild(placeholder);
  values.forEach((value) => {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = TYPE_LABELS[value] || value;
    select.appendChild(option);
  });
  // Keep the current selection if it still exists, else reset.
  select.value = values.includes(currentValue) ? currentValue : "";
  if (select.value !== currentValue) {
    return false;
  }
  return true;
}

function populateFilterOptions() {
  const types = [...new Set(state.captures.map((c) => c.type).filter(Boolean))].sort();
  const authors = [...new Set(state.captures.map((c) => c.author).filter(Boolean))].sort();
  const topics = [...new Set(state.captures.flatMap((c) => c.topics || []).filter(Boolean))].sort();
  if (!fillSelect(elements.filterType, types, state.filters.type)) state.filters.type = "";
  if (!fillSelect(elements.filterAuthor, authors, state.filters.author)) state.filters.author = "";
  if (!fillSelect(elements.filterTopic, topics, state.filters.topic)) state.filters.topic = "";
}

function render() {
  const query = elements.search.value.trim().toLowerCase();
  const captures = state.captures.filter(
    (capture) => matchesQuery(capture, query) && matchesFilters(capture)
  );

  const activeCount = activeFilterCount();
  if (elements.filterToggle) {
    elements.filterToggle.classList.toggle("active", activeCount > 0);
    elements.filterToggle.textContent = activeCount > 0 ? `Filter (${activeCount})` : "Filter";
  }
  if (elements.filterCount) {
    elements.filterCount.textContent = `${captures.length} of ${state.captures.length}`;
  }

  elements.empty.hidden = captures.length > 0;
  elements.row.classList.toggle("editing", state.editMode);

  elements.row.innerHTML = captures
    .map((capture) => {
      const id = escapeHtml(capture.id);
      const title = escapeHtml(capture.title || "Untitled");
      const date = escapeHtml(capture.date || "Undated");
      const deleteButton = `
        <button class="card-delete" type="button" data-delete="${id}"
                aria-label="Delete ${title}" title="Delete permanently" tabindex="-1">−</button>`;

      return `
        <article class="capture-card" tabindex="0" role="button"
                 data-id="${id}" data-title="${title}"
                 aria-label="Open ${title}">
          ${deleteButton}
          <div class="card-copy">
            <h3>${title}</h3>
          </div>
          <time>${date}</time>
        </article>
      `;
    })
    .join("");
}

function openCapture(id) {
  if (!id) return;
  postToHost({ action: "open", id });
}

function requestDelete(id, title) {
  if (!id) return;
  // Confirmation is handled natively by the host (permanent action).
  postToHost({ action: "delete", id, title: title || "" });
}

function setEditMode(active) {
  state.editMode = Boolean(active);
  if (elements.editToggle) {
    elements.editToggle.setAttribute("aria-pressed", String(state.editMode));
    elements.editToggle.classList.toggle("active", state.editMode);
    elements.editToggle.textContent = state.editMode ? "Done" : "Edit";
  }
  render();
}

// --- Reader ---------------------------------------------------------------

function formatDay(value) {
  return String(value || "").slice(0, 10); // YYYY-MM-DD
}

function metaLine(record) {
  const parts = [];
  const isYouTube = record.type === "youtube";
  if (record.author) {
    // For YouTube the author field holds the channel name.
    parts.push(escapeHtml(isYouTube ? record.author : `by ${record.author}`));
  }
  if (!isYouTube && record.publishedAt) {
    parts.push(`Published ${escapeHtml(formatDay(record.publishedAt))}`);
  }
  if (record.date) parts.push(`Saved ${escapeHtml(record.date)}`);
  return parts.join(" · ");
}

function renderReaderActions(record) {
  const actions = [];
  if (record.url) {
    const label = record.type === "youtube" ? "Watch on YouTube" : "Go to Source";
    actions.push(
      `<button class="reader-action reader-action-primary" type="button" data-open-url="${escapeHtml(record.url)}">${label} ↗</button>`
    );
  }
  if (record.filePath) {
    actions.push(
      `<button class="reader-action" type="button" data-reveal="${escapeHtml(record.filePath)}">Reveal in Finder</button>`
    );
  }
  actions.push(
    `<button class="reader-action reader-action-danger" type="button" data-reader-delete="1">Delete</button>`
  );
  elements.readerActions.innerHTML = actions.join("");
}

// Mirror of the Rust topic sanitizer (src/topics.rs) so optimistic UI matches.
function sanitizeTopic(raw) {
  let out = "";
  for (const ch of String(raw ?? "")) {
    const code = ch.codePointAt(0);
    out += code < 32 || code === 127 ? " " : ch;
  }
  return out.replace(/\s+/g, " ").trim().slice(0, 64).trim();
}

function sanitizeTopics(list) {
  const seen = new Set();
  const out = [];
  (list || []).forEach((raw) => {
    const topic = sanitizeTopic(raw);
    if (topic && !seen.has(topic.toLowerCase())) {
      seen.add(topic.toLowerCase());
      out.push(topic);
    }
  });
  return out;
}

function renderReaderTopics(topics) {
  elements.readerTopics.hidden = false;
  const chips = (topics || [])
    .map(
      (topic) => `
        <span class="topic-chip topic-chip-editable">${escapeHtml(topic)}<button
          class="topic-remove" type="button" data-remove-topic="${escapeHtml(topic)}"
          aria-label="Remove topic ${escapeHtml(topic)}">×</button></span>`
    )
    .join("");
  elements.readerTopics.innerHTML = `${chips}<input class="topic-add" id="topic-add-input"
    type="text" placeholder="Add topic…" aria-label="Add a topic" maxlength="64">`;
}

function commitReaderTopics(topics) {
  if (!currentReaderRecord) return;
  const clean = sanitizeTopics(topics);
  currentReaderRecord.topics = clean;
  renderReaderTopics(clean);
  postToHost({ action: "setTopics", id: currentReaderRecord.id, topics: clean });
}

let currentReaderRecord = null;

window.renderStarleeReader = (record) => {
  if (!record || record.error) {
    currentReaderRecord = null;
    elements.readerTitle.textContent = "Could not open capture";
    elements.readerMeta.textContent = record?.error
      ? String(record.error)
      : "This record is no longer available.";
    elements.readerTopics.innerHTML = "";
    elements.readerTopics.hidden = true;
    elements.readerActions.innerHTML = "";
    elements.readerBody.textContent = "";
  } else {
    currentReaderRecord = record;
    elements.readerTitle.textContent = record.title || "Untitled";
    elements.readerMeta.textContent = metaLine(record);
    renderReaderTopics(record.topics);
    renderReaderActions(record);
    // The captured body/transcript is intentionally never shown here. The full
    // text lives in the vault for search and Codex; the reader is metadata-only
    // and sends the reader to the original source.
    const isYouTube = record.type === "youtube";
    elements.readerBody.textContent = record.url
      ? `The full ${isYouTube ? "transcript" : "text"} is saved privately in your vault for search and Codex. Open the source to ${isYouTube ? "watch the original" : "read the original"}.`
      : "The full text is saved privately in your vault for search and Codex.";
    elements.readerBody.classList.add("reader-body-note");
    elements.readerBody.scrollTop = 0;
  }
  elements.reader.hidden = false;
  elements.reader.classList.add("open");
  requestAnimationFrame(() => elements.readerBody.focus({ preventScroll: true }));
};

window.closeStarleeReader = () => {
  currentReaderRecord = null;
  elements.reader.classList.remove("open");
  elements.reader.hidden = true;
};

function applyBackgroundSettings(settings) {
  state.backgroundSettings = {
    ...state.backgroundSettings,
    ...(settings || {}),
  };
  pixelBackground.apply(state.backgroundSettings);
}

window.renderStarleeLibrary = (payload) => {
  state.captures = Array.isArray(payload?.captures) ? payload.captures : [];
  state.monthLabel = payload?.monthLabel || "Library";
  state.totalCount = Number(payload?.totalCount ?? state.captures.length);
  state.readiness = payload?.readiness || "Ready";
  applyBackgroundSettings(payload?.backgroundSettings);
  populateFilterOptions();
  render();
  if (payload?.showOnboarding) onboarding.maybeShow();
};

window.showStarleeOnboarding = () => onboarding.show();

window.applyStarleeBackgroundSettings = (settings) => {
  applyBackgroundSettings(settings);
};

// --- Events ---------------------------------------------------------------

elements.search.addEventListener("input", render);

if (elements.editToggle) {
  elements.editToggle.addEventListener("click", () => setEditMode(!state.editMode));
}

if (elements.uploadButton) {
  elements.uploadButton.addEventListener("click", () => postToHost({ action: "upload" }));
}

// --- Filters --------------------------------------------------------------

function setFilterPanelOpen(open) {
  if (!elements.filterPanel || !elements.filterToggle) return;
  elements.filterPanel.hidden = !open;
  elements.filterToggle.setAttribute("aria-expanded", String(open));
}

if (elements.filterToggle) {
  elements.filterToggle.addEventListener("click", (event) => {
    event.stopPropagation();
    setFilterPanelOpen(elements.filterPanel.hidden);
  });
}

function bindFilterControl(element, key) {
  if (!element) return;
  element.addEventListener("input", () => {
    state.filters[key] = element.value;
    render();
  });
}

bindFilterControl(elements.filterType, "type");
bindFilterControl(elements.filterAuthor, "author");
bindFilterControl(elements.filterTopic, "topic");
bindFilterControl(elements.filterFrom, "from");
bindFilterControl(elements.filterTo, "to");

if (elements.filterClear) {
  elements.filterClear.addEventListener("click", () => {
    state.filters = { type: "", author: "", topic: "", from: "", to: "" };
    [elements.filterType, elements.filterAuthor, elements.filterTopic, elements.filterFrom, elements.filterTo]
      .forEach((control) => { if (control) control.value = ""; });
    render();
  });
}

// Close the filter panel when clicking outside it.
document.addEventListener("click", (event) => {
  if (!elements.filterPanel || elements.filterPanel.hidden) return;
  if (event.target.closest(".filter-wrap")) return;
  setFilterPanelOpen(false);
});

// Delegated clicks on the card grid: delete button vs. open card.
elements.row.addEventListener("click", (event) => {
  const deleteButton = event.target.closest("[data-delete]");
  if (deleteButton) {
    event.preventDefault();
    event.stopPropagation();
    const card = deleteButton.closest(".capture-card");
    requestDelete(deleteButton.dataset.delete, card?.dataset.title);
    return;
  }
  if (state.editMode) return;
  const card = event.target.closest(".capture-card");
  if (card) openCapture(card.dataset.id);
});

// Keyboard: Enter/Space opens a focused card (when not editing).
elements.row.addEventListener("keydown", (event) => {
  if (state.editMode) return;
  if (event.key !== "Enter" && event.key !== " ") return;
  const card = event.target.closest(".capture-card");
  if (!card) return;
  event.preventDefault();
  openCapture(card.dataset.id);
});

// Reader dismissal (backdrop, ✕) and actions.
elements.reader.addEventListener("click", (event) => {
  if (event.target.closest("[data-reader-dismiss]")) {
    window.closeStarleeReader();
    return;
  }
  const openUrl = event.target.closest("[data-open-url]");
  if (openUrl) {
    postToHost({ action: "openURL", url: openUrl.dataset.openUrl });
    return;
  }
  const reveal = event.target.closest("[data-reveal]");
  if (reveal) {
    postToHost({ action: "reveal", path: reveal.dataset.reveal });
    return;
  }
  const removeTopic = event.target.closest("[data-remove-topic]");
  if (removeTopic && currentReaderRecord) {
    const next = (currentReaderRecord.topics || []).filter(
      (topic) => topic !== removeTopic.dataset.removeTopic
    );
    commitReaderTopics(next);
    return;
  }
  if (event.target.closest("[data-reader-delete]") && currentReaderRecord) {
    requestDelete(currentReaderRecord.id, currentReaderRecord.title);
  }
});

// Add a topic from the reader's "Add topic…" field.
elements.reader.addEventListener("keydown", (event) => {
  if (event.target.id !== "topic-add-input" || event.key !== "Enter") return;
  event.preventDefault();
  const value = event.target.value;
  event.target.value = "";
  if (!currentReaderRecord) return;
  commitReaderTopics([...(currentReaderRecord.topics || []), value]);
  requestAnimationFrame(() => document.querySelector("#topic-add-input")?.focus());
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !elements.reader.hidden) {
    window.closeStarleeReader();
  }
});

// --- Onboarding -----------------------------------------------------------

const onboarding = (() => {
  const root = document.querySelector("#onboarding");
  const steps = root ? Array.from(root.querySelectorAll(".onb-step")) : [];
  const dots = root ? Array.from(root.querySelectorAll("[data-step-dot]")) : [];
  const backButton = document.querySelector("#onb-back");
  const nextButton = document.querySelector("#onb-next");
  const skipButton = document.querySelector("#onb-skip");
  const browserNote = document.querySelector("#onb-browser-note");
  let step = 0;
  let suppressed = false; // don't auto-reopen after dismiss within a session

  function paint() {
    steps.forEach((section) => {
      section.hidden = Number(section.dataset.step) !== step;
    });
    dots.forEach((dot) => {
      dot.classList.toggle("active", Number(dot.dataset.stepDot) === step);
    });
    if (backButton) backButton.hidden = step === 0;
    if (nextButton) nextButton.textContent = step === steps.length - 1 ? "Get started" : "Next";
  }

  function show() {
    if (!root) return;
    step = 0;
    if (browserNote) browserNote.hidden = true;
    root.hidden = false;
    paint();
  }

  function maybeShow() {
    // Only open from a closed state — a Library reload re-invokes
    // renderStarleeLibrary, and without this guard it would reset the user's
    // progress back to step 1 mid-flow.
    if (!suppressed && root && root.hidden) show();
  }

  function finish() {
    if (!root) return;
    suppressed = true;
    root.hidden = true;
    postToHost({ action: "onboardingDone" });
  }

  if (root) {
    nextButton?.addEventListener("click", () => {
      if (step >= steps.length - 1) {
        finish();
      } else {
        step += 1;
        paint();
      }
    });
    backButton?.addEventListener("click", () => {
      step = Math.max(0, step - 1);
      paint();
    });
    skipButton?.addEventListener("click", finish);

    root.querySelectorAll("[data-browser]").forEach((button) => {
      button.addEventListener("click", () => {
        const browser = button.dataset.browser;
        if (browser === "chrome") {
          postToHost({ action: "openBrowserSetup" });
          if (browserNote) {
            browserNote.hidden = false;
            browserNote.textContent =
              "Opening Chrome setup — load the unpacked extension, then come back.";
          }
        }
      });
    });

    root.querySelector('[data-action="codex"]')?.addEventListener("click", () => {
      postToHost({ action: "codexGuide" });
    });
  }

  return { show, maybeShow, finish };
})();

if (window.__starleeLibraryPayload) {
  window.renderStarleeLibrary(window.__starleeLibraryPayload);
} else {
  render();
}
