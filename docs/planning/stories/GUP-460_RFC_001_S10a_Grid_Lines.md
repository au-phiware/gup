---
implementer: sonnet
implementer-reason:
  API, colour, placement, z and clipping are fixed in Decisions; follows the
  axis rule code in `Plot::resolve`
---

# GUP-460: RFC-001 Step S10a: Grid Lines on `Plot`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-18

## Context

RFC-001 S7 listed "axes/grid/legend/title guides". Axes, legends and titles
landed (GUP-425, GUP-426), but grids never did. `plot.rs`'s module docs still
say "grids … are later RFC-001 S7 steps", and `scene.rs` defines `Z_GRID` that
nothing uses. The old builders' users call `.grid()` and `.horizontal_grid()`.
Dogfood tasks 1 and 3 configure a grid and carry the tracked gap `NO_GRID`, and
RFC-001 S10's exit criterion is that dogfood tasks 1–3 pass. The S10 builders
(GUP-462 onward) forward a `.grid(..)` setter to this API.

## Read First

- `crates/gup-core/src/plot.rs` lines 1690–1760 (axis rules and ticks in
  `resolve`) and 1825–1850 (pushing scene items with a clip).
- `crates/gup-core/src/scene.rs` lines 10–30 (`Z_*`).
- `crates/gup-core/src/input.rs` lines 90–115 (`Axes`).
- `crates/gup-core/src/theme.rs` (fields and `Default`).
- `crates/gup-core/tests/theme_png.rs` (a golden test to copy).

## User Story

> "As a developer making a chart for a report, I want light grid lines at the
> ticks, so that readers can read values off the chart."

## Acceptance Criteria

### AC1: API

- [ ] `Plot::grid(&mut self, axes: Axes) -> &mut Self` and `Theme::grid: Color`
      exist as in Decisions, with a doctest that builds a plot with
      `.grid(Axes::Y)`.
- [ ] The default is no grid. Every existing golden is unchanged (run the whole
      gup-core suite without `GUP_BLESS`).

### AC2: Drawn right

- [ ] A golden, `gup_core/grid`, of the reference scatter
      (`tests/common/scatter.rs`) with `.grid(Axes::Both)` at 720 × 450. The
      implementer reads it:
  - one light line at every x and y tick;
  - inside the plot rect only;
  - behind the dots;
  - the axis lines still dark where a grid line meets them.
- [ ] Pixel checks in the same test:
  - halfway between two x ticks, on a y tick's row, the pixel is
    `Theme::default().grid` (±2/255);
  - a dot centred on a grid line shows the dot's colour at its centre;
  - 3 px outside the plot rect, on a tick's row, there is no grid colour.
- [ ] `Axes::X` draws only vertical lines, `Axes::Y` only horizontal ones, and
      `Axes::Neither` none. Counted from the scene's `Rules` item at `Z_GRID`.

### AC3: Zoom and outputs

- [ ] While zooming, the grid follows the ticks. One zoom step on a grid plot
      writes 0 column bytes, and its guide-instance bytes grow by exactly the
      grid's rules (count them).
- [ ] `SvgTarget` on a grid plot writes one `<line>` per grid line, clipped like
      the marks. Extend `tests/svg.rs`.
- [ ] `mask perf-budget`'s `zoom.instances.bytes` is unchanged (`zoom_bench` has
      no grid).

### AC4: Docs

- [ ] `plot.rs`'s module docs no longer say grids are later work.

## Decisions

- **API**:

  ```rust
  impl Plot {
      /// Draw grid lines at the ticks of the given axes (default
      /// `Axes::Neither`): vertical lines at x ticks, horizontal at y ticks.
      pub fn grid(&mut self, axes: Axes) -> &mut Self;
  }
  pub struct Theme { /* … */ pub grid: Color }   // default Color::hex(0xe5e5e5)
  ```

  `Axes` is reused, so `Axes::Both`, `X`, `Y` and `Neither` read naturally; no
  new enum is added.

- **Geometry**: 1 px `Rule`s on pixel centres (`floor(px) + 0.5`, as the ticks
  are), spanning the plot rect's full width or height. They go in one
  `ItemKind::Rules` item at `Z_GRID`, clipped to the plot rect (the clip the
  marks use). The tick values are the axis's (`x_ticks.values`,
  `y_ticks.values`), including on `Band` and `Point` axes.
- **Revision**: `grid` bumps the revision like `title`.
- **Not themed per axis**: one colour and width for both directions.

## Escalation Triggers

- Any existing golden changes with the grid off.
- The grid needs its own pipeline, rather than the `Rules` item the axes use.

## Out of Scope

- Minor grid lines, dashed lines, per-axis colours.
- Builder `.grid(..)` setters: GUP-462 onward.

## Technical Tasks

- [ ] `Theme::grid`; `Plot::grid` and its field.
- [ ] Emit the grid rules in `resolve`; push the clipped item at `Z_GRID`.
- [ ] Golden and pixel tests (AC2); zoom and SVG tests (AC3); docs.

## Dependencies

### Prerequisite Stories

- GUP-425: RFC-001 S7b Theme and Axis Titles ✅ — `Theme`.

### Enables Stories

- GUP-462: chart builders forward `.grid(..)`.
- GUP-465: dogfood tasks 1 and 3's grid checks.

## Testing Strategy

- **Integration**: golden plus pixel checks; rule counts per `Axes`; SVG line
  count; zoom upload counts.
- **Visual**: `gup_core/grid.png`, read by the implementer.

## Success Metrics

- [ ] `NO_GRID` can become a plain check in dogfood tasks 1 and 3.

## Risk Assessment

- **Low**: a grid line on the plot rect's edge overdraws the axis line.
  _Mitigation_: axes draw at `Z_AXES`, above `Z_GRID`; AC2 checks it.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: `gup_core/grid.png`
- [ ] `mask wasm-size` passes
- [ ] `mask old-path-loc` recorded (expected unchanged, 28,910)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
