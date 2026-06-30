# Chrome Web Store Launch Readiness

Date: 2026-06-30
Branch: `codex/chrome-web-store-launch`
Baseline: `origin/main` at `11b2ef7`
Package: `release/chrome-extension/starlee-capture-0.1.0.zip`

Starlee Capture is ready for an unlisted Chrome Web Store beta upload after the
manual dashboard assets below are prepared. The working Chrome-only capture
baseline is preserved: no runtime code, manifest permissions, packaging
behavior, onboarding contract, or capture architecture changed for this launch
readiness pass.

## Current Status

- Chrome-only v1 baseline was merged in PR #39, `11b2ef7`, and tagged
  `chrome-capture-v1-baseline-2026-06-29`.
- Source of truth remains
  [`docs/chrome-capture-v1-baseline.md`](chrome-capture-v1-baseline.md).
- Manifest is Manifest V3 and requests exactly:
  - `storage`
  - `activeTab`
  - `tabs`
  - `alarms`
  - `http://127.0.0.1/*`
  - `http://*/*`
  - `https://*/*`
- Broad page access remains intentional for the native macOS menu-bar capture
  path. A menu-bar click is not a Chrome toolbar click and does not grant
  Chrome's temporary `activeTab` permission.
- Firefox and Safari remain future work. Do not mention them in Chrome Web Store
  launch claims, onboarding, reviewer notes, or diagnostics as supported v1
  browser targets.

## Package Verification

Run these commands before every Chrome Web Store upload candidate:

```sh
cd sensor && npm run test:chrome-release
make package-chrome
./scripts/inspect-chrome-extension-package.sh \
  release/chrome-extension/starlee-capture-0.1.0.zip
```

The inspection gate must report `ok: true` and confirm the ZIP excludes:

- `starlee-config.json`
- capture tokens
- vault data
- local config
- source maps
- model files
- `node_modules`
- unexpected remote URLs

## Chrome Web Store Requirements Checked

Official Google docs used for this readiness pass:

- Publish/upload: the Chrome Web Store dashboard accepts a ZIP whose manifest is
  valid and located at the root of the ZIP.
- Manifest V3: new public/unlisted Manifest V2 extensions are no longer
  accepted; Manifest V3 uses a service worker and disallows remotely hosted
  executable code.
- Listing assets: the dashboard requires store listing details, a 128x128 store
  icon, at least one 1280x800 screenshot, and a 440x280 small promotional tile.
- Privacy fields: the privacy practices tab asks for the extension's single
  purpose, permission justifications, and user data handling disclosures.
- User data policy: products that handle sensitive user data, even locally, need
  a privacy policy that describes collection, use, sharing, security, access,
  deletion, and retention.
- Single-purpose policy: requested permissions must directly support the narrow
  stated purpose.
- Distribution: submit the first candidate as unlisted beta; unlisted items go
  through the same policy review as public items.

## Store Listing Copy

Short description:

> Save rendered articles and YouTube transcripts to your local Starlee brain.

Long description:

> Starlee Capture lets you save the article or YouTube transcript you are
> reading into Starlee, a local-first digital brain on your Mac. Click the
> Starlee toolbar button, the in-page save button, or the Starlee menu-bar app;
> the extension extracts readable text and metadata from the active tab and sends
> it to the Starlee app running locally on your computer.
>
> Starlee Capture does not upload article bodies, transcripts, browsing history,
> vault data, or capture tokens to Starlee servers. The extension communicates
> with `127.0.0.1`, your own computer, where the Starlee local service stores and
> indexes captures in `~/Starlee`.

Single-purpose statement:

> Starlee Capture saves the rendered article or YouTube transcript the user is
> viewing into the local Starlee app running on that user's Mac.

## Permission Justifications

