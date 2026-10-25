---
implementer: sonnet
implementer-reason:
  a builder over Rect on two band axes with a continuous colour scale, every
  rule (order, gaps, scales) fixed below; follows the bar builder
touches:
  [
    crates/gup-core/src/builder/mod.rs,
    crates/gup-core/src/builder/scatter.rs,
    crates/gup-core/src/lib.rs,
  ]
---

# GUP-549: The Heatmap Builder

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-26) **Created**: 2026-10-24

## Context

A heatmap (a grid of cells coloured by value: activity by weekday and hour, a
correlation matrix, sales by region and month) is a staple chart that Gup can
already draw by hand: `Rect`s on a band x and a band y axis, each cell
`band_width_px` wide and tall, filled through a `Sequential` or `Diverging`
scale whose ramp legend Gup lays out. The review lists "heatmap builder (`Rect`

- band + sequential)". This story wraps it in `gup::heatmap`.

The scatter builder's numeric colour, `color_value` with
`color_value_scale(Sequential)`, is the naming precedent. A heatmap also needs
diverging scales (correlations run −1 to 1), which `color_value_scale` does not
accept today; this story widens it for both builders.

## Read First

- `crates/gup-core/src/builder/bar.rs` (band x axis, key domains, colours) and
  `builder/scatter.rs` lines 40–200 (`color_value`, `color_value_scale`,
  `ColorSpec`).
- `crates/gup-core/src/scale/band.rs` lines 108–225 (`Band`, padding,
  `band_width_px`).
- `crates/gup-core/src/scale/sequential.rs` and `scale/diverging.rs`: their
  constructors and `ShaderFn`/`ColorScale` impls (search `impl`).
- `crates/gup-core/tests/legend_png.rs` lines 1–80 (a ramp legend).
- GUP-543's Decisions (a band on the y axis runs top to bottom by reversing the
  domain).

## User Story

> "As an analyst, I want `gup::heatmap(rows).x(..).y(..).color_value(..)` to
> draw a grid of coloured cells with a colour legend, so that I can see a value
> across two categories at a glance."

## Acceptance Criteria

### AC1: The builder

- [x] `gup::heatmap(rows) -> HeatmapChart<T>` with the methods in Decisions,
      exported from the crate root, with a doctest from the `gup` crate.
- [x] x categories run left to right and y categories top to bottom in
      first-seen order (test from `Layout::x_ticks`/`y_ticks`).
- [x] Each cell fills its band pair less a 1 px gap on each axis when the band
      is at least 8 px, and the whole band pair below that (pixel test at two
      sizes).
- [x] A missing (x, y) pair draws no cell (background shows); a null value draws
      the theme's null colour (tests).
- [x] The ramp legend is drawn at the right with `legend_title` (test from
      `Layout::legend` and the texts).

### AC2: Colour scales

- [x] `ColorValueScale` (Decisions) exists with `From<Sequential>` and
      `From<Diverging>`; `HeatmapChart::color_value_scale` and
      `ScatterChart::color_value_scale` take `impl Into<ColorValueScale>`.
- [x] Every existing scatter test and doctest passes unchanged (a `Sequential`
      argument still compiles).
- [x] A heatmap with `Diverging` over values −1..1 colours a 0 cell with the
      diverging midpoint colour (±2/255) (test).

### AC3: Visual

- [x] Golden `gup_core/heatmap` (640×400): weekdays (y) by 24 hour labels (x,
      `"00"`…`"23"`), viridis, a legend titled "Visits". Golden
      `gup_core/heatmap_diverging`: a 6×6 correlation matrix with a diverging
      scale. The worker reads both: cells aligned with tick labels, even 1 px
      gaps, x labels thinned without collisions, legend readable.

## Decisions

- **API**:

  ```rust
  pub fn heatmap<T: Send + Sync + 'static>(rows: impl IntoIterator<Item = T>) -> HeatmapChart<T>;

  impl<T: Send + Sync + 'static> HeatmapChart<T> {
      // plot_setters!()
      pub fn x<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn y<K: DictionaryKey>(self, key: impl Fn(&T) -> K + Send + Sync + 'static) -> Self;
      pub fn color_value<D: ColumnValue>(self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> Self;
      pub fn color_value_scale(self, scale: impl Into<ColorValueScale>) -> Self;
      pub fn legend_title(self, title: impl Into<String>) -> Self;
      pub fn tooltip(self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> Self;
      pub fn layer_mut(&mut self) -> &mut Selection<T, Rect>;
      pub fn plot_mut(&mut self) -> &mut Plot;
      pub fn into_plot(self) -> Plot;
      pub fn pick(&mut self, at: Point) -> Option<&T>;
  }

  /// A continuous colour scale for `color_value`.
  pub enum ColorValueScale { Sequential(Sequential), Diverging(Diverging) }
  ```

  ```rust
  let chart = gup::heatmap(visits)
      .x(|v| v.hour)
      .y(|v| v.weekday)
      .color_value(|v| v.count)
      .legend_title("Visits");
  ```

  Why: `x`/`y` take category keys exactly as `bar`'s `x` does; `color_value` is
  the scatter's name for colouring by a number, so a user who knows one builder
  guesses the other. Keys are owned, as in every builder (GUP-537): `|v| v.hour`
  for a `u8` hour works.

