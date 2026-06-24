import { build } from "esbuild";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const outdir = fileURLToPath(new URL("../dist/extension/", import.meta.url));
const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const git = (...args) => {
  try {
    return execFileSync("git", args, { cwd: repoRoot, encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
};
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
await writeFile(`${outdir}/build-info.json`, `${JSON.stringify({
  git_commit: git("rev-parse", "--short", "HEAD"),
  git_branch: git("branch", "--show-current"),
  git_dirty: git("status", "--short") ? "true" : "false",
  built_at: new Date().toISOString()
}, null, 2)}\n`);
