const state = {
  captures: [],
  monthLabel: "Library",
  totalCount: 0,
  readiness: "",
  backgroundSettings: window.starleeDefaultPixelDitherSettings,
  editMode: false,
};

const elements = {
  search: document.querySelector("#search-input"),
  row: document.querySelector("#card-row"),
  empty: document.querySelector("#empty-state"),
  background: document.querySelector("#pixel-dither-background"),
  editToggle: document.querySelector("#edit-toggle"),
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
  return [capture.title, capture.source, capture.type, capture.snippet]
    .join(" ")
    .toLowerCase()
    .includes(query);
}

function render() {
  const query = elements.search.value.trim().toLowerCase();
  const captures = state.captures.filter((capture) => matchesQuery(capture, query));

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

function metaLine(record) {
  const parts = [];
  if (record.type) parts.push(escapeHtml(record.type));
  if (record.source) parts.push(escapeHtml(record.source));
  if (record.author) parts.push(`by ${escapeHtml(record.author)}`);
  if (record.date) parts.push(escapeHtml(record.date));
  if (typeof record.wordCount === "number" && record.wordCount > 0) {
    parts.push(`${record.wordCount.toLocaleString()} words`);
  }
  if (record.transcriptStatus) parts.push(escapeHtml(record.transcriptStatus));
  return parts.join(" · ");
}

function renderReaderActions(record) {
  const actions = [];
  if (record.url) {
    actions.push(
      `<button class="reader-action" type="button" data-open-url="${escapeHtml(record.url)}">Open original</button>`
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
  if (!Array.isArray(topics) || topics.length === 0) {
    elements.readerTopics.innerHTML = "";
    elements.readerTopics.hidden = true;
    return;
  }
  elements.readerTopics.hidden = false;
  elements.readerTopics.innerHTML = topics
    .map((topic) => `<span class="topic-chip">${escapeHtml(topic)}</span>`)
    .join("");
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
    // body is plain text/markdown; render as preformatted text to avoid injection.
    elements.readerBody.textContent = record.body || "(No saved text for this capture.)";
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
};

window.applyStarleeBackgroundSettings = (settings) => {
  applyBackgroundSettings(settings);
};

// --- Events ---------------------------------------------------------------

elements.search.addEventListener("input", render);

if (elements.editToggle) {
  elements.editToggle.addEventListener("click", () => setEditMode(!state.editMode));
}

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

if (window.__starleeLibraryPayload) {
  window.renderStarleeLibrary(window.__starleeLibraryPayload);
} else {
  render();
}
