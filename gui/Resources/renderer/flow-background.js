// Starlee "Flow" background engine.
//
// A slow, organic navy/cream/black ribbon field — large domain-warped shapes
// quantized into three colour bands (black base, navy mass, thin cream edge).
// Three finishes:
//   - sharp: crisp posterized edges
//   - soft:  same field, softened (bilinear upscale + GPU blur)
//   - glass: soft field sliced into vertical glass slats with crisp separators
//
// It is seeded (a different seed reshapes the field) and drifts ever so subtly
// over time. Cheap by design: the field is computed at low resolution and
// scaled up; blur is offloaded to the compositor via CSS filter rather than a
// per-frame canvas filter.
(function () {
  const DEFAULTS = {
    pixelColor: "#062F64", // navy mass
    backgroundColor: "#F9E4B6", // cream edge
    speed: 0.02,
    zoom: 4.8,
    flowFinish: "soft",
    flowSeed: 0.42,
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
    ];
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
    let amp = 0.6;
    let norm = 0;
    let frequency = 1;
    for (let octave = 0; octave < 4; octave += 1) {
      sum += valueNoise(x * frequency, y * frequency) * amp;
      norm += amp;
      frequency *= 2.03;
      amp *= 0.5;
    }
    return sum / norm;
  }

  const FINISHES = ["sharp", "soft", "glass"];

  class FlowBackground {
    constructor(canvas, settings) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d", { alpha: false });
      this.buffer = document.createElement("canvas");
      this.bctx = this.buffer.getContext("2d");
      this.destroyed = false;
      this.start = performance.now();
      this.lastPaint = 0;
      this.applyParams(settings);
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(document.documentElement);
      this.resize();
      requestAnimationFrame((time) => this.frame(time));
    }

    applyParams(settings) {
      const s = settings || {};
      this.speed = clamp(s.speed ?? DEFAULTS.speed, 0.001, 0.08);
      this.zoom = clamp(s.zoom ?? DEFAULTS.zoom, 1, 10);
      const finish = s.flowFinish ?? s.finish ?? DEFAULTS.flowFinish;
      this.finish = FINISHES.includes(finish) ? finish : "soft";
      this.seed = Number(s.flowSeed ?? s.seed ?? DEFAULTS.flowSeed);
      if (!Number.isFinite(this.seed)) this.seed = DEFAULTS.flowSeed;
      this.seedX = (this.seed * 97.31) % 100;
      this.seedY = (this.seed * 57.13 + 11.7) % 100;
      this.navy = parseHex(s.pixelColor, parseHex(DEFAULTS.pixelColor, [6, 47, 100]));
      this.cream = parseHex(s.backgroundColor, parseHex(DEFAULTS.backgroundColor, [249, 228, 182]));
    }

    apply(settings) {
      this.applyParams(settings);
      // Flow always reads as a dark field; keep any sub-pixel gaps black.
      document.body.style.backgroundColor = "#000000";
      this.applyFinishStyles();
      this.resize();
    }

    applyFinishStyles() {
      // Blur via the compositor (cheap) for the soft finish; the glass finish
      // keeps its separators crisp, so it must not be CSS-blurred.
      this.canvas.style.imageRendering = this.finish === "sharp" ? "pixelated" : "auto";
      if (this.finish === "soft") {
        const radius = Math.max(6, Math.min(this.viewW || 1200, this.viewH || 800) * 0.012);
        this.canvas.style.filter = `blur(${radius.toFixed(1)}px)`;
      } else {
        this.canvas.style.filter = "none";
      }
    }

    resize() {
      const width = Math.max(1, window.innerWidth);
      const height = Math.max(1, window.innerHeight);
      const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
      this.viewW = width;
      this.viewH = height;
      // Low-res field buffer — the look is soft, so detail is wasted here.
      const bufferWidth = 240;
      const bufferHeight = Math.max(1, Math.round(bufferWidth * height / width));
      this.buffer.width = bufferWidth;
      this.buffer.height = bufferHeight;
      this.image = this.bctx.createImageData(bufferWidth, bufferHeight);
      this.canvas.width = Math.round(width * dpr);
      this.canvas.height = Math.round(height * dpr);
      this.canvas.style.width = "100vw";
      this.canvas.style.height = "100vh";
      this.applyFinishStyles();
      this.paint(performance.now() - this.start);
    }

    field(u, v, time) {
      const zoom = this.zoom * 0.5;
      const drift = time * this.speed * 0.00035;
      const x = (u * 1.62 + v * 0.22) * zoom + this.seedX + drift * 0.6;
      const y = (v * 1.18 - u * 0.12) * zoom + this.seedY - drift * 0.42;
      const warp = fbm(x * 0.5 + 5.2, y * 0.5 - 3.1) - 0.5;
      return fbm(x + warp * 1.85, y - warp * 1.4);
    }

    paint(time) {
      const data = this.image.data;
      const bw = this.buffer.width;
      const bh = this.buffer.height;
      const navy = this.navy;
      const cream = this.cream;
      let index = 0;
      for (let y = 0; y < bh; y += 1) {
        for (let x = 0; x < bw; x += 1) {
          const f = this.field(x / bw, y / bh, time);
          let r = 0;
          let g = 0;
          let b = 0;
          if (f < 0.5) {
            // black base
          } else if (f < 0.555) {
            r = cream[0];
            g = cream[1];
            b = cream[2];
          } else {
            r = navy[0];
            g = navy[1];
            b = navy[2];
          }
          data[index] = r;
          data[index + 1] = g;
          data[index + 2] = b;
          data[index + 3] = 255;
          index += 4;
        }
      }
      this.bctx.putImageData(this.image, 0, 0);
      this.composite();
    }

    composite() {
      const ctx = this.ctx;
      const w = this.canvas.width;
      const h = this.canvas.height;
      ctx.clearRect(0, 0, w, h);
      if (this.finish === "sharp") {
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(this.buffer, 0, 0, w, h);
        return;
      }
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      if (this.finish === "glass") {
        this.compositeGlass();
        return;
      }
      // soft: bilinear upscale; extra softness comes from the CSS blur.
      ctx.drawImage(this.buffer, 0, 0, w, h);
    }

    compositeGlass() {
      const ctx = this.ctx;
      const w = this.canvas.width;
      const h = this.canvas.height;
      const slats = 20;
      const slatWidth = w / slats;
      for (let i = 0; i < slats; i += 1) {
        const x = i * slatWidth;
        // Per-slat vertical refraction offset.
        const shift = (i % 2 === 0 ? 1 : -1) * h * 0.018 + Math.sin(i * 1.7 + this.seedX) * h * 0.01;
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, 0, slatWidth + 1, h);
        ctx.clip();
        ctx.drawImage(this.buffer, 0, shift, w, h);
        // Subtle glass sheen across each slat.
        const sheen = ctx.createLinearGradient(x, 0, x + slatWidth, 0);
        sheen.addColorStop(0, "rgba(255,255,255,0.06)");
        sheen.addColorStop(0.5, "rgba(255,255,255,0.0)");
        sheen.addColorStop(1, "rgba(0,0,0,0.10)");
        ctx.fillStyle = sheen;
        ctx.fillRect(x, 0, slatWidth + 1, h);
        ctx.restore();
        // Crisp separator between panes.
        ctx.fillStyle = "rgba(255,255,255,0.14)";
        ctx.fillRect(Math.round(x), 0, 1, h);
      }
    }

    frame(time) {
      if (this.destroyed) return;
      const frameGap = this.finish === "glass" ? 90 : 70;
      if (time - this.lastPaint > frameGap) {
        this.lastPaint = time;
        this.paint(time - this.start);
      }
      requestAnimationFrame((next) => this.frame(next));
    }

    destroy() {
      this.destroyed = true;
      if (this.resizeObserver) {
        this.resizeObserver.disconnect();
        this.resizeObserver = null;
      }
      this.canvas.style.filter = "none";
    }
  }

  window.createStarleeFlowBackground = function (canvas, settings) {
    return new FlowBackground(canvas, settings);
  };
  window.starleeFlowDefaultSettings = DEFAULTS;
})();
