# Chrome Capture V1 Baseline

This document records the production baseline for Starlee browser capture.

As of branch `codex/chrome-only-capture-stabilization`, Starlee v1 supports
Chrome capture only. Chrome is the default, source-of-truth browser path for
onboarding, diagnostics, release packaging, manual QA, and Chrome Web Store
launch prep.

## Product Contract

- Starlee v1 supports Chrome article capture and Chrome YouTube transcript
  capture.
- The macOS menu-bar capture button always creates a Chrome-targeted capture
  request.
- Only a Chrome extension heartbeat can make the browser bridge healthy.
- Firefox and Safari state must not affect capture routing, doctor output,
  bridge health, onboarding, or next-action text.
- Diagnostics are observers. They may explain what happened, but they must not
  choose a browser target or change the capture contract.
- Safari and Firefox are future browser targets. Their plans may live in docs
  and package scripts, but they are not v1 runtime behavior.

## Known Good Manual QA

Manual QA on June 29, 2026 confirmed the Chrome-only baseline after installing
from `codex/chrome-only-capture-stabilization`:

- `./scripts/install.sh` built and installed Starlee.
- The installer generated Chrome extension assets at
  `~/Starlee/sensor-extension`.
- `starlee doctor` recommended Chrome, not Firefox or Safari.
- The Chrome extension was reloaded from `~/Starlee/sensor-extension`.
- Article capture succeeded for:
  - `https://www.dreammachines.ai/p/physical-ai-deep-dive-data-flywheels`
  - `https://stratechery.com/2026/anthropics-safety-superpower/`
  - `https://www.zerohedge.com/political/83-french-favor-deportation-criminals-and-long-term-unemployed-foreigners`
- YouTube capture succeeded and saved timestamped transcript text for:
  - `https://www.youtube.com/watch?v=QzvadHngNnI`
  - `https://www.youtube.com/watch?v=dITKchI1HME&t=5s`
- `https://www.paulgraham.com/taste.html` failed capture. This is accepted as
  an extraction edge case for v1 because the broader modern article and YouTube
  contract held.

## Automated Gates

Before promoting a change that touches browser capture, run:

```sh
cargo fmt --check
cargo test --locked --quiet
cargo clippy --all-targets --locked -- -D warnings
ln -sfn "$(pwd)" /tmp/starlee-gui-test && /tmp/starlee-gui-test/scripts/test-gui.sh
(cd sensor && npm run test:chrome-release)
```

The baseline branch added regression coverage for the failure that caused the
Firefox rollback: legacy Firefox extension state in local config must not make
Chrome-only bridge health or doctor output recommend Firefox.

## Future Browser Rule

Do not add Safari or Firefox support by generalizing the Chrome path in place.
Any future browser target must be developed on its own branch with tests proving:

- Chrome capture still works.
- Chrome doctor/bridge health is unchanged.
- Stale non-Chrome state cannot affect Chrome capture.
- The new browser has its own explicit install, permission, capture, and
  diagnostic contract.
