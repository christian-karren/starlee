import { build } from "esbuild";
import { cp, mkdir, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const outdir = fileURLToPath(new URL("../dist/extension/", import.meta.url));
await import("./make-icons.mjs");
await rm(outdir, { recursive: true, force: true });
await mkdir(outdir, { recursive: true });
await build({
  entryPoints: ["src/content.js", "src/background.js", "src/options.js"],
  outdir,
  bundle: true,
  format: "esm",
  target: "chrome120",
  minify: true,
  sourcemap: true
});
await cp("extension/manifest.json", `${outdir}/manifest.json`);
await cp("extension/options.html", `${outdir}/options.html`);
await cp("extension/assets", `${outdir}/assets`, { recursive: true });
