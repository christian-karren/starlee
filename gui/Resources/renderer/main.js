const state = {
  captures: [],
  monthLabel: "Library",
  totalCount: 0,
  readiness: "",
  backgroundSettings: window.starleeDefaultDitherSettings,
  editMode: false,
  sortMode: "newest",
};

const elements = {
  search: document.querySelector("#search-input"),
  row: document.querySelector("#card-row"),
  noResults: document.querySelector("#no-results-state"),
  emptyLibrary: document.querySelector("#empty-library-state"),
  background: document.querySelector("#pixel-dither-background"),
  editToggle: document.querySelector("#edit-toggle"),
  uploadButton: document.querySelector("#upload-button"),
  settingsButton: document.querySelector("#settings-button"),
  sortToggle: document.querySelector("#sort-toggle"),
  sortPanel: document.querySelector("#sort-panel"),
  reader: document.querySelector("#reader"),
  readerTitle: document.querySelector("#reader-title"),
  readerMeta: document.querySelector("#reader-meta"),
  readerTopics: document.querySelector("#reader-topics"),
  readerActions: document.querySelector("#reader-actions"),
  readerBody: document.querySelector("#reader-body"),
};

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

function sortValue(value) {
  return String(value || "").trim().toLocaleLowerCase();
}

function timestampOf(capture) {
  const parsed = Date.parse(capture.capturedAt || "");
  return Number.isFinite(parsed) ? parsed : 0;
}

function sortedCaptures(captures) {
  const sorted = [...captures];
  switch (state.sortMode) {
  case "oldest":
    sorted.sort((a, b) => timestampOf(a) - timestampOf(b) || sortValue(a.title).localeCompare(sortValue(b.title)));
    break;
  case "title":
    sorted.sort((a, b) => sortValue(a.title).localeCompare(sortValue(b.title)) || timestampOf(b) - timestampOf(a));
    break;
  case "source":
    sorted.sort((a, b) => sortValue(a.source).localeCompare(sortValue(b.source)) || timestampOf(b) - timestampOf(a));
    break;
  case "newest":
  default:
    sorted.sort((a, b) => timestampOf(b) - timestampOf(a) || sortValue(a.title).localeCompare(sortValue(b.title)));
    break;
  }
  return sorted;
}

function render() {
  const query = elements.search.value.trim().toLowerCase();
  const captures = state.captures.filter((capture) => matchesQuery(capture, query));
  const visibleCaptures = sortedCaptures(captures);

  if (elements.sortToggle) {
    elements.sortToggle.classList.toggle("active", state.sortMode !== "newest");
    elements.sortToggle.title = `Sort the library (${sortLabel(state.sortMode)})`;
  }
  if (elements.sortPanel) {
    elements.sortPanel.querySelectorAll("[data-sort]").forEach((button) => {
      button.setAttribute("aria-pressed", String(button.dataset.sort === state.sortMode));
    });
  }
  const libraryEmpty = state.captures.length === 0;
  const noResults = !libraryEmpty && visibleCaptures.length === 0;
  if (elements.emptyLibrary) elements.emptyLibrary.hidden = !libraryEmpty;
  if (elements.noResults) elements.noResults.hidden = !noResults;
  elements.row.classList.toggle("editing", state.editMode);

  elements.row.innerHTML = visibleCaptures
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
    elements.editToggle.setAttribute("aria-label", state.editMode ? "Exit edit mode" : "Enter edit mode");
    elements.editToggle.title = state.editMode ? "Done editing" : "Edit mode: delete captures";
  }
  render();
}

function sortLabel(mode) {
  switch (mode) {
  case "oldest": return "oldest first";
  case "title": return "title A-Z";
  case "source": return "source A-Z";
  case "newest":
  default: return "newest first";
  }
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

function renderReaderTopics(topics) {
  elements.readerTopics.hidden = !(topics || []).length;
  const chips = (topics || [])
    .map(
      (topic) => `
        <span class="topic-chip">${escapeHtml(topic)}</span>`
    )
    .join("");
  elements.readerTopics.innerHTML = chips;
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

if (elements.settingsButton) {
  elements.settingsButton.addEventListener("click", () => postToHost({ action: "settings" }));
}

// --- Sort -----------------------------------------------------------------

function setSortPanelOpen(open) {
  if (!elements.sortPanel || !elements.sortToggle) return;
  elements.sortPanel.hidden = !open;
  elements.sortToggle.setAttribute("aria-expanded", String(open));
}

if (elements.sortToggle) {
  elements.sortToggle.addEventListener("click", (event) => {
    event.stopPropagation();
    setSortPanelOpen(elements.sortPanel.hidden);
  });
}

if (elements.sortPanel) {
  elements.sortPanel.addEventListener("click", (event) => {
    const option = event.target.closest("[data-sort]");
    if (!option) return;
    state.sortMode = option.dataset.sort || "newest";
    setSortPanelOpen(false);
    render();
  });
}

// Close popovers when clicking outside them.
document.addEventListener("click", (event) => {
  if (event.target.closest(".sort-wrap")) return;
  setSortPanelOpen(false);
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
  if (event.target.closest("[data-reader-delete]") && currentReaderRecord) {
    requestDelete(currentReaderRecord.id, currentReaderRecord.title);
  }
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
