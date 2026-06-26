// Starlee Settings renderer.
//
// Renders the Settings page inside the same WebView/CSS layer as the Library so
// the header and card surface match exactly. Owns the background-suite control
// panel (6 looks + per-engine controls), drives the live background instantly,
// and posts changes/actions back to the native app.

(function () {
  const DEFAULTS = {
    kind: "pixel-dither",
    pixelColor: "#062F64",
    backgroundColor: "#F9E4B6",
    black: "#000000",
    white: "#FFFFFF",
    pixelSize: 6,
    threshold: 0.31,
    speed: 0.02,
    zoom: 4.8,
    flowFinish: "soft",
    flowSeed: 0.42,
    auroraIntensity: 0.55,
    ditherDotSize: 6,
    ditherContrast: 1.3,
    ditherNavyBuffer: 1.4,
    glassMode: "panes",
    glassPanes: 18,
    glassSoftness: 14,
    glassBrightness: 1.0,
    glassRefraction: 0.02,
  };

  const NAVY = "#13284B";
  const CREAM = "#F2E3B6";

  const PRESETS = [
    { name: "Navy Cream ·175", s: { kind: "pixel-dither", pixelColor: "#062F64", backgroundColor: "#F9E4B6", threshold: 0.175, speed: 0.02, zoom: 4.8 } },
    { name: "Navy Cream ·366", s: { kind: "pixel-dither", pixelColor: "#062F64", backgroundColor: "#F9E4B6", threshold: 0.366, speed: 0.02, zoom: 4.8 } },
    { name: "Ribbon", s: { kind: "flow", pixelColor: "#102A57", backgroundColor: "#F2E0AE", threshold: 0.31, speed: 0.018, zoom: 4.6, flowFinish: "sharp", flowSeed: 0.42 } },
    { name: "Aurora", s: { kind: "aurora", pixelColor: NAVY, backgroundColor: CREAM, speed: 0.7, zoom: 4.8, auroraIntensity: 0.55 } },
    { name: "Dither", s: { kind: "dither", pixelColor: NAVY, backgroundColor: CREAM, speed: 0, ditherDotSize: 3, ditherContrast: 1.3, ditherNavyBuffer: 1.4 } },
    { name: "Glass", s: { kind: "glass", pixelColor: NAVY, backgroundColor: CREAM, speed: 0.004, glassMode: "panes", glassPanes: 18, glassSoftness: 14, glassBrightness: 1.0, glassRefraction: 0.02 } },
  ];

  let state = Object.assign({}, DEFAULTS);
  let sections = [];

  const canvas = document.getElementById("pixel-dither-background");
  const background = window.createStarleeBackground(canvas, state);
  const content = document.getElementById("settings-content");

  function postBackground() {
    try { window.webkit.messageHandlers.starlee.postMessage({ action: "setBackground", settings: state }); } catch (e) {}
  }
  function postAction(action) {
    try { window.webkit.messageHandlers.starlee.postMessage({ action: action }); } catch (e) {}
  }
  function applyLive() { background.apply(state); }

  function h(tag, attrs, kids) {
    const node = document.createElement(tag);
    if (attrs) {
      for (const k in attrs) {
        if (k === "class") node.className = attrs[k];
        else if (k === "text") node.textContent = attrs[k];
        else if (k.slice(0, 2) === "on") node.addEventListener(k.slice(2).toLowerCase(), attrs[k]);
        else node.setAttribute(k, attrs[k]);
      }
    }
    (kids || []).forEach((c) => { if (c) node.appendChild(typeof c === "string" ? document.createTextNode(c) : c); });
    return node;
  }

  function fmt(value, dec) {
    if (dec === 0) return String(Math.round(value));
    return Number(value).toFixed(dec);
  }

  function sliderRow(label, key, min, max, step, dec, transform) {
    const out = h("span", { class: "val", text: fmt(state[key], dec) });
    const input = h("input", {
      type: "range", min: min, max: max, step: step, value: state[key],
      oninput: () => {
        let v = parseFloat(input.value);
        if (transform) v = transform(v);
        state[key] = v;
        out.textContent = fmt(v, dec);
        applyLive();
        postBackground();
      },
    });
    return h("div", { class: "row" }, [h("label", { text: label }), input, out]);
  }

  function swatch(label, key) {
    const input = h("input", {
      type: "color", value: state[key],
      oninput: () => { state[key] = input.value.toUpperCase(); applyLive(); postBackground(); },
    });
    return h("span", { class: "swatch" }, [input, document.createTextNode(label)]);
  }

  function paletteRow(pairs) {
    return h("div", { class: "palette" }, pairs.map((p) => swatch(p[0], p[1])));
  }

  function segmented(options, current, onPick) {
    const wrap = h("div", { class: "seg" });
    options.forEach((opt) => {
      const b = h("button", { type: "button", text: opt.label, "aria-pressed": String(opt.value === current),
        onclick: () => onPick(opt.value) });
      wrap.appendChild(b);
    });
    return wrap;
  }

  function randomizeBtn() {
    return h("button", { class: "btn", type: "button", text: "Randomize",
      onclick: () => { state.flowSeed = Math.random(); applyLive(); postBackground(); } });
  }

  function controlsFor(kind) {
    const c = h("div", { class: "controls" });
    if (kind === "pixel-dither") {
      c.appendChild(sliderRow("Pixel size", "pixelSize", 1, 12, 1, 0, Math.round));
      c.appendChild(sliderRow("Threshold", "threshold", 0.12, 0.55, 0.005, 3));
      c.appendChild(sliderRow("Speed", "speed", 0.005, 0.08, 0.001, 3));
      c.appendChild(sliderRow("Zoom", "zoom", 2, 7, 0.1, 1));
    } else if (kind === "flow") {
      c.appendChild(sliderRow("Speed", "speed", 0.005, 0.08, 0.001, 3));
    } else if (kind === "aurora") {
      c.appendChild(sliderRow("Intensity", "auroraIntensity", 0.2, 0.85, 0.01, 2));
      c.appendChild(sliderRow("Speed", "speed", 0, 1.6, 0.05, 2));
    } else if (kind === "dither") {
      c.appendChild(sliderRow("Dot size", "ditherDotSize", 3, 12, 1, 0, Math.round));
      c.appendChild(sliderRow("Contrast", "ditherContrast", 0.7, 2.2, 0.05, 2));
      c.appendChild(sliderRow("Navy buffer", "ditherNavyBuffer", 0.5, 2.5, 0.05, 2));
      c.appendChild(sliderRow("Speed", "speed", 0, 0.01, 0.0005, 4));
    } else if (kind === "glass") {
      c.appendChild(sliderRow("Speed", "speed", 0, 0.01, 0.0005, 4));
      c.appendChild(sliderRow("Softness", "glassSoftness", 0, 40, 1, 0, Math.round));
      c.appendChild(sliderRow("Brightness", "glassBrightness", 0.6, 1.6, 0.05, 2));
      if (state.glassMode !== "blur") {
        c.appendChild(sliderRow("Panes", "glassPanes", 8, 32, 1, 0, Math.round));
        c.appendChild(sliderRow("Refraction", "glassRefraction", 0, 0.05, 0.002, 3));
      }
    }
    return c;
  }

  function engineExtras(kind) {
    // Color + mode/finish controls that sit above the sliders.
    const block = h("div", { class: "controls-block" });
    if (kind === "pixel-dither") {
      block.appendChild(paletteRow([["Pixel", "pixelColor"], ["Background", "backgroundColor"]]));
    } else if (kind === "flow") {
      block.appendChild(paletteRow([["Navy", "pixelColor"], ["Cream", "backgroundColor"]]));
      const finish = segmented(
        [{ label: "Sharp", value: "sharp" }, { label: "Soft", value: "soft" }, { label: "Glass", value: "glass" }],
        state.flowFinish,
        (v) => { state.flowFinish = v; applyLive(); postBackground(); renderBgControls(); }
      );
      block.appendChild(h("div", { class: "row", style: "margin-top:12px" }, [h("label", { text: "Finish" }), finish]));
    } else {
      block.appendChild(paletteRow([["Navy", "pixelColor"], ["Cream", "backgroundColor"], ["Black", "black"], ["White", "white"]]));
      if (kind === "glass") {
        const mode = segmented(
          [{ label: "Panes", value: "panes" }, { label: "Blur only", value: "blur" }],
          state.glassMode,
          (v) => { state.glassMode = v; applyLive(); postBackground(); renderBgControls(); }
        );
        block.appendChild(h("div", { class: "row", style: "margin-top:12px" }, [h("label", { text: "Mode" }), mode]));
      }
    }
    return block;
  }

  function lookMatches(preset) {
    if (preset.s.kind !== state.kind) return false;
    if (preset.s.kind === "pixel-dither") {
      return Math.abs((state.threshold || 0) - preset.s.threshold) < 0.001
        && state.pixelColor.toUpperCase() === preset.s.pixelColor.toUpperCase();
    }
    return true;
  }

  function renderTiles() {
    const tiles = h("div", { class: "tiles" });
    PRESETS.forEach((preset) => {
      tiles.appendChild(h("button", {
        class: "tile", type: "button", text: preset.name, "aria-pressed": String(lookMatches(preset)),
        onclick: () => {
          state = Object.assign({}, DEFAULTS, preset.s);
          if (state.kind !== "pixel-dither") state.flowSeed = Math.random();
          applyLive();
          postBackground();
          renderBackgroundPanel();
        },
      }));
    });
    return tiles;
  }

  let bgControlsHost = null;
  function renderBgControls() {
    if (!bgControlsHost) return;
    bgControlsHost.textContent = "";
    bgControlsHost.appendChild(engineExtras(state.kind));
    bgControlsHost.appendChild(controlsFor(state.kind));
    bgControlsHost.appendChild(h("div", { class: "btn-row" }, [randomizeBtn()]));
  }

  let bgPanel = null;
  function renderBackgroundPanel() {
    if (!bgPanel) return;
    bgPanel.textContent = "";
    bgPanel.appendChild(h("div", { class: "panel-head" }, [
      h("div", {}, [h("div", { class: "panel-title", text: "Background" }),
        h("div", { class: "panel-sub", text: "Pick a look — changes apply and save instantly." })]),
    ]));
    bgPanel.appendChild(h("div", { class: "caption", text: "Style" }));
    bgPanel.appendChild(renderTiles());
    // Fine-tuning knobs are tucked under a disclosure so the presets lead.
    const advanced = h("details", { class: "advanced" });
    advanced.appendChild(h("summary", { text: "Advanced controls" }));
    bgControlsHost = h("div", {});
    advanced.appendChild(bgControlsHost);
    bgPanel.appendChild(advanced);
    renderBgControls();
  }

  function utilCard(section) {
    const kids = [
      h("div", { class: "util-title", text: section.title }),
      h("div", { class: "chip " + (section.ok ? "ok" : "warn"), text: section.status }),
    ];
    if (section.detail) kids.push(h("div", { class: "util-detail", text: section.detail }));
    if (section.action) {
      kids.push(h("div", { class: "util-action" }, [
        h("button", { class: "btn", type: "button", text: section.actionLabel || "Open", onclick: () => postAction(section.action) }),
      ]));
    }
    return h("div", { class: "util-card" }, kids);
  }

  function render() {
    content.textContent = "";
    bgPanel = h("div", { class: "panel" });
    content.appendChild(bgPanel);
    renderBackgroundPanel();

    if (sections.length) {
      content.appendChild(h("div", { class: "caption", text: "System" }));
      const grid = h("div", { class: "card-grid" });
      sections.forEach((s) => grid.appendChild(utilCard(s)));
      content.appendChild(grid);
    }
  }

  window.renderStarleeSettings = function (payload) {
    if (payload && payload.background) state = Object.assign({}, DEFAULTS, payload.background);
    if (payload && Array.isArray(payload.sections)) sections = payload.sections;
    applyLive();
    render();
  };

  window.applyStarleeBackgroundSettings = function (s) {
    state = Object.assign({}, state, s || {});
    background.apply(state);
  };

  if (window.__starleeSettingsPayload) {
    window.renderStarleeSettings(window.__starleeSettingsPayload);
  } else {
    render();
  }
})();