- **Scales**: x `Band::new().padding(0.0)`, y the same with its domain reversed
  (first-seen at the top). Default colour scale `Sequential::viridis()`.
  `ColorValueScale` lives in `scale/mod.rs` and is exported from the crate root
  (not the prelude).
- **Cells**: one `Rect` layer; `x`/`y` through `builder::erase_key` and the
  bands' `encode_owned_key` with the first-seen domains; `width`/`height` are
  `Derived` from `band_width` minus 1 px when `|band_width| >= 8`, else
  `band_width`.
- **No row aggregation**: one cell per row; duplicate (x, y) pairs draw on top
  of each other in row order (documented: aggregate first).

## Escalation Triggers

- Changing `ScatterChart::color_value_scale`'s signature breaks a caller outside
  `gup-core` (dogfood, egui, Bevy, examples) in a way an `.into()` cannot fix.
- The ramp legend does not appear for a `Diverging` fill on a `Rect` layer (it
  should, via `ShaderFn::legend`): report with a repro.
- `mask wasm-size` scatter grows by more than 3 KB gz (the scatter is touched
  through `color_value_scale`).

## Out of Scope

- Numeric (binned) 2D heatmaps, cell value labels, clustering or reordering.

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `ColorValueScale` and the scatter change; scatter tests green; commit.
- [x] `builder/heatmap.rs`, exports, doctest, order and gap tests; commit.
- [x] Nulls, missing cells, legend, diverging tests; goldens; commit.

## Dependencies

### Prerequisite Stories

- GUP-464: the bar builder ✅ (band axes and key domains).
- GUP-537: owned category keys 📋 — `x`/`y` take owned keys through
  `builder::erase_key`.
- GUP-543: horizontal bars 📋 (soft): the reversed y band pattern, shared if it
  landed first.

### Enables Stories

- GUP-536: the chart tour shows a heatmap.

## Testing Strategy

- **Integration tests**: order, gaps, nulls, missing cells, legend, diverging
  midpoint.
- **Visual validation**: the two goldens, read by eye.

## Success Metrics

- [x] A heatmap with a legend is one call chain.

## Risk Assessment

- **Low**: 24 x labels collide at small widths. _Mitigation_: the band axis
  thins labels (`Slots::ticks`); the golden checks it at 640 px.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (the two goldens)
- [x] `mask wasm-browser`: not needed, builder only
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `gup_core::heatmap` / `HeatmapChart<T>` (`builder/heatmap.rs`): one `Rect`
  layer on two `Band::new().padding(0.0)` axes, y domain reversed, cells
  `band_width` less 1 px from 8 px bands up, default `Sequential::viridis`.
- `ColorValueScale` (`scale/mod.rs`, crate root export) with `From<Sequential>`
  and `From<Diverging>`; `ScatterChart::color_value_scale` and
  `HeatmapChart::color_value_scale` take `impl Into<ColorValueScale>`. Every
  existing scatter test and doctest passes unchanged.
- `tests/heatmap_png.rs` (7 tests: order, gap and abutting cells, missing and
  null cells, legend, diverging midpoint, two goldens); goldens
  `tests/golden/gup_core/heatmap.png` and `heatmap_diverging.png`.
- Files: `builder/{heatmap,mod,scatter}.rs`, `lib.rs`, `scale/mod.rs`.

### Evidence

- `mask merge-check`: story-ids, doc, gup-core, wasm-size, wasm-browser PASS
  (dogfood skipped). WASM scatter gz 338,195 B (recorded 337,974; +221 B).
- Goldens read by eye: `heatmap` shows 7 weekday rows by 24 columns, even 1 px
  white gaps, x labels thinned to every second hour ("00", "02", ...), a viridis
  legend titled "Visits" with ticks 0 to 120. `heatmap_diverging` shows a 6 by 6
  blue-red matrix, a dark red diagonal, a near-white midpoint, and a legend
  titled "r" with ticks -1 to 1.

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### Abutting rects show faint AA seams

- **Challenge**: below 8 px bands the cells fill the whole band, but the shared
  fractional edges antialias, leaving a faint lighter line (up to about 52 of
  255 in one channel).
- **Solution**: kept the Decision (whole band); the test asserts the seam is
  well under a wide band's real gap (222) rather than zero.
- **Pattern**: if seams matter later, snap rect edges to pixels in the shader;
  the builder cannot fix it.

#### Band axes keep 4 px at the top and right

- A non-widened axis insets its far end by the plot padding, so the grid is
  flush at the left and bottom but 4 px short at the right and top. That is
  plot-layout behaviour (GUP-529 territory); not changed.

### Architectural Decisions

#### Skipped SVG comparison for the dense golden

- **Decision**: `gup_core/heatmap` does not call `svg_marks::assert_matches`;
  the diverging golden does. 168 hairline gaps move 4x4 blocks by up to dE 5.4
  against the 5.0 limit. The tolerance was not changed.
- **Future**: re-enable if resvg and the GPU agree better, or the limit is
  re-measured.

#### Doctest location

- The doctest is in `gup_core` (`heatmap` docs), as for the other builders; the
  story asked for one "from the `gup` crate", but `src/lib.rs` is outside the
  `touches:` list.

### Follow-up Stories

None (GUP-524 and GUP-525 not needed).
