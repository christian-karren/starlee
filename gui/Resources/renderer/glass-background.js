// Starlee "Glass" background engine.
//
// A domain-warped field smoothly mapped across black -> navy -> cream -> white,
// then either sliced into vertical glass panes (crisp separators + per-pane
// refraction offset) or shown as a single soft blur ("blur only" mode). Very
// slow, calm drift.
(function () {
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
  function lerp(a, b, t) {
    return [Math.round(a[0] + (b[0] - a[0]) * t), Math.round(a[1] + (b[1] - a[1]) * t), Math.round(a[2] + (b[2] - a[2]) * t)];
  }
  function smoothColor(f, tones) {
    var stops = [0, 0.45, 0.78, 1];
    if (f <= 0) return tones[0];
    if (f >= 1) return tones[3];
    for (var i = 0; i < 3; i++) {
      if (f <= stops[i + 1]) { var t = (f - stops[i]) / (stops[i + 1] - stops[i]); t = t * t * (3 - 2 * t); return lerp(tones[i], tones[i + 1], t); }
    }
    return tones[3];
  }

  class GlassBackground {
    constructor(canvas, settings) {
      this.canvas = canvas;
      this.ctx = canvas.getContext("2d", { alpha: false });
      this.buffer = document.createElement("canvas");
      this.bctx = this.buffer.getContext("2d");
      this.destroyed = false;
      this.start = performance.now();
      this.lastPaint = 0;
      this.applyParams(settings);
      canvas.style.imageRendering = "auto";
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
      this.mode = s.glassMode === "blur" ? "blur" : "panes";
      this.panes = Math.round(clamp(Number(s.glassPanes != null ? s.glassPanes : 18), 6, 40));
      this.softness = clamp(Number(s.glassSoftness != null ? s.glassSoftness : 14), 0, 48);
      this.brightness = clamp(Number(s.glassBrightness != null ? s.glassBrightness : 1), 0.4, 2);
      this.refraction = clamp(Number(s.glassRefraction != null ? s.glassRefraction : 0.02), 0, 0.08);
      this.speed = clamp(Number(s.speed != null ? s.speed : 0.004), 0, 0.01);
      var seed = Number(s.flowSeed != null ? s.flowSeed : 0.5);
      seed = isNaN(seed) ? 0.5 : seed;
      this.seedX = (seed * 97.31) % 100;
      this.seedY = (seed * 57.13 + 11.7) % 100;
    }

    apply(settings) {
      this.applyParams(settings);
      document.body.style.backgroundColor = "#000000";
      this.resize();
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
      var bw = 220;
      var bh = Math.max(1, Math.round(bw * this.canvas.height / this.canvas.width));
      this.buffer.width = bw;
      this.buffer.height = bh;
      this.image = this.bctx.createImageData(bw, bh);
      this.paint(performance.now() - this.start);
    }

    paint(time) {
      var drift = time * this.speed * 0.015;
      var bw = this.buffer.width, bh = this.buffer.height, d = this.image.data, i = 0;
      for (var y = 0; y < bh; y++) {
        for (var x = 0; x < bw; x++) {
          var f = this.field(x / bw, y / bh, drift);
          f = Math.pow(clamp(f * this.brightness, 0, 1), 1.25);
          var c = smoothColor(f, this.tones);
          d[i] = c[0]; d[i + 1] = c[1]; d[i + 2] = c[2]; d[i + 3] = 255; i += 4;
        }
      }
      this.bctx.putImageData(this.image, 0, 0);

      var ctx = this.ctx, w = this.canvas.width, h = this.canvas.height;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      if (this.mode === "blur") {
        this.canvas.style.filter = "blur(" + this.softness + "px)";
        ctx.clearRect(0, 0, w, h);
        ctx.drawImage(this.buffer, 0, 0, w, h);
        return;
      }
      this.canvas.style.filter = "none";
      ctx.clearRect(0, 0, w, h);
      var n = this.panes, sw = w / n;
      for (var k = 0; k < n; k++) {
        var sx = k * sw;
        var shift = ((k % 2 === 0 ? 1 : -1) * this.refraction + Math.sin(k * 1.7 + this.seedX) * this.refraction * 0.6) * h;
        ctx.save();
        ctx.beginPath();
        ctx.rect(sx, 0, sw + 1, h);
        ctx.clip();
        ctx.filter = "blur(" + this.softness + "px)";
        ctx.drawImage(this.buffer, 0, shift, w, h);
        ctx.filter = "none";
        var sheen = ctx.createLinearGradient(sx, 0, sx + sw, 0);
        sheen.addColorStop(0, "rgba(255,255,255,0.05)");
        sheen.addColorStop(0.5, "rgba(255,255,255,0)");
        sheen.addColorStop(1, "rgba(0,0,0,0.10)");
        ctx.fillStyle = sheen;
        ctx.fillRect(sx, 0, sw + 1, h);
        ctx.restore();
        ctx.fillStyle = "rgba(255,255,255,0.12)";
        ctx.fillRect(Math.round(sx), 0, 1, h);
      }
    }

    frame(time) {
      if (this.destroyed) return;
      if (time - this.lastPaint > 80) { this.lastPaint = time; this.paint(time - this.start); }
      requestAnimationFrame((next) => this.frame(next));
    }

    destroy() {
      this.destroyed = true;
      if (this.resizeObserver) { this.resizeObserver.disconnect(); this.resizeObserver = null; }
      this.canvas.style.filter = "none";
    }
  }

  window.createStarleeGlassBackground = function (canvas, settings) {
    return new GlassBackground(canvas, settings);
  };
})();
