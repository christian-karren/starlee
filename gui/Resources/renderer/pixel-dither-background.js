(function () {
  const DEFAULT_SETTINGS = {
    pixelColor: "#062F64",
    backgroundColor: "#F9E4B6",
    pixelSize: 6,
    threshold: 0.31,
    speed: 0.02,
    zoom: 4.8,
  };

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, Number(value)));
  }

  function parseHex(hex, fallback) {
    const clean = String(hex || "").trim().replace("#", "");
    if (!/^[0-9a-fA-F]{6}$/.test(clean)) return fallback;
    return [
      parseInt(clean.slice(0, 2), 16),
      parseInt(clean.slice(2, 4), 16),
      parseInt(clean.slice(4, 6), 16),
      255,
    ];
  }

  function normalizeSettings(settings) {
    const next = { ...DEFAULT_SETTINGS, ...(settings || {}) };
    return {
      pixelColor: String(next.pixelColor || DEFAULT_SETTINGS.pixelColor),
      backgroundColor: String(next.backgroundColor || DEFAULT_SETTINGS.backgroundColor),
      pixelSize: Math.round(clamp(next.pixelSize, 2, 16)),
      threshold: clamp(next.threshold, 0.1, 0.6),
      speed: clamp(next.speed, 0.001, 0.08),
      zoom: clamp(next.zoom, 1, 10),
    };
  }

  function hash2(x, y) {
    const value = Math.sin(x * 127.1 + y * 311.7) * 43758.5453123;
    return value - Math.floor(value);
  }

  function smooth(t) {
    return t * t * (3 - 2 * t);
  }

  function valueNoise(x, y) {
    const xi = Math.floor(x);
    const yi = Math.floor(y);
    const xf = x - xi;
    const yf = y - yi;
    const u = smooth(xf);
    const v = smooth(yf);
    const a = hash2(xi, yi);
    const b = hash2(xi + 1, yi);
    const c = hash2(xi, yi + 1);
    const d = hash2(xi + 1, yi + 1);
    const x1 = a + (b - a) * u;
    const x2 = c + (d - c) * u;
    return x1 + (x2 - x1) * v;
  }

  function fbm(x, y) {
    let sum = 0;
    let amp = 0.55;
    let norm = 0;
    let frequency = 1;
    for (let octave = 0; octave < 4; octave += 1) {
      sum += valueNoise(x * frequency, y * frequency) * amp;
      norm += amp;
      frequency *= 2.08;
      amp *= 0.5;
    }
    return sum / norm;
  }

  class PixelDitherBackground {
    constructor(canvas, initialSettings) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d", { alpha: false });
      this.settings = normalizeSettings(initialSettings);
      this.pixel = parseHex(this.settings.pixelColor, [6, 47, 100, 255]);
      this.paper = parseHex(this.settings.backgroundColor, [249, 228, 182, 255]);
      this.lastPaint = 0;
      this.start = performance.now();
      this.destroyed = false;
      this.resizeObserver = new ResizeObserver(() => this.resize(true));
      this.resizeObserver.observe(document.documentElement);
      this.canvas.style.filter = "none";
      this.canvas.style.imageRendering = "pixelated";
      this.resize(true);
      requestAnimationFrame((time) => this.frame(time));
    }

    apply(settings) {
      this.settings = normalizeSettings(settings);
      this.pixel = parseHex(this.settings.pixelColor, this.pixel);
      this.paper = parseHex(this.settings.backgroundColor, this.paper);
      document.body.style.backgroundColor = this.settings.backgroundColor;
      this.resize(true);
    }

    destroy() {
      this.destroyed = true;
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
        this.resizeObserver = null;
      }
    }

    resize(force) {
      const width = Math.max(1, window.innerWidth);
      const height = Math.max(1, window.innerHeight);
      const pixelSize = this.settings.pixelSize;
      const cols = Math.ceil(width / pixelSize);
      const rows = Math.ceil(height / pixelSize);
      if (!force && this.cols === cols && this.rows === rows) return;
      this.cols = cols;
      this.rows = rows;
      this.canvas.width = cols;
      this.canvas.height = rows;
      this.canvas.style.width = "100vw";
      this.canvas.style.height = "100vh";
      this.image = this.ctx.createImageData(cols, rows);
      this.paint(performance.now());
    }

    fieldAt(x, y, time) {
      const zoom = this.settings.zoom * 0.58;
      const drift = time * this.settings.speed * 0.00045;
      const grain = (x - y) * 0.018;
      const px = (x * 1.15 + y * 0.16) / zoom + drift * 0.7;
      const py = (y * 0.72 - x * 0.05) / zoom - drift * 0.42;
      const warp = fbm(px * 0.34 + 19.2, py * 0.34 - 8.4) - 0.5;
      const base = fbm(px + warp * 1.65, py - warp * 1.25 + grain);
      const veins = fbm(px * 2.3 - drift * 0.18, py * 0.85 + drift * 0.22);
      return base * 0.74 + veins * 0.26;
    }

    paint(time) {
      const data = this.image.data;
      const cutoff = this.settings.threshold + 0.28;
      let index = 0;
      for (let y = 0; y < this.rows; y += 1) {
        for (let x = 0; x < this.cols; x += 1) {
          const edgeBias = valueNoise(x * 0.045 + 21.7, y * 0.045 - 4.1) * 0.08;
          const on = this.fieldAt(x, y, time) + edgeBias > cutoff;
          const color = on ? this.pixel : this.paper;
          data[index] = color[0];
          data[index + 1] = color[1];
          data[index + 2] = color[2];
          data[index + 3] = 255;
          index += 4;
        }
      }
      this.ctx.putImageData(this.image, 0, 0);
    }

    frame(time) {
      if (this.destroyed) return;
      const frameGap = 120;
      if (time - this.lastPaint > frameGap) {
        this.lastPaint = time;
        this.paint(time - this.start);
      }
      requestAnimationFrame((next) => this.frame(next));
    }
  }

  window.createStarleePixelDitherBackground = function (canvas, settings) {
    return new PixelDitherBackground(canvas, settings);
  };
  window.starleeDefaultPixelDitherSettings = DEFAULT_SETTINGS;
})();
