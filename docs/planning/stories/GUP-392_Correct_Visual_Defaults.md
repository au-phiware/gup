# GUP-392: Correct Visual Defaults

## Story Overview

**Initiative**: Strategic Review 2026-10 **Status**: 📋 Planned **Created**:
2026-10-04 **Revised**: 2026-10-04

**Revised 2026-10-04**: trimmed to the subset of T4a visual-defaults work that
survives [RFC-001](../rfcs/RFC-001_Core_Architecture.md) (Core Architecture,
accepted) and its new `crates/gup-core` path. The colour-pipeline linearisation
work originally proposed here (sRGB→linear conversion ahead of `*Srgb` render
targets) is **removed**: RFC-001 §7 adopts an sRGB-space colour policy for
`gup-core` that replaces this story's approach, so implementing a separate
linearisation fix on the old `gup` path would be thrown away. The remaining
scope below — a bundled font asset, a single palette data source, tick-precision
logic, and muted theme colour _values_ — are reusable inputs (assets and pure
data/algorithms) that `gup-core` can consume directly, not old-path-only
plumbing, which is why they remain in scope despite the old `gup` crate path
being otherwise frozen for feature work ahead of the RFC-001 S14 flip.

## Context

Verified against `main` on 2026-10-04:

- **Three copy-pasted categorical palettes**, each with the identical 8-colour
  Category10-style array, confirmed via
  `grep -n "0.122, 0.467, 0.706" src/chart_builder/builders/*.rs`:
  `src/chart_builder/builders/bar.rs:118` (`DEFAULT_PALETTE`, marked
  `#[allow(dead_code)]` and already removed by GUP-389), `line.rs:41`, and
  `area.rs:54`. **Correction to the review's "bars always use colour 0" claim**:
  that dead `DEFAULT_PALETTE`/ `accessor_to_color` pair in `bar.rs`
  (`bar.rs:767-773`) has zero callers — the actually-live default-colour path is
  the hardcoded steel-blue fallback in `apply_accessors_to_selection()`
  (`src/chart_builder/builders.rs:736`) plus `AccessorValue::as_color()` mapping
  any `String` category value to flat grey
  (`src/chart_builder/accessor.rs:235-240`), matching the dogfood audit's own
  finding that `.color(String)` renders grey. This story produces a single
  **palette data source**; wiring it into the old path's
  `apply_accessors_to_selection()` is in scope only as a minimal fix enabling
  AC2's test, not a broader old-path colour-pipeline rewrite.
- **Font**: the single embedded fallback font is `assets/fonts/default.ttf`,
  loaded by `FontSource::embedded_fallback()` (`src/text/font.rs:332-339`) and
  labelled `"Squada One"` — a display/headline face. Six test assertions in
  `src/text/font.rs` and `src/text/atlas.rs` check the literal string
  `"Squada One"` and must be updated alongside the font swap.
