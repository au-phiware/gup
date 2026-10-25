---
implementer: sonnet
implementer-reason:
  a Theme preset with every colour chosen below and checked by contrast ratios,
  plus a theme palette the default categorical scale follows
touches:
  [
    crates/gup-core/src/theme.rs,
    crates/gup-core/src/scale/categorical.rs,
    crates/gup-core/src/lib.rs,
  ]
---

# GUP-553: A Dark Theme

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 🚧 In
Progress **Created**: 2026-10-24

## Context

`Theme::default()` is the only preset, light on white. Dashboards, IDE panels,
egui and Bevy apps are often dark, and a white chart in a dark app is glaring.
The October review's T4b asked for light and dark presets; the November review
lists "a dark theme" in N2. A user can build one by struct update today
(`Theme { background: .., ..Theme::default() }`), but picking a dozen colours
that work together is exactly the work a library should do once.

One colour is not in `Theme`: the categorical palette.
`Categorical::okabe_ito()` ends with black (`OKABE_ITO[7]`), invisible on a dark
background, and its generated colours (keys past 8) are chosen far from white
"the background" (`scale/categorical.rs`). The theme already reaches scales
through `ShaderFn::apply_theme` (the null colour), so the palette can follow the
theme the same way.

## Read First

- `crates/gup-core/src/theme.rs` (whole).
- `crates/gup-core/src/scale/categorical.rs` lines 1–200 (palette, LUT,
  generation) and 280–300 (`apply_theme`).
- `crates/gup-core/tests/theme_png.rs` (a theme golden).
- `crates/gup-core/src/guide.rs` lines 1–60 and `tooltip.rs` lines 1–80 (which
  theme fields legends and tooltips use).
- GUP-552's Decisions (subtitle fields), if merged.

## User Story

> "As a developer of a dark-mode app, I want `Theme::dark()`, so that my charts
> match the app without hand-picking colours."

## Acceptance Criteria

### AC1: The preset

- [x] `Theme::dark()` and `Theme::light()` (equal to `Theme::default()`) exist,
      documented, with the values in Decisions; a doctest from the `gup` crate
      sets `.theme(Theme::dark())` on a builder.
- [x] Unit tests compute WCAG 2 contrast ratios against `background` and assert:
      `label` ≥ 7, `title` ≥ 12, `axis` ≥ 4.5, `subtitle` (if GUP-552 has
      merged) ≥ 4.5, `grid` within 1.2–1.6, `tooltip_text` against
      `tooltip_background` ≥ 7, and every `palette` colour ≥ 3.

### AC2: The palette follows the theme

- [x] `Theme` gains `palette: [Color; 8]`, `OKABE_ITO` for light and
      `OKABE_ITO_DARK` (Decisions) for dark; `OKABE_ITO_DARK` is a public
      constant beside `OKABE_ITO`.
- [x] `Categorical::okabe_ito()` draws the theme's palette, its colours past 8
      generated against the theme's background; `Categorical::from_palette`
      ignores the theme (tests for both).
- [x] Applying the same theme again does not rebuild the LUT (test: a second
      resolve with nothing changed is a cache hit with no texture writes in
      `Context::upload_stats`).
- [x] Every existing golden is unchanged (the light palette equals today's).

### AC3: Visual

- [x] Golden `gup_core/theme_dark` (640×400): a scatter of 9 categories (so the
      9th, generated colour shows) with legend, `grid(Axes::Both)`, a title, a
      hovered point's tooltip (`Input::PointerMoved` before rendering) and a
      brushed region in a second golden `gup_core/theme_dark_brush`. The worker
      reads both: every series visible, text legible, grid subtle, tooltip and
      brush styled dark.

## Decisions

- **Values** of `Theme::dark()` (sizes and spacing as default):
  `background #1E1E22`, `axis #A0A0A8`, `grid #34343A`, `label #C8C8D0`,
  `title #F0F0F4`, `subtitle #A8A8B0` (only if GUP-552 has merged; otherwise
  GUP-552 adds it), `null_color #6E6E78`, `tooltip_background #2A2A30`,
  `tooltip_border #5A5A64`, `tooltip_text #E6E6EA`, `brush_fill #8AB4F8` at
  alpha 0.2, `brush_stroke #8AB4F8`, `unbrushed Fade(0.3)`,
  `palette OKABE_ITO_DARK`.
- **`OKABE_ITO_DARK`** is `OKABE_ITO` with black replaced by `#E8E8E8`. The
  other seven Okabe-Ito colours keep their colour-blind safety and all reach 3:1
  on `#1E1E22` (the lowest, blue `#0072B2`, is 3.2:1).
- **Mark defaults stay**: `Circle::DEFAULT_FILL` and the other marks' default
  `#0072B2` are constants in the mark's channel table and are not themed; at
  3.2:1 they meet the 3:1 non-text contrast guideline on the dark background.
  Documented on `Theme::dark`.
- **LUT rebuild**: `Categorical` remembers the `(palette, background)` its LUT
  was built for and rebuilds in `apply_theme` only when they differ; the light
  theme keeps today's static LUT.
- **Sequential and diverging palettes are not themed** (viridis' dark end is
  low-contrast on dark backgrounds; documented as a known limit).

## Escalation Triggers

- A contrast test fails for a value in Decisions: report the ratio; do not
  change the colour without the orchestrator (the values were chosen by computed
  ratios).
- Rebuilding the LUT in `apply_theme` writes a texture on every resolve.
- Any light-theme golden changes.

## Out of Scope

- High-contrast themes (track N4); themed sequential palettes; automatic dark
  mode from the host.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `palette`, `OKABE_ITO_DARK`, the categorical change; light goldens green;
      commit.
- [x] `Theme::dark`/`light`, contrast tests; commit.
- [x] The two goldens; docs; commit.

## Dependencies

### Prerequisite Stories

- GUP-552: subtitle 📋 (soft): sets the subtitle colour if merged first; both
  edit `theme.rs`, so they never run together.

### Enables Stories

- GUP-536: the chart tour shows one chart in the dark theme.

## Testing Strategy

- **Unit tests**: contrast ratios; palette selection; LUT reuse.
- **Visual validation**: the two goldens, read by eye.

## Success Metrics

- [x] `.theme(Theme::dark())` gives a legible, balanced chart with every element
      styled.

## Risk Assessment

- **Low**: a `Theme` field addition breaks users' exhaustive struct literals.
  _Mitigation_: breaking changes are accepted pre-0.1; every in-repo literal
  uses `..Theme::default()`.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the two goldens)
