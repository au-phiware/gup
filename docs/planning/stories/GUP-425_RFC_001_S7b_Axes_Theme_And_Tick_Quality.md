# GUP-425: RFC-001 Step S7b: Axes, Theme and Tick Quality

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-11

## Context

`crates/gup-core/src/plot.rs`'s `style` module
(`crates/gup-core/src/plot.rs:26-39`) is explicitly provisional: "Fixed S0a
styling (a `Theme` is RFC-001 S7 / GUP-392)". It hard-codes colours
(`BACKGROUND`, `AXIS`, `LABEL`, `TITLE`), sizes and the x/y tick-spacing targets
as `const`s, and `Plot::resolve` draws exactly one title (`self.title`) with no
axis titles at all: no chart in `gup-core` has one today.
[RFC-001](../rfcs/RFC-001_Core_Architecture.md)'s S7 migration step asks for
"axes/grid/legend/title guides, `Theme` (Inter, Okabe-Ito, viridis)" and §12
risk-free design already specifies a grid `ItemKind::Rules` and a `Theme` type;
this story is the part of S7 that makes axes and ticks correct and gives
`gup-core` a real `Theme`, scoped apart from
[GUP-424](GUP-424_RFC_001_S7a_Multi_Layer_Plot_And_Shared_Scales.md)'s
structural multi-layer work (which this story depends on, since tick density and
domain niceness should account for every layer sharing an axis) and
[GUP-426](GUP-426_RFC_001_S7c_Legend_Placement.md)'s legend placement (a
separate guide this story does not place).

Four concrete gaps motivate this story, three from
[GUP-419](GUP-419_RFC_001_S5b_Colour_Scales_And_Dictionary_Hook.md)'s
retrospective and one from dogfooding the scale family in
[GUP-418](GUP-418_RFC_001_S5a_ShaderFn_V2_And_Numeric_Scales.md)/GUP-419's own
example charts:

