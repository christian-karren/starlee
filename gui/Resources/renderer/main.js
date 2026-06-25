const state = {
  captures: [],
  monthLabel: "Library",
  totalCount: 0,
  readiness: "",
  backgroundSettings: window.starleeDefaultPixelDitherSettings,
};

const elements = {
  search: document.querySelector("#search-input"),
  row: document.querySelector("#card-row"),
  empty: document.querySelector("#empty-state"),
  background: document.querySelector("#pixel-dither-background"),
};

const pixelBackground = window.createStarleePixelDitherBackground(
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

function matchesQuery(capture, query) {
  if (!query) return true;
  return [
    capture.title,
    capture.source,
    capture.type,
    capture.snippet,
  ].join(" ").toLowerCase().includes(query);
}

function render() {
  const query = elements.search.value.trim().toLowerCase();
  const captures = state.captures.filter((capture) => matchesQuery(capture, query));

  elements.empty.hidden = captures.length > 0;

  elements.row.innerHTML = captures.map((capture) => {
    const title = escapeHtml(capture.title || "Untitled");
    const date = escapeHtml(capture.date || "Undated");

    return `
      <article class="capture-card" tabindex="0" data-id="${escapeHtml(capture.id)}">
        <div class="card-copy">
          <h3>${title}</h3>
        </div>
        <time>${date}</time>
      </article>
    `;
  }).join("");
}

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

elements.search.addEventListener("input", render);

if (window.__starleeLibraryPayload) {
  window.renderStarleeLibrary(window.__starleeLibraryPayload);
} else {
  render();
}
