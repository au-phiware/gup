---
implementer: sonnet
implementer-reason:
  a library mark on the existing Polyline geometry and MSAA path, with the
  vertex rule (including the baseline crossing) and the SVG form fixed below
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/plot.rs,
  ]
---

# GUP-545: The `Area` Mark

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete 2026-10-25 **Created**: 2026-10-24

## Context

Area charts (a filled band under a series, stacked areas, ranges between two
series) are among the most common charts and Gup has no mark for them; the
strategic review lists "area/stacked area" first among missing chart types.

The pieces exist. `Geometry::Polyline` (GUP-432) gives each instance a row and
its successor, with bridges across chunk boundaries and nulls breaking the run;
an area segment is the trapezoid between rows `i` and `i + 1`, under the top
edge `y` and over the baseline `y_end`. `Antialias::Msaa` makes any scene
holding the mark draw with 4 samples, which polygons with arbitrary edges need
(RFC-001 §7; no library mark uses it yet; `tests/msaa.rs` covers the path).
Adjacent trapezoids share their edge exactly, so a translucent area has no seam
and no double blending (unlike `Line`'s capsules, GUP-439).

This story adds the mark, its `Plot::area` adder and its SVG form. Picking is
GUP-546; the builder and stacking are GUP-547.

## Read First

- `crates/gup-core/src/marks.rs` lines 195–255 (`Line`) and its tests at the end
  of the file.
- `crates/gup-wgsl/library/line.wgsl` (whole) and `view.wgsl`.
- `crates/gup-core/src/channel.rs` lines 275–310 (`Antialias`, `Geometry`).
- RFC-001 "GUP-432 findings" (`docs/planning/rfcs/RFC-001_Core_Architecture.md`
  lines 3170–3260): next-row reads, bridges, validity, the slot budget.
- `crates/gup-core/src/vector.rs` lines 30–130 (`MarkKind`, `VectorMark`) and
  353–400 (the `Line` arm); `crates/gup-core/src/svg.rs` lines 505–540 (the
  polyline writer); `crates/gup-core/src/selection.rs` lines 985–1010
  (`vector`'s mark dispatch).
- `crates/gup-core/src/plot.rs` lines 600–650 (`circles`, `line`, `add`).
- `crates/gup-core/tests/line_png.rs` lines 1–80 (a polyline pixel test).

## User Story

> "As a visualization developer, I want a filled area between a series and a
> baseline, so that I can draw area charts and ranges."

## Acceptance Criteria

### AC1: The mark

- [x] `gup_core::Area` (and `AreaAttrs`) as in Decisions, exported from the
      crate root and the prelude, with
      `Plot::area(rows) -> &mut Selection<T, Area>`.
- [x] Unit test like `line_reads_its_positions_at_the_next_row`: `AreaIn`'s
      members are `x, y, y_end, fill, fill_opacity, x_next, y_next, y_end_next`;
      `Area::ANTIALIAS == Antialias::Msaa`;
      `Area::GEOMETRY == Geometry::Polyline`.
- [x] A doctest draws an area from the `gup` crate and asserts a pixel inside it
      has the fill colour.

### AC2: Geometry

- [x] Integration tests (`crates/gup-core/tests/area_png.rs`): the pixel at the
      centre of each trapezoid is the fill over white (±2/255); at
      `fill_opacity(0.5)` a horizontal scan through the interior of five
      consecutive trapezoids varies by at most 1/255 (no seams); a segment whose
      top crosses its baseline draws the two triangles of Decisions (a pixel
      just above the baseline left of the crossing and just below it right of
      the crossing are filled; the pixel at the crossing column 3 px off the
      baseline on the wrong side is background); a null `y` row breaks the area
      (no fill over the two segments touching it).
- [x] A 10-row area in 4-row chunks (`max_chunk_rows(4)`) renders within 8
      pixels at 2/255 of the one-chunk render (the bridges work).
- [x] `Scene::min_samples()` is 4 for a scene with an area layer.

### AC3: SVG

- [x] `SvgTarget` writes an area run as one `<path>` (Decisions); test: an area
      of 5 rows with a null in the middle writes two paths, with the expected
      point counts.
- [x] The rasterised SVG (resvg, as `svg_marks.rs` does) matches the GPU render
      under the existing `svg_marks` block-average tolerance.

### AC4: Visual

- [x] Golden `gup_core/area_mark` (480×320): a sine wave filled to a 0 baseline
      at `fill_opacity(0.6)` over `grid(Axes::Y)`, crossing the baseline four
      times, with one null gap. The worker reads it: smooth edges, no seams, the
      fill meets the baseline exactly at each crossing.

## Decisions

- **The mark**:

  ```rust
  #[derive(Debug, gup_core_macros::Mark)]
  #[mark(module = "gup::marks::area", geometry = Polyline, antialias = Msaa)]
  pub struct Area {
      /// x (logical pixels).
      #[channel(role = X)]
      x: Px,
      /// The top edge (logical pixels).
      #[channel(role = Y)]
      y: Px,
      /// The baseline (logical pixels); set it, e.g. `.y_end(|_| 0.0)`.
      #[channel(role = Y)]
      y_end: Px,
      /// Fill colour. Default Okabe-Ito blue.
      #[channel(role = Color, default = Area::DEFAULT_FILL)]
      fill: Color,
      /// Fill opacity, multiplied into the fill's alpha. Default 1.
      #[channel(default = Unit(1.0))]
      fill_opacity: Unit,
  }
  ```

  `Area::DEFAULT_FILL = Color::hex(0x0072b2)`. User code with a `Plot`:

  ```rust
  plot.area(days).x(|d| d.t).y(|d| d.rain).y_end(|_| 0.0).fill_opacity(Unit(0.6));
  ```

  Why: `plot.area(rows)` mirrors `plot.line(rows)` (one polyline-shaped layer
  per call); `y_end` is the baseline name `Rect` already uses, so ranges (`y` =
  high, `y_end` = low) read the same as bars.

- **WGSL** (`crates/gup-wgsl/library/area.wgsl`, import path
  `gup::marks::area`): 6 vertices per instance, `vertex_index` 0–5. Let
  `a = (x, y)`, `b = (x_next, y_next)`, `a0 = (x, y_end)`,
  `b0 = (x_next, y_end_next)`, `d0 = y − y_end`, `d1 = y_next − y_end_next`. If
  `d0 * d1 >= 0` (no crossing): triangles `(a, b, b0)` and `(a, b0, a0)`.
  Otherwise, with `t = d0 / (d0 − d1)` and `c = a + t (b − a)` (on the top edge,
  which meets the baseline there): triangles `(a, a0, c)` and `(b, b0, c)`.
  `shade` returns the fill with `fill_opacity` multiplied into alpha,
  premultiplied. The module also emits `row` as a flat varying (for GUP-546).
- **No CPU or GPU pick in this story**: `hit` stays `Unpickable`; GUP-546 adds
  both.
- **SVG**: `MarkKind::Area` and
  `VectorMark::Area { top: Vec<Point>, base: Vec<Point>, fill: Color }`, a run
  of consecutive valid segments with one fill (a run breaks where a segment is
  hidden or its first row's fill differs). Written as
  `<path d="M t0 L t1 … L tn L bn … L b0 Z" fill=… fill-opacity=…/>` with the
  default nonzero fill rule, which fills both lobes of a crossing exactly as the
  two GPU triangles do.
- **Hosts drawing in their own pass** (`Prepared::draw`, egui, Bevy) choose
  their sample count; at 1 sample an area's edges alias. Documented on `Area`;
  no change to hosts.

## Escalation Triggers

- The derive rejects `geometry = Polyline` with `antialias = Msaa`, or two
  `Y`-role channels on a polyline: stop and report the error; do not change the
  macro's rules without the orchestrator.
- An area layer pushes a scene over the device's vertex buffer limit with every
  channel a column (8 slots: 5 columns + 3 next-row bindings): report the error
  text; do not drop channels.
- The seam test fails at 1/255 with MSAA on (shared edges double-covered):
  report pixel values; this contradicts the rasterisation rule the design relies
  on.
- `mask wasm-size` scatter grows by more than 6 KB gz.

## Out of Scope

- Picking and tooltips (GUP-546); the `gup::area` builder, stacking and outlines
  (GUP-547); curved areas (follow-up after GUP-551); horizontal areas (an
  `x_end` baseline).

## Technical Tasks

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] `area.wgsl`, the `Area` struct, `Plot::area`, exports; the unit test;
      commit.
- [x] `area_png.rs` geometry tests and the chunked test; commit.
- [x] The SVG form and its tests; the golden; commit.

## Dependencies

### Prerequisite Stories

- GUP-432: the `Line` mark and `Polyline` geometry ✅.
- GUP-467: SVG writes marks ✅.

### Enables Stories

- GUP-546: area picking; GUP-547: the area builder.

## Testing Strategy

- **Unit tests**: the input struct, constants.
- **Integration tests**: `area_png.rs` (fill, seams, crossing, nulls, chunks);
  the SVG tests.
- **Visual validation**: `gup_core/area_mark`, read by eye.

## Success Metrics

- [x] A filled area renders identically (within the SVG tolerance) in PNG and
      SVG, with no seams at any opacity.

## Risk Assessment

- **Medium**: first library mark on the MSAA path; a 4× sample scene costs more
  to draw. _Mitigation_: `tests/msaa.rs` already covers the target path; record
  the `zoom_bench`-free cost as a note only (no budget for areas yet).
- **Low**: the crossing split in `f32` near `d0 ≈ d1`. _Mitigation_: the
  `d0 * d1 >= 0` branch takes equal signs and zeros; a crossing has `d0 − d1`
  bounded away from 0.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/area_mark`)
- [x] `mask wasm-browser` passes (a new mark in every target)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `area.wgsl` (6 vertices per instance; the crossing split), `Area` /
  `AreaAttrs` in `marks.rs`, `Plot::area`, exports in the crate root and the
  prelude.
- SVG: `MarkKind::Area`, `VectorMark::Area`, one `<path>` per run (`vector.rs`,
  `svg.rs`, the dispatch in `selection.rs`).
- Tests: `area_reads_its_positions_at_the_next_row` (unit), the `Area` doctest
  (a pixel inside has the fill), and `tests/area_png.rs` (7 tests: centres at
  two opacities, no seams, the crossing, a null break, chunks, SVG paths, the
  golden `gup_core/area_mark` and its SVG match).
- The doctest lives in `gup-core`'s `Area` docs (the `gup` crate re-exports the
  mark); no doctest was added to the root crate's `src/lib.rs`.
- The golden has a 60% fill, so it sets no expected colour (the colour check
  looks for the configured, opaque colour); the pixel tests check the blend.
- `mask merge-check` PASS; WASM scatter 334,365 B gz (budget 334,149, +216 B).

## Retrospective

**Completed**: 2026-10-25

### Key Technical Learnings

#### Shared edges and MSAA

- **Challenge**: Whether adjacent MSAA triangles seam at partial opacity.
- **Solution**: They do not: the seam scan varies by at most 1/255, so the
  rasterisation rule gives each sample to one triangle.
- **Pattern**: Build translucent fills from triangles sharing exact vertices.

#### The bowtie

- **Challenge**: A segment whose top crosses the baseline.
- **Solution**: The naive `(a, b, b0), (a, b0, a0)` split fills the wrong side
  of the crossing (right of it, above the baseline); the test checks those
  pixels, so the split at `c` is what the test proves.

### Architectural Decisions

#### No expected colour on a translucent golden

- **Decision**: `area_mark` sets no `with_expected_color`.
- **Reasoning**: `color_present` looks for the configured colour at full
  strength; a 60% fill never shows it.
- **Future**: A harness option for "the colour over white at this opacity" would
  let translucent marks keep the check.

### Development Workflow Insights

- The worktree sandbox refuses compound shell commands (`cd x && ...`, `;` with
  `git`); run plain commands from the checkout root.

### Follow-up Stories

None.