- **No axis titles.** `Plot` has a chart title but no per-axis title (RFC-001
  §10's examples never label an axis beyond its tick values). The strategic
  review's T4b independently lists "Axis titles, left-aligned title and
  subtitle" as a visual-design gap.
- **`theme.null_color` does not exist.** `NULL_COLOR` is a literal constant used
  directly in the glue and in `Categorical::Params`
  (`crates/gup-core/src/scale/categorical.rs`), not a theme value a user can
  override.
- **Time-axis tick hierarchy.** GUP-418's "S5a findings" gave `Time` calendar
  ticks ("year ticks heavier than month ticks" per this story's brief) but every
  tick label renders at one weight; RFC-001's "S5b findings → Proposed
  adjustments" says this "needs a second weight in the bundled font" — a
  font-asset change, not just a layout one.
- **Band/categorical labels collide, and log domains waste space.** RFC-001's
  "S5b findings" records Band/Point's tick behaviour as "above twice the
  requested tick count, they keep every k-th key. Collision-aware labels are
  S7's." Separately, `categorical_species` (a GUP-419 golden) auto-domains a log
  scale from roughly 8 to 12,000 but the "niced" domain runs 1 to 100,000 — two
  wasted decades of plot area — because the log scale's `nice()` rounds to whole
  powers of ten rather than a tighter 1-2-5-style step.

## User Story

> "As a visualization developer, I want axis titles, a themeable null colour, a
> readable hierarchy between year and month ticks on a time axis, non-colliding
> category labels, and log domains that do not waste a third of the plot on
> empty space, so that a default `gup-core` chart looks finished without
> per-chart manual tuning."

## Acceptance Criteria

### AC1: Axis titles

- [ ] `Plot` (or `ScaleRef`/the axis configuration, whichever RFC-001 §9's
      `Layout` design implies) accepts an axis title per position scale (`x` and
      `y`), measured with `gup-text` like the chart title, and placed without
      overlapping tick labels — the plot rect shrinks to make room, the same way
      the chart title already does (`crates/gup-core/src/plot.rs:236-258`).
- [ ] A golden shows both an x-axis and a y-axis title alongside tick labels and
      the chart title, read by eye: all three are legible, none overlaps
      another, and the axis titles describe their axis (for example "GDP per
      capita" / "Life expectancy").
- [ ] Overflow: a very long axis title (longer than the plot width) is handled
      without panicking or drawing off-canvas — truncation, wrapping or a
      documented minimum-width behaviour, whichever this story implements; the
      chosen behaviour is demonstrated in a test.

### AC2: `Theme` with `null_color`

- [ ] A `Theme` type exists (even if minimal in this story — a fuller palette
      system can still arrive incrementally) holding at least
      `null_color:     Color`, the font choice already bundled (Inter), and the
      categorical default (Okabe-Ito) — these are read from `Theme` rather than
      the literal `NULL_COLOR` constant and the hard-coded `style` module
      values.
- [ ] `Categorical::Params` and the glue read `theme.null_color` instead of the
      `NULL_COLOR` literal; a test overrides the theme's null colour and
      confirms a null-key row draws in the overridden colour, not the old
      literal.
- [ ] `Plot::theme(&mut self, theme: Theme)` (or equivalent) lets a caller
      override the default; a default `Theme` reproduces today's hard-coded
      `style` module values exactly (no visual regression in any existing
      golden).

### AC3: Time-axis tick hierarchy

- [ ] The bundled Inter subset gains a second weight (for example Inter Medium
      or Semibold alongside the existing Regular), subset to the same character
      set `gup-text`'s font sweep already covers.
- [ ] `Time` axis tick labels render a year tick (the coarsest unit a tick
      begins, per GUP-418's "S5a findings" labelling rule) in the heavier weight
      and every other tick (month, day, etc.) in the regular weight.
- [ ] A golden (`gup_core/time_tick_hierarchy` or a re-blessed `time_years`)
      shows, read by eye, a visibly heavier year label among lighter sub-year
      ticks.
- [ ] WASM size: the second font weight's subset is measured against
      `PERF_BUDGETS.md`'s existing WASM-size ceiling (400,000 B gz over bare
      wgpu per RFC-001 §12 risk 10); record the new total.

### AC4: Collision-aware Band/Point labels

- [ ] Band/Point tick labels no longer use the "keep every k-th key" fallback
      when that fallback would still produce overlapping text at the resolved
      font size; instead, labels are thinned (or rotated, or abbreviated —
      document the chosen strategy) based on their actual measured width versus
      the available per-key pixel span, using `gup-text`'s measurement API
      already used for margins.
- [ ] A test with many short keys (where every-k-th works) and one with few long
      keys (where every-k-th alone would still overlap, for example
      `band_months`'s full month names narrowed to a tiny plot width) both
      produce non-overlapping labels, confirmed by comparing each label's
      measured ink box against its neighbours' — not merely "fewer labels than
      keys".

### AC5: Tighter log-domain niceness

- [ ] A log scale's automatic `nice()` domain no longer always rounds to the
      nearest whole power of ten on each end; it uses a tighter step (for
      example 1-2-5-style boundaries, matching the convention linear `nice()`
      already uses for its step choice) so a domain spanning roughly one and a
      half decades (like `categorical_species`'s ~8 to ~12,000) does not round
      out to two full wasted decades (1 to 100,000).
- [ ] `categorical_species` (or its log-axis equivalent) is re-rendered and,
      read by eye, uses noticeably more of the plot's vertical or horizontal
      extent for data than before; the golden is re-blessed with a recorded
      before/after description.
- [ ] A unit test asserts the niced domain for a few representative data ranges
      (an exact decade, a sub-decade span, a multi-decade span) matches the new,
      tighter rule, not the old whole-decade rule.

## Technical Tasks

- [ ] Design and implement `Theme` (`crates/gup-core/src/theme.rs` or similar):
      fields for `null_color`, categorical default palette reference, font
      family/weights; a `Default` impl matching today's `style` module values.
- [ ] Thread `Theme` through `Plot` (a `theme()` setter) and into
      `Categorical`'s params/glue (replacing the `NULL_COLOR` literal) and into
      `plot.rs`'s guide drawing (replacing the `style` module's hard-coded
      colours where a theme value now exists).
- [ ] Add axis-title support to `Plot`'s builder API and `resolve`'s margin
      computation.
- [ ] Source and subset a second Inter weight; extend the font sweep/embedding
      tooling (GUP-407's subsetting) to include it; wire tick-label text runs to
      pick the weight based on the `Time` scale's per-tick unit.
- [ ] Implement collision-aware label thinning for `Band`/`Point` using
      `gup-text` measurements against each key's allotted pixel span.
- [ ] Revise the log scale's `nice()` domain-rounding rule to a tighter step;
      add the representative-range unit test.
- [ ] Re-bless affected goldens (`categorical_species`, `time_years` or its
      replacement, `band_months` if label density changes it).

## Dependencies

### Prerequisite Stories

- GUP-424: RFC-001 S7a Multi-Layer Plot and Shared Scales 📋 — tick density and
  domain niceness should be computed against every layer sharing an axis, not
  just the first; this story's AC4/AC5 are most meaningful once GUP-424's domain
  union lands.
- GUP-419: RFC-001 S5b Colour Scales and Dictionary Hook ✅ — `Categorical`'s
  `NULL_COLOR` literal (AC2) and Band/Point's tick behaviour (AC4) this story
  revises.
- GUP-418: RFC-001 S5a ShaderFn v2 and Numeric Scales ✅ — `Time`'s calendar
  tick labelling rule (AC3) and the log scale (AC5) this story builds on.
- GUP-407: Subset Inter and SVG Font Embedding ✅ — the font-subsetting tooling
  AC3's second weight extends.

### Enables Stories

- GUP-426: RFC-001 S7c Legend Placement — a `Theme` to read default colours
  from, and a settled axis-title/margin computation to place a legend alongside.
- RFC-001 S10 (builders; not yet written as a story) — axis titles and a `Theme`
  are part of the "5-10 lines" north-star experience every builder needs.

## Testing Strategy

- **Unit tests**: `Theme` default equals today's hard-coded values; log `nice()`
  domain for representative ranges; collision-aware label thinning against
  measured ink boxes.
- **Integration tests**: axis-title overflow handling; time-tick-hierarchy
  weight selection by unit.
- **Visual validation**: axis-title golden, `time_tick_hierarchy` (or re-blessed
  `time_years`), re-blessed `categorical_species` and `band_months` — all read
  by eye with before/after notes in the retrospective.
- **Performance**: WASM size recorded against the 400,000 B gz ceiling after
  adding the second font weight (AC3).

## Success Metrics

- [ ] Axis titles render without overlapping other guides in every affected
      golden, read by eye.
- [ ] `categorical_species`'s log axis visibly uses more of the plot for data
      after the niceness fix.
- [ ] The second font weight stays within the WASM-size budget, with the new
      total recorded in `PERF_BUDGETS.md`.
- [ ] No existing golden regresses unintentionally; any that change are
      re-blessed with a documented reason.

## Risk Assessment

- **Medium**: collision-aware label thinning (AC4) has several reasonable
  strategies (drop labels, rotate, abbreviate) with different trade-offs for
  readability and implementation cost. _Mitigation_: start with drop/thin
  (extending the existing every-k-th mechanism to be measurement-driven rather
  than count-driven) before considering rotation, which interacts with margin
  computation more invasively.
- **Medium**: a tighter log `nice()` rule changes every existing log-axis
  golden's domain, which may cascade into other visual changes (tick count,
  label width, margins). _Mitigation_: re-bless deliberately and read every
  affected golden by eye before committing; do not blindly accept a re-blessed
  image.
- **Low**: adding a second font weight touches the WASM-size budget GUP-418/
  GUP-419 were careful to stay under; a careless full-weight embed (rather than
  a subset matching the existing character-set sweep) could blow it.
  _Mitigation_: reuse GUP-407's subsetting tooling exactly, scoped to the same
  character set already swept for Regular.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read)
- [ ] `mask wasm-browser` run if the font/WASM size change affects it
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