- [x] `mask wasm-browser`: not needed, no layout or input change
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `Theme::dark()`, `Theme::light()`, `Theme::palette: [Color; 8]`, public
  `OKABE_ITO_DARK` (re-exported from `gup_core` and `gup`).
- `Categorical::okabe_ito()` remembers the `(palette, background)` its LUT was
  built for and rebuilds in `apply_theme` only when they differ. `from_palette`
  ignores the theme. The light theme keeps the static LUT.
- Generated colours (keys past 8) on a dark background (luminance < 0.18) must
  reach 3:1 contrast against it; a light background keeps every candidate, so no
  light colour or golden changed.
- Tests: contrast and palette unit tests (`theme.rs`), `categorical.rs` theme
  test, `tests/theme_dark_png.rs` (two goldens + no-texture-write test),
  doctests in `Theme::dark` and the `gup` crate docs.
- Goldens: `tests/golden/gup_core/theme_dark.png`, `theme_dark_brush.png`.

## Definition-of-Done evidence

- Contrast against `#1E1E22` (WCAG 2): label 9.99, title 14.62, axis 6.40, grid
  1.34, null 3.29, tooltip text 11.46. Palette: orange 7.38, sky 7.20, green
  4.86, blue 3.20, vermillion 4.30, purple 5.43, yellow 12.56, `#E8E8E8` 13.56.
  Every generated colour (keys 9-64) is at least 3:1 (test).
- `mask merge-check` PASS: story-ids 1 s, doc 24 s, gup-core 735 s, wasm-size 48
  s, wasm-browser 4 s (dogfood skipped). WASM scatter gz 338441 B (recorded
  337974, +0.1%), over wgpu 306081 B.
- `theme_dark`: nine series on near-black, the 9th (generated) a mid purple;
  title and ticks legible, grid barely visible, legend swatches and labels
  clear, tooltip a slightly lighter box with a grey border and light text.
- `theme_dark_brush`: unbrushed rows fade toward the background but stay
  visible, the brush is a translucent blue box with a light-blue border.
- Also rendered dark histogram (stacked), barh and line charts (scratch, not
  committed): all legible. The heatmap builder (GUP-549) is not in this branch's
  base; it uses `Sequential`, which is not themed (known limit).

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### Generated colours need a dark-background rule

- **Challenge**: with the background merely swapped for the dark one, the 9th
  colour was the same dark purple as on white (1.9:1 on `#1E1E22`), nearly
  invisible.
- **Solution**: on a dark background, generated candidates must reach 3:1; a
  light background is untouched, so light goldens and the pinned LUT entries are
  unchanged.
- **Pattern**: farthest-point generation in OKLab ignores legibility; add the
  contrast rule where the background can be dark.

#### Goldens with tooltips and brushes

- **Challenge**: the SVG parity check compares glyphs (tooltip text is not in
  `Layout::texts`) and cannot dim unbrushed rows; a tooltip past the plot rect
  fails `marks_confined`.
- **Solution**: the tooltip box is added as a text region; the brush golden
  skips the SVG check (follow-up GUP-524); the hovered row is mid-plot.
- **Pattern**: choose the hovered row so its tooltip stays inside the plot.

### Architectural Decisions

#### Cache key is `(palette, background)`

- **Decision**: as the story says; the generated colours also avoid
  `NULL_COLOR`, not the theme's `null_color`.
- **Trade-off**: dark `null_color` `#6E6E78` is not avoided by generation.
- **Future**: add `null_color` to the key if a generated colour ever clashes.

### Development Workflow Insights

- `scale/mod.rs` (outside `touches`, used by GUP-542) got one added
  `pub use categorical::OKABE_ITO_DARK;` line; the `gup` crate doctest went in
  `src/lib.rs` docs.
- Compound `cd && heredoc` commands are refused in a worktree; use Write/Edit.

### Follow-up Stories

1. **GUP-524: SVG Draws Unbrushed Rows Dimmed** — SVG parity for brushed charts.
