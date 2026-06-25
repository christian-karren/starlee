// Starlee background suite dispatcher.
//
// Selects a background engine by `settings.kind` and exposes one controller with
// `.apply(settings)`. On a kind change it destroys the previous engine (stopping
// its loop/observers) AND swaps in a fresh canvas before building the new one —
// a single <canvas> can only ever hold one context type, so moving between a 2D
// engine (pixel-dither / dither / glass / flow) and the WebGL aurora requires a
// clean canvas.
(function () {
  function kindOf(settings) { return (settings && settings.kind) || "pixel-dither"; }

  function make(canvas, settings) {
    var k = kindOf(settings);
    if (k === "flow" && window.createStarleeFlowBackground) return window.createStarleeFlowBackground(canvas, settings);
    if (k === "aurora" && window.createStarleeAuroraBackground) return window.createStarleeAuroraBackground(canvas, settings);
    if (k === "dither" && window.createStarleeDitherBackground) return window.createStarleeDitherBackground(canvas, settings);
    if (k === "glass" && window.createStarleeGlassBackground) return window.createStarleeGlassBackground(canvas, settings);
    return window.createStarleePixelDitherBackground(canvas, settings);
  }

  function freshCanvas(old) {
    var next = old.cloneNode(false);
    next.style.filter = "none";
    if (old.parentNode) old.parentNode.replaceChild(next, old);
    return next;
  }

  window.createStarleeBackground = function (canvas, settings) {
    var current = { kind: kindOf(settings), canvas: canvas, instance: make(canvas, settings) };
    return {
      apply: function (next) {
        var k = kindOf(next);
        if (k !== current.kind) {
          if (current.instance && current.instance.destroy) current.instance.destroy();
          current.canvas = freshCanvas(current.canvas);
          current.kind = k;
          current.instance = make(current.canvas, next);
          return;
        }
        if (current.instance && current.instance.apply) current.instance.apply(next);
      }
    };
  };
})();