- `storage`: stores the local capture token, port, and redacted connection state.
- `activeTab`: captures the tab the user chooses from the Chrome toolbar.
- `tabs`: finds the active browser tab when the Mac menu-bar app requests a save.
- `alarms`: performs low-frequency local polling for menu-bar capture requests.
- `http://127.0.0.1/*`: talks to the Starlee service running on the user's Mac.
- `http://*/*` and `https://*/*`: keep the extractor available for the native
  menu-bar one-click flow on pages the user may save.

Do not broaden permissions. Do not remove broad host permissions before launch
unless menu-bar capture has an equivalent tested replacement.

## Privacy Disclosure

Use [`docs/chrome-web-store-privacy-policy.md`](chrome-web-store-privacy-policy.md)
as the draft privacy policy page. The Chrome Web Store privacy disclosure should
match it:

> Starlee Capture sends captured article text, selected text, metadata, and
> YouTube transcript data only to `127.0.0.1`, the user's own computer, where the
> Starlee app stores captures in `~/Starlee`. The extension does not sell, share,
> upload, or transmit article bodies, transcripts, browsing history, vault data,
> or capture tokens to Starlee servers.

## Reviewer Notes

> This extension depends on the native Starlee app or `starlee serve` running
> locally on macOS. All capture endpoints are loopback-only and authenticated
> with a locally generated bearer token. Broad `http://*/*` and `https://*/*`
> content script access is used so the native macOS menu-bar capture button can
> save the active browser page; a native menu-bar click does not grant Chrome's
> temporary `activeTab` permission.

## Screenshot Checklist

Create these dashboard assets before submitting:

- Chrome toolbar button on a representative article page.
- Extension options page showing connected local Starlee state.
- macOS menu-bar "Save Current Article" flow.
- Saved result or recent capture visible in Starlee.
- At least one 1280x800 screenshot.
- 128x128 store icon.
- 440x280 small promotional tile.

Avoid any screenshot or copy that implies Safari or Firefox support in v1.

## Unlisted Beta Submission Checklist

- Register and pay for the Chrome Web Store developer account.
- Upload `release/chrome-extension/starlee-capture-0.1.0.zip`.
- Set visibility to unlisted for the first beta.
- Enter the short description, long description, single-purpose statement,
  permission justifications, privacy disclosure, reviewer notes, category,
  support contact, and privacy policy URL.
- Attach the screenshots and promotional tile.
- Keep the package inspection JSON and Chrome release test output with the
  release candidate notes.

## Post-Approval Clean-Profile Test

After unlisted approval, verify on a clean Chrome profile:

- Install from the Chrome Web Store listing.
- Start the local Starlee app or `starlee serve`.
- Open the extension options page and confirm connected local state.
- Confirm `starlee doctor` records a recent Chrome extension handshake.
- Toolbar-capture one representative article.
- Menu-bar-capture one representative article.
- Capture one YouTube watch page with rendered transcript segments.
- Capture or verify fallback behavior for one YouTube watch page without an
  available transcript.
- Run `starlee diagnostics --last-capture` and confirm it has terminal status
  without article bodies, selected text, transcript text, full bearer tokens, or
  vault data.

## Preserved Manual QA Context

Successful Chrome-only baseline captures:

- `https://www.dreammachines.ai/p/physical-ai-deep-dive-data-flywheels`
- `https://stratechery.com/2026/anthropics-safety-superpower/`
- `https://www.zerohedge.com/political/83-french-favor-deportation-criminals-and-long-term-unemployed-foreigners`
- `https://www.youtube.com/watch?v=QzvadHngNnI`
- `https://www.youtube.com/watch?v=dITKchI1HME&t=5s`

Accepted v1 edge case:

- `https://www.paulgraham.com/taste.html` failed capture. Treat this as
  post-v1 extraction polish unless a tiny, low-risk, tested fix is available.

## Manual Launch Answer

Ready to upload as an unlisted beta: yes, after Christian prepares the required
Chrome Web Store screenshots, small promotional tile, category/support fields,
developer account payment, and hosted privacy policy URL.

Not ready for public listing: do not go public until the unlisted beta is
approved and the post-approval clean-profile test passes.
