// Starlee "Aurora" background engine.
//
// A full-screen WebGL canvas running a domain-warped simplex-noise shader (the
// shader is the aurora-gradient-background technique, copied verbatim): 2-3
// colors painted into a slowly morphing field, mixed toward a solid base so the
// base stays dominant. Black base, with navy/cream/white washing through.
(function () {
  function hex2rgb(h) {
    var n = parseInt(String(h || "").replace("#", ""), 16);
    if (isNaN(n)) return [0, 0, 0];
    return [(n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255];
  }
  function clamp(v, a, b) { return Math.min(b, Math.max(a, v)); }

  var VERT = "attribute vec2 p;void main(){gl_Position=vec4(p,0.0,1.0);}";
  var FRAG = [
    "precision highp float;",
    "uniform vec2 u_res; uniform float u_time; uniform float u_intensity;",
    "uniform vec3 u_c1,u_c2,u_c3,u_base;",
    "vec3 mod289(vec3 x){return x-floor(x*(1.0/289.0))*289.0;}",
    "vec2 mod289(vec2 x){return x-floor(x*(1.0/289.0))*289.0;}",
    "vec3 permute(vec3 x){return mod289(((x*34.0)+1.0)*x);}",
    "float snoise(vec2 v){",
    "  const vec4 C=vec4(0.211324865405187,0.366025403784439,-0.577350269189626,0.024390243902439);",
    "  vec2 i=floor(v+dot(v,C.yy));vec2 x0=v-i+dot(i,C.xx);",
    "  vec2 i1=(x0.x>x0.y)?vec2(1.0,0.0):vec2(0.0,1.0);",
    "  vec4 x12=x0.xyxy+C.xxzz;x12.xy-=i1;i=mod289(i);",
    "  vec3 pp=permute(permute(i.y+vec3(0.0,i1.y,1.0))+i.x+vec3(0.0,i1.x,1.0));",
    "  vec3 m=max(0.5-vec3(dot(x0,x0),dot(x12.xy,x12.xy),dot(x12.zw,x12.zw)),0.0);",
    "  m=m*m;m=m*m;vec3 x=2.0*fract(pp*C.www)-1.0;vec3 h=abs(x)-0.5;",
    "  vec3 ox=floor(x+0.5);vec3 a0=x-ox;",
    "  m*=1.79284291400159-0.85373472095314*(a0*a0+h*h);",
    "  vec3 g;g.x=a0.x*x0.x+h.x*x0.y;g.yz=a0.yz*x12.xz+h.yz*x12.yw;",
    "  return 130.0*dot(m,g);",
    "}",
    "void main(){",
    "  vec2 uv=gl_FragCoord.xy/u_res; vec2 p=uv; p.x*=u_res.x/u_res.y; float t=u_time*0.05;",
    "  float w1=snoise(p*1.1+vec2(t,t*0.6));",
    "  float w2=snoise(p*1.4+vec2(-t*0.7,t*0.9)+w1*0.6);",
    "  vec2 q=p+0.38*vec2(w1,w2);",
    "  float n=snoise(q*0.95+t*0.35); float n2=snoise(q*1.7-t*0.28+7.0);",
    "  float m=n*0.5+0.5; float m2=n2*0.5+0.5;",
    "  vec3 col=mix(u_c3,u_c1,smoothstep(0.20,0.80,m2));",
    "  col=mix(col,u_c2,smoothstep(0.55,1.0,(m+m2)*0.5));",
    "  float cov=smoothstep(0.34,0.92,m)*u_intensity;",
    "  gl_FragColor=vec4(mix(u_base,col,cov),1.0);",
    "}"
  ].join("\n");

  class AuroraBackground {
    constructor(canvas, settings) {
      this.canvas = canvas;
      this.destroyed = false;
      this.start = performance.now();
      this.gl = null;
      try { this.gl = canvas.getContext("webgl") || canvas.getContext("experimental-webgl"); } catch (e) {}
      this.applyParams(settings);
      canvas.style.imageRendering = "auto";
      canvas.style.filter = "none";
      document.body.style.backgroundColor = "#000000";
      if (!this.gl) { return; }
      this.initGL();
      this.resizeObserver = new ResizeObserver(() => this.resize());
      this.resizeObserver.observe(document.documentElement);
      this.resize();
      requestAnimationFrame((t) => this.frame(t));
    }

    applyParams(s) {
      s = s || {};
      this.base = hex2rgb(s.black || "#000000");
      this.c1 = hex2rgb(s.backgroundColor || "#F2E3B6"); // cream / spread
      this.c3 = hex2rgb(s.pixelColor || "#13284B");      // navy / mid
      this.c2 = hex2rgb(s.white || "#FFFFFF");           // white / dense pools
      this.intensity = clamp(Number(s.auroraIntensity != null ? s.auroraIntensity : 0.55), 0.1, 1);
      this.speed = clamp(Number(s.speed != null ? s.speed : 0.7), 0, 2.5);
      var seed = Number(s.flowSeed != null ? s.flowSeed : 0.5);
      this.seed = isNaN(seed) ? 0.5 : seed;
    }

    apply(settings) {
      this.applyParams(settings);
      document.body.style.backgroundColor = "#000000";
    }

    initGL() {
      var gl = this.gl;
      var compile = (type, src) => { var sh = gl.createShader(type); gl.shaderSource(sh, src); gl.compileShader(sh); return sh; };
      var prog = gl.createProgram();
      gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
      gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
      gl.linkProgram(prog);
      gl.useProgram(prog);
      var buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
      var loc = gl.getAttribLocation(prog, "p");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
      this.U = {
        res: gl.getUniformLocation(prog, "u_res"),
        time: gl.getUniformLocation(prog, "u_time"),
        intensity: gl.getUniformLocation(prog, "u_intensity"),
        base: gl.getUniformLocation(prog, "u_base"),
        c1: gl.getUniformLocation(prog, "u_c1"),
        c2: gl.getUniformLocation(prog, "u_c2"),
        c3: gl.getUniformLocation(prog, "u_c3")
      };
    }

    resize() {
      if (!this.gl) return;
      var dpr = Math.min(window.devicePixelRatio || 1, 1.75);
      this.canvas.width = Math.max(1, Math.floor(window.innerWidth * dpr));
      this.canvas.height = Math.max(1, Math.floor(window.innerHeight * dpr));
      this.canvas.style.width = "100vw";
      this.canvas.style.height = "100vh";
      this.gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      this.gl.uniform2f(this.U.res, this.canvas.width, this.canvas.height);
    }

    frame(t) {
      if (this.destroyed || !this.gl) return;
      var gl = this.gl;
      gl.uniform3fv(this.U.base, this.base);
      gl.uniform3fv(this.U.c1, this.c1);
      gl.uniform3fv(this.U.c3, this.c3);
      gl.uniform3fv(this.U.c2, this.c2);
      gl.uniform1f(this.U.intensity, this.intensity);
      gl.uniform1f(this.U.time, ((t - this.start) / 1000) * this.speed + this.seed * 40);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      requestAnimationFrame((next) => this.frame(next));
    }

    destroy() {
      this.destroyed = true;
      if (this.resizeObserver) { this.resizeObserver.disconnect(); this.resizeObserver = null; }
      try {
        var ext = this.gl && this.gl.getExtension("WEBGL_lose_context");
        if (ext) ext.loseContext();
      } catch (e) {}
    }
  }

  window.createStarleeAuroraBackground = function (canvas, settings) {
    return new AuroraBackground(canvas, settings);
  };
})();