- **Tick precision**: `NumericFormatter::default()`
  (`src/label/formatter.rs:28-36`) hardcodes `precision: 2`, and
  `src/axis.rs:1180` constructs the default axis formatter via
  `NumericFormatter::default()`, so ticks always show "0.00, 2.50, 5.00"
  regardless of the actual tick step. Per
  [Decision 6](../STRATEGIC_REVIEW_2026-10.md#decisions)-adjacent RFC-001
  framing, the tick-generation algorithms become the scales' CPU mirror in
  `gup-core` — the step-aware precision logic this story writes should be
  implemented as a reusable pure function (step → decimal places), not
  axis-specific code, so it ports directly.
- **Muted theme values**: default axis line colour, gridline colour, and
  box-plot stroke/fill colour are currently full-saturation/near-black
  (`AxisConfiguration::default()` in `src/axis.rs:149`,
  `GridConfiguration::default()` in `src/grid.rs:398`). This story adjusts the
  concrete default colour _values_; it does not build the `Theme` type itself
  (T4b / `gup-core`'s own theme design, out of scope here).

## User Story

> "As a user who calls `gup::scatter(&data)` and saves a PNG without configuring
> colours, I want categorical series to get visibly distinct, colour-blind-safe
> colours, axis tick labels to show only the precision the data needs, chart
> text to use a readable UI font, and axes/gridlines to look muted rather than
> harsh — all without configuring a theme, and with the underlying font asset,
> palette data, and tick-precision logic reusable by `gup-core`."

## Acceptance Criteria

### AC1: Single Okabe-Ito categorical palette (data source)

- [ ] One palette module/constant (not three copies) defines the default
      categorical palette, using the
      [Okabe-Ito](https://jfly.uni-koeln.de/color/) colour-blind-safe set per
      [Decision 4](../STRATEGIC_REVIEW_2026-10.md#decisions), as plain data
      (e.g. `pub const OKABE_ITO: [[f32; 4]; N]`) with no render-target-specific
      logic attached, so `gup-core` can import the same constant.
- [ ] `src/chart_builder/builders/line.rs` and `area.rs` use this single source;
      their duplicated 8-colour arrays are removed.
- [ ] `apply_accessors_to_selection()` (`builders.rs:736`) and
      `AccessorValue::as_color()` (`accessor.rs:235-240`) are updated so a
      `String` category value maps to a palette colour by stable index (hash or
      first-seen order), and the no-accessor-configured default pulls from
      palette index 0, instead of flat grey / hardcoded steel-blue.
- [ ] A test with 3+ distinct string category values asserts 3+ visually
      distinct colours at the expected mark locations.

### AC2: Inter as the default font

> **Note (2026-10-06, GUP-400):** the font item is done for the surviving path.
> [GUP-400](GUP-400_RFC_001_S2_Extract_Gup_Text.md) vendored Inter Regular 4.1
> as `crates/gup-text/fonts/Inter-Regular.ttf`, with its SIL OFL 1.1 licence as
> `crates/gup-text/fonts/Inter-OFL.txt`. `gup-text` and `gup-core` use it by
> default, and the `gup_core` scatter golden was re-blessed with it. GUP-400
> left `assets/fonts/default.ttf` (Squada One) unchanged, so the frozen old path
> and its goldens did not change. If this story still swaps the old path's font,
> it should `include_bytes!` the vendored file rather than add a second copy or
> a second licence. Tabular figures need OpenType feature support (`tnum`),
> which `gup-text` lacks (see GUP-400's follow-ups).

- [ ] The embedded fallback font (`assets/fonts/default.ttf`) is replaced with
      [Inter](https://github.com/rsms/inter) (SIL OFL 1.1), a static weight
      appropriate for UI text (e.g. Regular) with tabular figures available.
- [ ] `FontSource::embedded_fallback()` reports `"Inter"`; all six
      `"Squada One"` assertions in `src/text/font.rs`/`atlas.rs` are updated.
- [ ] SVG export's default `font-family` (`src/export/svg/element.rs:233`) is
      updated to match.
- [ ] An SIL OFL 1.1 licence file for Inter is added under `assets/fonts/` or a
      top-level `LICENSES/` directory, with attribution.
- [ ] A rendered PNG with default-font title/tick-label text is visually
      inspected (attached/described in Definition-of-Done evidence) to confirm
      correct rendering (no missing glyphs for the ASCII range used by axis
      labels).

### AC3: Tick precision derived from tick step

- [ ] A pure function `decimal_places_for_step(step: f64) -> usize` (or
      equivalent) computes the minimum decimal places needed to distinguish a
      tick sequence generated at that step, independent of any axis/formatter
      type, so it can be reused by `gup-core`'s scale CPU mirror.
- [ ] The old path's default axis tick formatter construction
      (`src/axis.rs:1180`) uses this function instead of a fixed `precision: 2`.
- [ ] A test with tick values `[0.0, 2.5, 5.0, 7.5, 10.0]` (step `2.5`) asserts
      labels `"0", "2.5", "5", "7.5", "10"`, not `"0.00", "2.50", ...`.
- [ ] A test with a fractional step (e.g. `0.25`) still shows the necessary
      decimal places.

### AC4: Muted theme colour values

- [ ] Default axis line colour, gridline colour, and box-plot stroke/fill colour
      values in `AxisConfiguration::default()` (`axis.rs:149`),
      `GridConfiguration::default()` (`grid.rs:398`), and
      `src/chart_builder/builders/boxplot.rs`'s defaults read as muted/neutral
      (e.g. mid-greys) rather than full-saturation or near-black.
- [ ] Before/after rendered PNGs of a chart with default axes/gridlines are
      attached or described in the Definition-of-Done evidence.

## Technical Tasks

- [ ] Create the single Okabe-Ito palette constant/module as plain data.
- [ ] Update `apply_accessors_to_selection()` and `AccessorValue::as_color()`
      per AC1; remove the duplicated arrays in `line.rs`/`area.rs`.
- [ ] Vendor Inter (static weight(s) actually used) under `assets/fonts/`, add
      its OFL licence file, update `FontSource::embedded_fallback()` and the six
      test assertions, and the SVG default `font-family`.
- [ ] Implement `decimal_places_for_step` as a standalone pure function; wire it
      into the old path's default axis formatter construction.
- [ ] Adjust the muted default colour values in `AxisConfiguration::default()`,
      `GridConfiguration::default()`, and boxplot defaults.
- [ ] Bless/update any golden images in GUP-388's harness affected by these
      intentional visual changes.

## Dependencies

### Prerequisite Stories

- GUP-388 (Visual regression harness) ✅ — needed to bless the new golden images
  these intentional visual changes produce.

### Enables Stories

- Feeds `gup-core`'s theme/scale work (RFC-001, out of scope for this story)
  with a ready-made palette constant, font asset + licence, and a reusable
  tick-precision function.

## Testing Strategy

- **Unit tests**: `decimal_places_for_step` for a range of step values (1, 2.5,
  5, 0.25, 0.1, 100); palette index lookup for repeated/new category strings.
- **Pixel tests**: multi-category distinct-colour test (AC1).
- **Visual validation**: default-font rendering (AC2) and muted-defaults
  rendering (AC4) are visually inspected by the implementer, not just
  pixel-counted.

## Success Metrics

- [ ] Three distinct string category values render as three distinct colours by
      default.
- [ ] Default tick labels for common steps (whole numbers, halves, quarters)
      show no unnecessary trailing zeros.
- [ ] Inter renders correctly with no missing glyphs for ASCII tick
      labels/titles.
- [ ] Palette constant, font asset, and tick-precision function are each
      self-contained enough to import into `gup-core` without old-path
      dependencies.

## Risk Assessment

- **Low**: These are old-path visual-default value/asset changes with no
  architectural coupling to the frozen render pipeline, so they carry lower risk
  of wasted work than the removed colour-linearisation scope or GUP-393's
  mark-geometry fixes.
- **Low**: Choosing which Inter static weight(s) to vendor affects embedded font
  file size; match the current single-weight embedding pattern.
- **Low**: Tick-precision-from-step logic has edge cases (floating-point step
  values); round the step to a small number of significant figures before
  computing required decimal places, and test against real tick-generation
  output (`src/tick_generator.rs`).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] Lint and format clean: `mask all-fix`.
- [ ] All examples compile: `cargo check --examples`.
- [ ] Golden images in GUP-388's harness re-blessed with before/after PNGs
      described in the completion evidence.
- [ ] Story status updated to ✅ Complete in story file and INDEX.md.
- [ ] Retrospective added to story document.
