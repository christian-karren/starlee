import { build } from "esbuild";
import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const targets = {
  chrome: {
    outdir: "../dist/extension/",
    manifest: "extension/manifest.json",
    esbuildTarget: "chrome120"
  },
  firefox: {
    outdir: "../dist/firefox-extension/",
    manifest: "extension/manifest.firefox.json",
    esbuildTarget: "firefox128"
  }
};
const target = parseTarget(process.argv.slice(2));
const config = targets[target];
const outdir = fileURLToPath(new URL(config.outdir, import.meta.url));
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
  target: config.esbuildTarget,
  minify: true,
  sourcemap: true
});
await cp(config.manifest, `${outdir}/manifest.json`);
await cp("extension/options.html", `${outdir}/options.html`);
await cp("extension/assets", `${outdir}/assets`, { recursive: true });
await writeFile(`${outdir}/build-info.json`, `${JSON.stringify({
  target,
  git_commit: git("rev-parse", "--short", "HEAD"),
  git_branch: git("branch", "--show-current"),
  git_dirty: git("status", "--short") ? "true" : "false",
  built_at: new Date().toISOString()
}, null, 2)}\n`);

function parseTarget(args) {
  const found = args.find((arg) => arg === "--target" || arg.startsWith("--target="));
  if (!found) return "chrome";
  const value = found === "--target"
    ? args[args.indexOf(found) + 1]
    : found.slice("--target=".length);
  if (!Object.hasOwn(targets, value)) {
    throw new Error(`Unsupported extension build target: ${value || "(missing)"}`);
  }
  return value;
}
