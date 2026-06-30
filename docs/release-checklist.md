# Release checklist

## V1 release baseline

Starlee v1 browser capture is Chrome-only. The branch
`codex/chrome-only-capture-stabilization` is the baseline for promoting browser
capture to production. The source-of-truth contract and manual QA record are in
[`docs/chrome-capture-v1-baseline.md`](chrome-capture-v1-baseline.md).

For v1 release readiness:

- Onboarding mentions Chrome only.
- `./scripts/install.sh` installs Starlee and generates
  `~/Starlee/sensor-extension`; it does not install Safari by default.
- `starlee doctor` and bridge health recommend Chrome, not Firefox or Safari.
- Legacy Firefox/Safari extension state cannot make Chrome capture unhealthy.
- Chrome article capture works on representative modern article pages.
- Chrome YouTube capture saves timestamped transcript text when the rendered
  transcript is available.
- Paul Graham-style old/static pages may remain extraction edge cases for v1.

## Implemented gates

- Markdown vault is canonical and fully reindexable.
- Quantized BGE-small embeddings run locally; no inference API is present.
- Hybrid sqlite-vec + FTS5 retrieval returns source paths, URLs, and snippets.
- MCP stdio uses newline-delimited JSON-RPC and negotiates stable protocol versions.
- Capture endpoint binds to `127.0.0.1` and requires a random 256-bit bearer token.
- Article extraction runs in the rendered browser DOM through Mozilla Readability.
- Access classification uses `isAccessibleForFree`, domain/marker heuristics, and fails closed.
- YouTube transcripts come only from rendered DOM segments, including a bounded
  rendered-page transcript discovery attempt, and retain timestamps.
- YouTube transcript-unavailable captures are saved as restricted records with
  explicit `transcript_status`, `transcript_source`, and `transcript_reason`
  metadata.
- Optional YouTube metadata uses official Data API `videos.list` only.
- URL-only server capture requires an explicit public schema signal.
- Recaptured canonical URLs update in place.
- Menu-bar capture diagnostics retain a bounded local lifecycle trace with no
  tokens, bodies, selected text, transcripts, or restricted content.
- Share export strips all restricted bodies and blocks output on audit failure.
- Borrowed bundles open read-only and return summary/citation for `get`.
- Setup installs the model, extension assets, local token, and example prompts without printing the token.
- Optional macOS menu-bar app supports status, recent items, search, pasted capture, vault access, and endpoint control.

## Validation commands

```sh
cargo fmt --check
cargo test --locked --quiet
cargo clippy --all-targets --locked -- -D warnings
ln -sfn "$(pwd)" /tmp/starlee-gui-test && /tmp/starlee-gui-test/scripts/test-gui.sh
(cd sensor && npm run test:chrome-release)
make package-chrome
./scripts/inspect-chrome-extension-package.sh release/chrome-extension/starlee-capture-0.1.0.zip
make package
```

Before a commercial public release, run the maintained 50-site extraction corpus
against current publisher pages and obtain counsel review for publisher-specific
terms and restricted-text embeddings. Those are operational release activities,
not hidden runtime dependencies.

## Chrome extension release gate

- Upload only the ZIP produced by `scripts/package-chrome-extension.sh`.
- Confirm package inspection passes before upload and keep its JSON output with
  the release candidate.
- Do not treat `make test` alone as the Chrome Web Store gate. It runs
  `sensor` unit tests through `npm test`, but `npm test` is scoped to
  `node --test 'test/*.test.js'` and does not run
  `sensor/test/e2e/extension.test.js` or
  `sensor/test/integration/handoff-loop.test.js`.
- Run `cd sensor && npm run test:chrome-release` before upload. This expands to
  the sensor build, unit tests, handoff integration tests, and Playwright
  extension E2E tests.
- Confirm the Chrome Web Store listing says captured article bodies and
  transcripts are sent only to the user's local Starlee service.
- Confirm the listing includes permission justifications for `storage`,
  `activeTab`, `tabs`, `alarms`, `http://127.0.0.1/*`, `http://*/*`, and
  `https://*/*`.
- Confirm the privacy disclosure says Starlee Capture does not sell, share, or
  upload article bodies, transcripts, browsing history, vault data, or capture
  tokens to Starlee servers.
- Confirm reviewer notes explain the local Starlee app dependency, loopback-only
  endpoint, bearer token, and why broad page content-script access is required
  for macOS menu-bar capture.
- Submit as an unlisted beta before public listing.
- Verify a clean Chrome profile can install, handshake, toolbar-capture,
  menu-bar-capture, and YouTube-capture before public launch.
- Validate one Chrome YouTube watch page with rendered transcript segments and
  one without an available transcript. Confirm the menu-bar success pulse occurs
  only after `capture_saved`, the vault has one restricted canonical video
  record, and bridge health/status do not expose transcript text.
- Verify `starlee diagnostics --last-capture` after one successful capture and
  at least one induced failure. The trace must omit article bodies, transcript
  text, selected text, bearer tokens, and vault data.

## Future Safari local extension gate

Safari is not part of the v1 production baseline. Use this gate only after a
separate Safari branch proves it does not change the Chrome contract in
[`docs/chrome-capture-v1-baseline.md`](chrome-capture-v1-baseline.md).

- Build with `scripts/package-safari-extension.sh`.
- Confirm Safari package inspection passes before loading into Xcode or Safari.
- Full Xcode is required to run Apple's `safari-web-extension-converter`; Command
  Line Tools alone are not enough.
- Treat `release/safari-extension/StarleeSafari` and
  `release/safari-extension/extension` as generated local artifacts unless a
  separate distribution decision checks in a curated wrapper project.
- For local use, install the generated macOS wrapper app with
  `scripts/install-safari-extension.sh`, then enable Starlee in Safari Settings >
  Extensions and grant site access for the pages being captured.
- Confirm `pluginkit -m -A -D -i com.starlee.capture.safari.Extension` lists the
  registered extension after install.
- Verify `starlee doctor`, a Safari article capture, and a Safari YouTube
  transcript capture before treating the local Safari path as working.
- Validate Safari YouTube separately from Chrome because extension permission
  prompts, transcript DOM timing, and local wrapper setup differ. Permission
  failures should resolve to `permission_denied` with actionable recovery text.
- Confirm `starlee diagnostics --last-capture` reports browser `Safari` and does
  not expose capture tokens, OAuth tokens, article bodies, transcript text,
  selected text, raw HTML, cookies, embeddings, or vault file bodies.

## Safari distribution gates

These are not required for local Safari parity, but they are required before
shipping Safari to users outside local development.

- Direct distribution requires Developer ID signing, a hardened runtime decision,
  notarization, stapling, and Gatekeeper verification on a clean Mac.
- Mac App Store distribution requires App Sandbox review, network client
  entitlement review, Safari Web Extension capability, bundle ID ownership,
  provisioning profile, privacy labels, screenshots, review copy, and Apple
  review approval.
- Public release copy must say Starlee reads only pages the user chooses to save
  and sends captured content to the Starlee app running locally on the user's Mac.
- Wrapper app and extension entitlements must be audited after conversion and
  before signing or App Store upload.
