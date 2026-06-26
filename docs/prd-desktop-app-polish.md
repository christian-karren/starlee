# PRD: Starlee Desktop App — UI Polish Pass (Make Interfaces Feel Better)

**Author:** Christian Karren
**Date:** 2026-06-25
**Status:** Draft
**Version:** 1.0
**Quality-Validated:** Yes

> **Branch & workflow:** Implement on branch `feature/desktop-app-polish`, cut
> from `feature/desktop-app-v1` once PRD 1 lands (never on `main`). This is PRD 2
> of 2 and is a sequential follow-on to `docs/prd-desktop-app-v1.md`: it only
> runs after the functional surfaces (reader, Filter/Edit/Upload, topics,
> Settings redesign, onboarding) exist, because a polish pass cannot polish
> surfaces that have not been built. Land via PR with `make test` and
> `./scripts/legal-invariants.sh` green and **zero product-behavior changes**.
>
> **Source skill:** This PRD operationalizes the `make-interfaces-feel-better`
> agent skill (`npx skills add jakubkrehel/make-interfaces-feel-better`), based on
> the article "Details that make interfaces feel better." Every requirement below
> maps one of that skill's principles to a concrete Starlee surface and uses the
> skill's exact prescribed values.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Problem Statement](#problem-statement)
3. [Goals & Success Metrics](#goals--success-metrics)
4. [User Stories](#user-stories)
5. [Functional Requirements](#functional-requirements)
6. [Non-Functional Requirements](#non-functional-requirements)
7. [Technical Considerations](#technical-considerations)
8. [Implementation Roadmap](#implementation-roadmap)
9. [Out of Scope](#out-of-scope)
10. [Open Questions & Risks](#open-questions--risks)
11. [Validation Checkpoints](#validation-checkpoints)
12. [Appendix: Task Breakdown Hints](#appendix-task-breakdown-hints)

---

## Executive Summary

After PRD 1 makes the desktop app functional, it will still "work but feel off":
the renderer ([`gui/Resources/renderer/styles.css`](../gui/Resources/renderer/styles.css),
[`settings.css`](../gui/Resources/renderer/settings.css)) and the native AppKit
shell (menu-bar popover, status window, dialogs, menu-bar capture animation) were
vibe-coded without the design-engineering details that make software feel
intentional and premium. This PRD applies a disciplined polish pass derived from
the `make-interfaces-feel-better` skill across every desktop surface — concentric
border radii, shadow-as-border depth, optical alignment, font smoothing, tabular
numbers, balanced text wrapping, interruptible transitions, staggered enters,
subtle exits, contextual icon animations, scale-on-press, GPU-discipline, and
≥40×40px hit areas — using the skill's exact values (e.g., `scale(0.96)` on press,
icon `scale 0.25→1` + `blur 4px→0px` + `bounce 0`, `100ms` stagger,
`rgba(0,0,0,0.1)` image outlines). The pass changes **no product behavior**:
layout, brand colors (navy `#062F64`/`#17152B`, cream `#F9E4B6`), copy, and
features are preserved; only visual rendering and micro-interactions improve. The
target is ≥95% of applicable checklist items passing on each of ~8 surfaces, all
codified in reusable design tokens, with zero behavior regressions.

---

## Problem Statement

### Current Situation

Starlee's UI is split across two rendering technologies, both authored without
systematic polish:

1. **WKWebView renderer** (HTML/CSS/JS) — the Library, reader, Settings, and
   onboarding. Plain CSS (no Tailwind/React/Framer), so the skill's Tailwind/
   Framer-Motion snippets must be translated to CSS transitions/variables.
2. **Native AppKit shell** — the menu-bar status menu
   ([`gui/StatusMenuController.swift`](../gui/StatusMenuController.swift)), desktop
   window ([`gui/DesktopWindowController.swift`](../gui/DesktopWindowController.swift)),
   menu-bar icon + capture animation ([`gui/MenuBarIcon.swift`](../gui/MenuBarIcon.swift)),
   dialogs ([`gui/DialogPresenter.swift`](../gui/DialogPresenter.swift)),
   notifications ([`gui/NotificationController.swift`](../gui/NotificationController.swift)),
   and the animated background ([`gui/FluidBackground.swift`](../gui/FluidBackground.swift)).

Observable symptoms today: nested elements share border radii (no concentric
math), depth is conveyed by hard borders rather than layered shadows, icons are
geometrically (not optically) centered, dynamic numbers shift layout as they
update, headings leave orphan words, interactive elements lack tactile press
feedback, some animations are non-interruptible keyframes, and small controls have
sub-40px hit areas. None of these are bugs — each is a detail whose absence makes
the app feel cheaper than its engineering deserves.

### User Impact

- **Who is affected:** Every desktop user, on every interaction.
- **How they're affected:** The app reads as "vibe-coded" rather than crafted —
  laggy/abrupt micro-interactions, inconsistent depth, and small visual
  imperfections that, compounded, erode perceived quality and trust.
- **Severity:** Medium — no functionality is blocked, but perceived quality
  directly affects whether users adopt the app as a daily, lifelong tool.

### Business Impact

- **Cost of problem:** A capture-and-recall product people are meant to use for
  decades must feel trustworthy and durable; an unpolished shell undercuts that.
- **Opportunity cost:** Polish is a cheap, high-leverage differentiator versus
  re-architecture; skipping it forgoes low-cost (~75h) perceived-quality gains.
- **Strategic importance:** Establishes a reusable design-token system that keeps
  every future surface consistent at low marginal cost.

### Why Solve This Now?

The polish pass is, by definition, a finishing step: it must run after PRD 1
builds the surfaces, and before the app is shown widely. Doing it now — while the
surface count is small (~8) — also lets the design tokens it produces govern all
later UI for free.

---

## Goals & Success Metrics

### Goal 1: Apply the polish checklist across every surface

- **Description:** Run the skill's 14-point review checklist against all desktop
  surfaces and remediate every applicable item.
- **Metric:** Percentage of applicable checklist items passing per surface,
  documented in before/after tables.
- **Baseline:** 0% formally verified (no checklist has been applied).
- **Target:** ≥95% of applicable items passing on each of the ~8 surfaces, with a
  per-surface before/after table committed to `docs/`.
- **Timeframe:** End of this milestone.
- **Measurement Method:** Per-surface checklist audit committed as
  `docs/polish-audit.md`; reviewer sign-off.

### Goal 2: Zero product-behavior regressions

- **Description:** Polish changes rendering only — never behavior, layout
  structure, copy, or features.
- **Metric:** Count of behavior regressions (failing existing tests, changed QA
  outcomes, altered feature behavior).
- **Baseline:** N/A (no changes yet).
- **Target:** 0 regressions; `make test`, `legal-invariants.sh`, the sensor suite,
  and the PRD-1 manual QA script all produce identical functional outcomes.
- **Timeframe:** At merge.
- **Measurement Method:** Full test suite + the PRD-1 QA script re-run pre/post.

### Goal 3: Meet animation-performance discipline

- **Description:** All transitions are property-specific and GPU-friendly.
- **Metric:** (a) Count of `transition: all` / shorthand `transition` occurrences
  in renderer CSS; (b) count of `will-change` declarations on non-compositable
  properties or `will-change: all`; (c) interaction frame rate.
- **Baseline:** (a) measured at audit start (current count TBD); (b) measured; (c)
  unmeasured.
- **Target:** (a) 0; (b) 0 (only `transform`/`opacity`/`filter`/`clip-path`, added
  only where first-frame stutter is observed); (c) ≥58fps sustained during press,
  hover, enter, and exit animations on the reference Mac.
- **Timeframe:** End of this milestone.
- **Measurement Method:** `grep` counts in CI + manual frame-rate capture.

### Goal 4: Consolidate styling into reusable design tokens

- **Description:** Radii, shadows, durations, easings, and outline colors live in
  one token source and are reused, not redefined ad hoc.
- **Metric:** Count of hardcoded shadow/border-radius/duration literals outside
  the central token file.
- **Baseline:** Many ad-hoc literals across `styles.css`/`settings.css` (counted at
  audit start).
- **Target:** 0 ad-hoc depth/radius/timing literals outside the token file; native
  AppKit uses matching constants (a `DesignTokens.swift`).
- **Timeframe:** End of this milestone.
- **Measurement Method:** `grep` audit of literals vs. token references.

---

## User Stories

### Story 1: Surfaces feel cohesive and deep (REQ-101, REQ-103, REQ-111)

**As a** Starlee user,
**I want** cards, panels, buttons, and the reader to have correct concentric
radii, shadow-based depth, and clean image edges,
**So that** the app looks intentionally designed rather than assembled.

**Acceptance Criteria:**
- [ ] Every nested rounded surface satisfies `outerRadius = innerRadius + padding`
      (skill rule); pairs are listed in the before/after table (e.g., Library card
      → inner thumbnail, reader panel → inner blocks, settings section → inner
      controls). Where padding >24px, layers are treated as independent surfaces.
- [ ] Depth on cards/containers/buttons/dropdowns uses the skill's layered
      `box-shadow` token (light mode: `0 0 0 1px rgba(0,0,0,0.06)`, `0 1px 2px -1px
      rgba(0,0,0,0.06)`, `0 2px 4px 0 rgba(0,0,0,0.04)`; hover variant per skill)
      instead of hard 1px borders — **except** true dividers/separators, which stay
      borders.
- [ ] Images/thumbnails/favicons get a `1px` inset outline at exactly
      `rgba(0,0,0,0.1)` (light) / `rgba(255,255,255,0.1)` (dark) with
      `outline-offset: -1px` — never a tinted near-black/near-white.
- [ ] Native AppKit surfaces (popover, status window chrome, dialog cards) get
      matching `CALayer` corner radii and `NSShadow`/layer shadows from
      `DesignTokens.swift`.

**Task Breakdown Hint:**
- Task 101.1: Create CSS token block (`:root` vars) + `DesignTokens.swift` (~5h)
- Task 101.2: Concentric-radius audit + fixes across renderer surfaces (~6h)
- Task 101.3: Shadow-as-border migration (renderer + AppKit) (~6h)
- Task 101.4: Image-outline rule across thumbnails/favicons (~2h)

**Dependencies:** None (foundation for all visual REQs).

### Story 2: Interactions feel tactile and responsive (REQ-107, REQ-108, REQ-112, REQ-114, REQ-116)

**As a** Starlee user,
**I want** buttons to depress on click, hover/selection to respond instantly,
icons to animate contextually, and controls to have ≥40×40px hit areas,
**So that** the app feels alive and physical instead of static.

**Acceptance Criteria:**
- [ ] Interactive buttons scale to exactly `scale(0.96)` on press via an
      interruptible CSS transition (`transition-property: scale; 150ms ease-out`);
      a `static` opt-out exists for controls where motion distracts. No press
      scale is below `0.95`.
- [ ] Library hover/selection state (already corrected functionally in PRD-1
      REQ-001) uses an interruptible transition on the shadow/border token, applied
      in <100ms, distinct hover vs. selected states.
- [ ] Contextual icons (e.g., capture-state glyph, edit-mode toggle, expand/
      collapse carets) animate with the skill's exact values: `scale 0.25→1`,
      `opacity 0→1`, `filter blur(4px)→blur(0px)`. Because the renderer has no
      motion library, use the CSS cross-fade pattern (both icons in DOM, one
      absolutely positioned, easing `cubic-bezier(0.2,0,0,1)`); native AppKit uses
      a matching `CABasicAnimation` with `bounce 0`.
- [ ] All animated state changes use interruptible CSS transitions (not one-shot
      keyframes) so reversing mid-animation is smooth (drawer/menu open-close,
      reader open-close).
- [ ] Every interactive control has a ≥40×40px hit area (extend small controls —
      e.g., the card minus-delete button, carets — with a pseudo-element), and no
      two hit areas overlap.

**Task Breakdown Hint:**
- Task 102.1: `scale(0.96)` press utility + `static` opt-out (renderer + AppKit) (~5h)
- Task 102.2: Interruptible hover/selection transitions on shadow token (~3h)
- Task 102.3: Contextual icon cross-fade pattern + AppKit equivalent (~6h)
- Task 102.4: Convert interactive keyframe animations to transitions (~4h)
- Task 102.5: Hit-area audit + pseudo-element extensions (~4h)

**Dependencies:** REQ-101 (tokens), PRD-1 REQ-001/REQ-002 (surfaces exist).

### Story 3: Entrances and exits feel composed (REQ-104, REQ-105, REQ-106, REQ-113)

**As a** Starlee user,
**I want** views and lists to animate in with staggered, composed motion and exit
subtly,
**So that** transitions feel designed and never jarring or attention-stealing.

**Acceptance Criteria:**
- [ ] Multi-element views (onboarding steps, reader, settings sections) split into
      semantic chunks and stagger their enter with ~100ms between groups (~80ms
      between title words where a hero title is split), combining `opacity` +
      `translateY(12px→0)` + `blur(4px→0)`.
- [ ] Exits are subtle: small fixed `translateY(-12px)` + `opacity→0` +
      `blur(4px)` over ~150ms (shorter than the ~300–400ms enter), never a full-
      height slide or dramatic scale.
- [ ] Enter animations do not replay on every render of already-default-state
      elements (the CSS analog of `initial={false}`): first paint of a static
      surface is animation-free; only genuine entrances animate.
- [ ] Onboarding step transitions and reader open/close use these patterns.

**Task Breakdown Hint:**
- Task 103.1: CSS stagger utility (`nth-child` delays + `fadeInUp` keyframe per
      skill) (~4h)
- Task 103.2: Apply staggered enters to onboarding/reader/settings (~5h)
- Task 103.3: Subtle exit utility + application (~3h)
- Task 103.4: Guard against replay-on-load for default-state elements (~2h)

**Dependencies:** REQ-101; PRD-1 REQ-002/REQ-007/REQ-008 (surfaces exist).

### Story 4: Text and numbers render crisply (REQ-109, REQ-110, REQ-115)

**As a** Starlee user,
**I want** crisp macOS text, no orphaned words, and numbers that don't jiggle as
they update,
**So that** reading my brain feels clean and stable.

**Acceptance Criteria:**
- [ ] `-webkit-font-smoothing: antialiased` (+ `-moz-osx-font-smoothing:
      grayscale`) is applied once at the renderer root (not per element).
- [ ] Headings use `text-wrap: balance`; body/descriptions/captions/card text use
      `text-wrap: pretty`; long bodies (≥10 lines, e.g., the reader article text)
      use neither.
- [ ] Dynamically updating numbers (capture counts, diagnostics counters, library
      result counts, timers, month tallies) use `font-variant-numeric:
      tabular-nums`; static numerals (version strings, IDs) do not. Native AppKit
      dynamic numbers use `NSFont.monospacedDigitSystemFont`.
- [ ] The tabular-nums change is visually verified in the renderer's actual font
      (the `1`-width caveat from the skill).

**Task Breakdown Hint:**
- Task 104.1: Root font smoothing + verify across surfaces (~1h)
- Task 104.2: Heading `balance` / body `pretty` pass (~3h)
- Task 104.3: `tabular-nums` on dynamic numbers (renderer + AppKit) (~3h)

**Dependencies:** REQ-101.

---

## Functional Requirements

| ID | Requirement (skill principle → Starlee surface) | Priority |
| --- | --- | --- |
| REQ-101 | Concentric border radius (`outer = inner + padding`) on all nested rounded surfaces (renderer + AppKit). | P0 (Must) |
| REQ-102 | Optical (not geometric) alignment for icon+text buttons (`icon-side padding = text-side − 2px`), the menu-bar/play-style glyphs (shift ~2px), and asymmetric icons (fix in SVG where possible). | P1 (Should) |
| REQ-103 | Replace depth borders on cards/containers/buttons/dropdowns with the layered `box-shadow` token; keep true dividers as borders. | P0 (Must) |
| REQ-104 | Use interruptible CSS transitions for all interactive state changes; reserve keyframes for one-shot sequences. | P0 (Must) |
| REQ-105 | Split-and-stagger enter animations (~100ms groups / ~80ms title words; opacity + translateY + blur). | P1 (Should) |
| REQ-106 | Subtle exit animations (fixed `translateY(-12px)`, ~150ms, shorter than enter). | P1 (Should) |
| REQ-107 | Contextual icon animations with exact values `scale 0.25→1`, `opacity 0→1`, `blur 4px→0`, `bounce 0`; CSS cross-fade in renderer, `CABasicAnimation` in AppKit. | P1 (Should) |
| REQ-108 | `scale(0.96)` interruptible press feedback on buttons, with a `static` opt-out; never below `0.95`. | P0 (Must) |
| REQ-109 | `-webkit-font-smoothing: antialiased` at the renderer root. | P0 (Must) |
| REQ-110 | `tabular-nums` on dynamically updating numbers (renderer) / `monospacedDigitSystemFont` (AppKit). | P1 (Should) |
| REQ-111 | `1px` inset image outlines at `rgba(0,0,0,0.1)` light / `rgba(255,255,255,0.1)` dark, `outline-offset: -1px`. | P1 (Should) |
| REQ-112 | Interruptible hover/selection feedback (<100ms) on the shadow/border token across the Library and controls. | P0 (Must) |
| REQ-113 | Skip enter animations on first paint for already-default-state elements (CSS `initial={false}` analog). | P2 (Could) |
| REQ-114 | Never `transition: all`; transition only the specific properties that change. | P0 (Must) |
| REQ-115 | `text-wrap: balance` on headings, `text-wrap: pretty` on short/medium body text, neither on long bodies. | P1 (Should) |
| REQ-116 | ≥40×40px hit areas on all interactive controls (extend small ones with pseudo-elements); no overlapping hit areas. | P0 (Must) |
| REQ-117 | `will-change` only on `transform`/`opacity`/`filter`/`clip-path`, added only where first-frame stutter is observed; never `will-change: all`. | P1 (Should) |
| REQ-118 | All changes consolidated into a single CSS token block + `DesignTokens.swift`; before/after tables committed to `docs/polish-audit.md`. | P0 (Must) |

---

## Non-Functional Requirements

**Behavior preservation (hard constraint)**
- No product behavior, layout structure, copy, navigation, or feature changes.
  All existing tests, `legal-invariants.sh`, the sensor suite, and the PRD-1
  manual QA script must produce identical functional outcomes pre/post.
- Brand palette is fixed: navy `#062F64` / `#17152B`, cream `#F9E4B6`. Polish must
  not introduce new brand hues (neutral shadow/outline values from the skill are
  permitted and are not "brand" colors).

**Animation & performance**
- Interaction animations sustain ≥58fps on the reference Mac during press, hover,
  enter, and exit.
- 0 occurrences of `transition: all` (or shorthand `transition`) in renderer CSS.
- `will-change` appears only on compositable properties and only where stutter is
  observed; 0 occurrences of `will-change: all`.
- Press feedback completes its transition within 150ms; enters run 300–400ms;
  exits run ~150ms; contextual icon transitions run ~300ms with `bounce 0`.

**Consistency**
- 0 ad-hoc depth/radius/timing literals outside the token source; AppKit constants
  match the CSS token values numerically.

**Accessibility**
- All interactive controls keep ≥40×40px hit areas and visible keyboard focus;
  motion additions respect `prefers-reduced-motion` (reduced/!disabled motion path
  required for all enter/exit/icon animations).

**Verification**
- Each of the ~8 surfaces has a committed before/after table covering every
  applicable checklist item; ≥95% of applicable items pass.

---

## Technical Considerations

### Architecture

- **Two rendering targets, one token system.** Define design tokens once in CSS
  (`:root` custom properties for radii scale, the 3-layer shadow + hover shadow,
  durations, easings, outline colors) and mirror them in a new
  `gui/DesignTokens.swift` for the AppKit layer, so the popover, status window,
  dialogs, and menu-bar animation match the renderer numerically.
- **Translate, don't import.** The skill's examples are Tailwind/Framer-Motion;
  the renderer is plain CSS/JS with no motion library. Use the skill's documented
  CSS-only fallbacks: layered `box-shadow` variables, `nth-child` stagger
  keyframes, and the two-icons-in-DOM cross-fade with `cubic-bezier(0.2,0,0,1)`.
  Do **not** add a JS animation dependency.
- **AppKit mapping.** Press scale → `CALayer` transform / `NSButton` highlight;
  concentric radii → `layer.cornerRadius`; shadows → `NSShadow`/layer shadow;
  tabular figures → `monospacedDigitSystemFont`; contextual glyph swaps →
  `CABasicAnimation` with `bounce 0`. SF Pro is already antialiased natively, so
  REQ-109 is renderer-only.
- **Surfaces in scope (~8):** (1) menu-bar status menu/popover, (2) desktop status
  window chrome, (3) Library list + cards, (4) the reader (PRD-1), (5) Settings
  (PRD-1 redesign), (6) onboarding (PRD-1), (7) dialogs/notifications, (8) menu-bar
  capture icon + state animation. The animated backgrounds
  ([`FluidBackground.swift`](../gui/FluidBackground.swift) + renderer aurora/
  dither/glass/flow) are reviewed only for the performance constraints (no
  `transition: all`, `will-change` discipline), not restyled.

### Tech Stack

- WKWebView HTML/CSS/JS renderer (no framework); Swift/AppKit.
- New: a renderer visual-regression check (snapshot or checklist-driven) and CI
  `grep` guards for `transition: all` and `will-change: all`.

### Integrations & External Dependencies

- None new at runtime. The `make-interfaces-feel-better` skill is a build-time
  reference, not a shipped dependency. No motion library is added.

---

## Implementation Roadmap

All on branch `feature/desktop-app-polish` (from `feature/desktop-app-v1`).

- **Phase 0 — Tokens & audit:** REQ-118 token system (CSS + `DesignTokens.swift`);
  run the 14-point checklist against all 8 surfaces and record baselines in
  `docs/polish-audit.md`; add CI `grep` guards (REQ-114/REQ-117).
- **Phase 1 — Surfaces & depth:** REQ-101 concentric radii, REQ-103 shadow-as-
  border, REQ-111 image outlines, REQ-102 optical alignment.
- **Phase 2 — Interaction:** REQ-108 press scale, REQ-112 hover/selection, REQ-107
  icon animations, REQ-104 interruptible transitions, REQ-116 hit areas.
- **Phase 3 — Motion & type:** REQ-105 staggered enters, REQ-106 subtle exits,
  REQ-113 first-paint guard, REQ-109 smoothing, REQ-110 tabular nums, REQ-115 text
  wrapping, `prefers-reduced-motion` paths.
- **Phase 4 — Verify:** complete before/after tables, frame-rate capture, full
  regression run, reviewer sign-off.

---

## Out of Scope

- Any layout, navigation, copy, feature, or product-behavior change (that is PRD 1
  and future work). This pass is rendering/micro-interaction only.
- Rebranding or new color palettes; the existing brand colors are fixed.
- Restyling the animated background shaders themselves (only their performance
  characteristics are checked).
- Adding a JS animation/motion library (Framer Motion / `motion`) — the renderer
  stays dependency-free using CSS fallbacks.
- Dark-mode design system work beyond applying the skill's dark-mode shadow/
  outline values where a dark surface already exists.
- iOS / multi-device / cross-browser polish (no such surfaces in this app).

---

## Open Questions & Risks

- **Reduced-motion coverage (Risk: low).** Every animation REQ must ship a
  `prefers-reduced-motion` path; risk is forgetting one — mitigated by the audit
  table requiring a reduced-motion row per surface.
- **Native/renderer numeric parity (Risk: medium).** Keeping `DesignTokens.swift`
  in sync with CSS tokens by hand can drift; consider generating one from the
  other, or a test asserting the key values match.
- **WKWebView vs. AppKit feel mismatch (Open).** Press/hover timing may need
  slight per-layer tuning to feel identical across the web and native surfaces.
- **Frame-rate measurement method (Open).** Decide the capture tool/threshold for
  the ≥58fps target on the reference Mac.
- **Tabular-nums font caveat (Risk: low).** Verify the renderer font's `1` width
  change reads well before rolling out broadly.

---

## Validation Checkpoints

- **Checkpoint A (end Phase 0):** Token system committed; `docs/polish-audit.md`
  baseline complete for all 8 surfaces; CI guards for `transition: all` and
  `will-change: all` active and currently counting the baseline.
- **Checkpoint B (end Phase 1):** Concentric-radius, shadow-as-border, image-
  outline, and optical-alignment tables show ≥95% applicable items fixed; brand
  colors unchanged.
- **Checkpoint C (end Phase 2):** Press scale exactly `0.96`, hover <100ms, icon
  animations at exact skill values, 0 interactive keyframes remaining, all hit
  areas ≥40×40px and non-overlapping.
- **Checkpoint D (end Phase 3):** Staggered enters/subtle exits applied, root font
  smoothing on, tabular-nums on all dynamic numbers, text-wrap rules applied,
  reduced-motion paths verified.
- **Checkpoint E (end Phase 4):** ≥58fps sustained on all interaction animations; 0
  `transition: all` / `will-change: all`; 0 behavior regressions (full suite + QA
  script identical); before/after tables complete; PR approved.

---

## Appendix: Task Breakdown Hints

Rough estimates aggregate to ~75 engineering hours.

- Phase 0 tokens + audit + CI guards: ~12h
- Phase 1 surfaces/depth (REQ-101/103/111/102): ~19h
- Phase 2 interaction (REQ-108/112/107/104/116): ~22h
- Phase 3 motion/type + reduced-motion (REQ-105/106/113/109/110/115): ~16h
- Phase 4 verification + tables + frame capture: ~6h

**Output format (mandatory, per the skill):** present every change as a markdown
before/after table grouped by principle, in `docs/polish-audit.md` — one row per
diff, citing the file and the specific property changed. Omit a principle's table
entirely if nothing needed changing on that surface. Do not list changes as loose
"Before:/After:" lines outside a table.

**Per-surface checklist (run for each of the 8 surfaces):** concentric radius ·
optical icon alignment · shadows-over-borders · split/staggered enters · subtle
exits · tabular-nums on dynamic numbers · root font smoothing · `text-wrap:
balance` on headings · image outlines · scale-on-press · first-paint animation
guard · no `transition: all` · `will-change` discipline · ≥40×40px hit areas.
