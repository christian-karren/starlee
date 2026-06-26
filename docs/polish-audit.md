# Desktop polish audit — make-interfaces-feel-better

Pass over the WKWebView renderer surfaces built in PRD 1 (Library cards, reader,
filter, onboarding, settings). Display/micro-interaction only — **no product
behavior, layout, copy, or brand-color changes** (verified: only
`gui/Resources/renderer/styles.css` changed; no JS or Swift logic touched; 107
Rust tests unaffected; the app's open/close/filter/edit/upload/onboarding flows
behave identically).

Brand palette preserved: navy `#13284B`, cream `#F2E3B6`/`#F9E4B6`, black/white.
The brutalist hard offset shadows (`6px 6px 0`) are a deliberate brand choice and
were kept; the skill's soft `--shadow-border` token is added to `:root` for
future use but not forced over the brand aesthetic.

## Design tokens
| Before | After |
| --- | --- |
| timings/easing/shadows inlined per rule | `:root` tokens `--dur-hover/press/enter`, `--ease-out` (`cubic-bezier(0.2,0,0,1)`), `--shadow-border[-hover]` |

## Font smoothing
| Before | After |
| --- | --- |
| no `-webkit-font-smoothing` | `-webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale` applied once on `html, body, .root` |

## Tabular numbers
| Before | After |
| --- | --- |
| card `<time>` proportional digits | added `font-variant-numeric: tabular-nums` to both `time` rules (card dates + mono) |
| (already done in PRD 1) | `.filter-count`, `.reader-meta` already tabular |

## Text wrapping
| Before | After |
| --- | --- |
| `h1` default wrap | `h1 { text-wrap: balance }` (reader/onboarding/section titles) |
| (already done in PRD 1) | `.reader-titles h2` balance; `.reader-body`, `.onb-lead/.onb-sub` pretty |

## Hit areas (≥40×40 where safe)
| Before | After |
| --- | --- |
| `.card-delete` 30×30 | `::after` extends the target to 44×44 (overlaps only its own card, which ignores clicks in edit mode) |
| `.topic-remove` 15×15 | `::after` extends to 26×26 (kept modest to avoid colliding with adjacent chips, per the skill's collision rule) |

## Press / interruptible transitions
| Before | After |
| --- | --- |
| `.reader-close`, `.topic-chip` no press feedback | property-specific interruptible transition; `.reader-close:active { scale(0.96) }` (exact skill value) |
| brutalist buttons (`.action-btn`, `.onb-primary`, …) | kept their translate-shadow press — on-brand and already interruptible |

## Enter animations (split + stagger)
| Before | After |
| --- | --- |
| reader appeared instantly | `.reader.open .reader-panel > *` staggered `polishFadeInUp` (opacity + `translateY(10px→0)` + `blur(4px→0)`), 0/70/140ms across head→actions→body |
| onboarding step appeared instantly | `.onb-step:not([hidden]) > *` staggered 0/60/120/180/220ms; each step animates in as it becomes visible |
| — | one-shot keyframes (correct for discrete entrances); guarded by `@media (prefers-reduced-motion: reduce)` |

## Performance / discipline (verified clean)
| Check | Result |
| --- | --- |
| `transition: all` | 0 occurrences (all transitions are property-specific) |
| `will-change` | 0 (none added — no first-frame stutter observed; reserved for need) |
| animated properties | only `opacity` / `transform` / `filter` (GPU-compositable) |

## Concentric radius (audited, compliant)
No violations requiring change: nested surfaces (reader body in panel, onboarding
browser buttons in panel, settings cards) sit behind ≥26px padding, which the
skill treats as independent surfaces (>24px rule) rather than strict
`outer = inner + padding`. Card depth is shadow-based, not bordered.

## Not in scope this pass
- Native AppKit layer (menu-bar popover, status-window chrome, dialogs, menu-bar
  capture icon) — would need `DesignTokens.swift` + `CABasicAnimation` work;
  follow-up.
- Animated-background shaders — reviewed only for the performance constraints
  above (no `transition: all`, compositable properties); not restyled.
