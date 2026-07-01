// Starlee "Dither" background engine.
//
// An animated halftone: a domain-warped field rendered as a grid of square dots
// ordered-dithered (Bayer 4x4) between four brightness-ordered tones
// (black -> navy -> cream -> white). Only ADJACENT tones dither together, so
// black never borders cream directly — a navy zone always separates them, and
// the "navy buffer" widens that zone. Motion is a very slow drift.
(function () {
  window.starleeDefaultDitherSettings = {
    kind: "dither",
    pixelColor: "#13284B",
    backgroundColor: "#F2E3B6",
    black: "#000000",
    white: "#FFFFFF",
    speed: 0,
    flowSeed: 0.5,
    ditherDotSize: 3,
    ditherContrast: 1.3,
    ditherNavyBuffer: 1.4
  };

  function hex2bytes(h) {
    var n = parseInt(String(h || "").replace("#", ""), 16);
    if (isNaN(n)) return [0, 0, 0];
    return [n >> 16 & 255, n >> 8 & 255, n & 255];
  }
  function clamp(v, a, b) { return Math.min(b, Math.max(a, v)); }
  function h2(x, y) { var v = Math.sin(x * 127.1 + y * 311.7) * 43758.5453123; return v - Math.floor(v); }
  function sm(t) { return t * t * (3 - 2 * t); }
  function vn(x, y) {
    var xi = Math.floor(x), yi = Math.floor(y), xf = x - xi, yf = y - yi, u = sm(xf), v = sm(yf);
    var a = h2(xi, yi), b = h2(xi + 1, yi), c = h2(xi, yi + 1), d = h2(xi + 1, yi + 1);
    var x1 = a + (b - a) * u, x2 = c + (d - c) * u; return x1 + (x2 - x1) * v;
  }
  function fbm(x, y) {
    var s = 0, amp = 0.6, n = 0, f = 1;
    for (var o = 0; o < 4; o++) { s += vn(x * f, y * f) * amp; n += amp; f *= 2.03; amp *= 0.5; }
    return s / n;
  }
  var BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

  class DitherBackground {
    constructor(canvas, settings) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d", { alpha: false });
      this.destroyed = false;
      this.start = performance.now();
      this.lastPaint = 0;
      this.applyParams(settings);
      canvas.style.imageRendering = "auto";
      canvas.style.filter = "none";
      document.body.style.backgroundColor = "#000000";
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(document.documentElement);
      this.resize();
      requestAnimationFrame((t) => this.frame(t));
    }

    applyParams(s) {
      s = s || {};
      this.tones = [
        hex2bytes(s.black || "#000000"),
        hex2bytes(s.pixelColor || "#13284B"),
        hex2bytes(s.backgroundColor || "#F2E3B6"),
        hex2bytes(s.white || "#FFFFFF")
      ];
      this.dot = clamp(Number(s.ditherDotSize != null ? s.ditherDotSize : 6), 2, 16);
      this.contrast = clamp(Number(s.ditherContrast != null ? s.ditherContrast : 1.3), 0.5, 2.5);
      this.navyBuffer = clamp(Number(s.ditherNavyBuffer != null ? s.ditherNavyBuffer : 1.4), 0.5, 2.5);
      this.speed = clamp(Number(s.speed != null ? s.speed : 0.005), 0, 0.01);
      var seed = Number(s.flowSeed != null ? s.flowSeed : 0.5);
      seed = isNaN(seed) ? 0.5 : seed;
      this.seedX = (seed * 97.31) % 100;
      this.seedY = (seed * 57.13 + 11.7) % 100;
    }

    apply(settings) {
      this.applyParams(settings);
      document.body.style.backgroundColor = "#000000";
    }

    field(u, v, drift) {
      var z = 2.2;
      var x = (u * 1.7 + v * 0.2) * z + this.seedX + drift * 0.6;
      var y = (v * 1.2 - u * 0.1) * z + this.seedY - drift * 0.42;
      var w = fbm(x * 0.5 + 5.2, y * 0.5 - 3.1) - 0.5;
      return fbm(x + w * 1.85, y - w * 1.4);
    }

    resize() {
      var dpr = Math.min(window.devicePixelRatio || 1, 1.75);
      this.canvas.width = Math.max(1, Math.floor(window.innerWidth * dpr));
      this.canvas.height = Math.max(1, Math.floor(window.innerHeight * dpr));
      this.canvas.style.width = "100vw";
      this.canvas.style.height = "100vh";
      this.dpr = dpr;
      this.paint(performance.now() - this.start);
    }

    paint(time) {
      var ctx = this.ctx;
      var w = this.canvas.width, h = this.canvas.height;
      var cell = Math.max(2, Math.round(this.dot * (this.dpr || 1)));
      var tones = this.tones, nb = this.navyBuffer, contrast = this.contrast;
      var drift = time * this.speed * 0.015;
      for (var cy = 0, ry = 0; cy < h; cy += cell, ry++) {
        for (var cx = 0, rx = 0; cx < w; cx += cell, rx++) {
          var f = this.field((cx + cell * 0.5) / w, (cy + cell * 0.5) / h, drift);
          f = (f - 0.5) * contrast + 0.5; if (f < 0) f = 0; if (f > 1) f = 1;
          var L;
          if (f < 0.5) { L = Math.pow(f / 0.5, 1 / nb); } else { L = 1 + (f - 0.5) / 0.5 * 2; }
          if (L > 3) L = 3; if (L < 0) L = 0;
          var lo = Math.floor(L); if (lo > 2) lo = 2; var hi = lo + 1; var frac = L - lo;
          var c = frac > (BAYER[ry & 3][rx & 3] + 0.5) / 16 ? tones[hi] : tones[lo];
          ctx.fillStyle = "rgb(" + c[0] + "," + c[1] + "," + c[2] + ")";
          ctx.fillRect(cx, cy, cell, cell);
        }
      }
    }

    frame(time) {
      if (this.destroyed) return;
      if (time - this.lastPaint > 70) { this.lastPaint = time; this.paint(time - this.start); }
      requestAnimationFrame((next) => this.frame(next));
    }

    destroy() {
      this.destroyed = true;
      if (this.resizeObserver) { this.resizeObserver.disconnect(); this.resizeObserver = null; }
    }
  }

  window.createStarleeDitherBackground = function (canvas, settings) {
    return new DitherBackground(canvas, settings);
  };
})();
