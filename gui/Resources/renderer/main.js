const state = {
  captures: [],
  monthLabel: "Library",
  totalCount: 0,
  readiness: "",
  backgroundSettings: window.starleeDefaultPixelDitherSettings,
};

const elements = {
  status: document.querySelector("#status-pill"),
  search: document.querySelector("#search-input"),
  heading: document.querySelector("#month-heading"),
  count: document.querySelector("#result-count"),
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

function displayKind(type) {
  if (type === "youtube") return "YT";
  if (type === "spotify_episode") return "SP";
  if (type === "article") return "AR";
  if (type === "note") return "NT";
  return "ST";
}

function displayType(type) {
  if (type === "youtube") return "YouTube";
  if (type === "spotify_episode") return "Spotify";
  if (type === "article") return "Article";
  if (type === "note") return "Note";
  return String(type ?? "Capture").replace(/_/g, " ");
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
  const itemText = `${captures.length} item${captures.length === 1 ? "" : "s"}`;

  elements.status.textContent = state.readiness || "Ready";
  elements.heading.textContent = state.monthLabel;
  elements.count.textContent = itemText;
  elements.empty.hidden = captures.length > 0;

  elements.row.innerHTML = captures.map((capture) => {
    const title = escapeHtml(capture.title || "Untitled");
    const source = escapeHtml(capture.source || displayType(capture.type));
    const date = escapeHtml(capture.date || "Undated");
    const kind = escapeHtml(displayKind(capture.type));
    const type = escapeHtml(displayType(capture.type));

    return `
      <article class="capture-card" tabindex="0" data-id="${escapeHtml(capture.id)}">
        <div class="card-icon" aria-hidden="true">${kind}</div>
        <div class="card-copy">
          <p class="card-kicker">${type} · ${source}</p>
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
