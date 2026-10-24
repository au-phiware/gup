---
implementer: opus
implementer-reason:
  GPU numerical work with no precedent: per-row Bezier controls computed from
  neighbours on the CPU, carried to the shader without changing straight
  lines, tessellated per instance; the mechanism choice has a fixed rule
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/builder/line.rs,
  ]
---

# GUP-551: Line Curves, Part 2: Smooth Monotone Curves

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

GUP-550 adds `Curve` and step curves on a per-layer vertex count and a curve
constant in the glue. A smooth curve is harder: the tangent at a row depends on
its neighbours (rows `i − 1` and `i + 1`), so segment `i` needs four rows. The
`Polyline` geometry reads two, and RFC-001 "GUP-432 findings" rejected reading
more for the vertex-buffer slot budget (8 slots; a line with column x, y, width
and colour already uses 6) and for the bridge logic across chunks.

A monotone cubic in x (Steffen's method, which several plotting libraries use as
their "monotone X" curve) never overshoots the data, which is the property a
data line needs. For each segment it is a cubic Bezier from row `i` to row
`i + 1` with control points at one third and two thirds of the segment in x and
y values `y_i + m_i h / 3` and `y_{i+1} − m_{i+1} h / 3` (`h` the x gap, `m` the
tangents). Under the plot's scales the curve stays monotone and within the
segment's y range because both control values lie between the end values and
every position scale is monotone; under linear and time scales it is exactly the
pixel-space curve, since Bezier curves are affine-invariant.

## Read First

- GUP-550's Decisions and RFC-001 "GUP-550 findings" (the per-layer constant and
  vertex count).
- RFC-001 "GUP-432 findings" (lines 3170–3300: slots, bridges, validity,
  append).
- `crates/gup-core/src/selection.rs`: `fit_domains` and the column evaluation
  path it calls in `column.rs` (search `fn evaluate` / `fn upload`), `append`
  (lines 300–345), `prepare` (1480–1565).
- `crates/gup-core/src/column.rs` lines 1–150 (the column store's model).
- `crates/gup-wgsl/library/line.wgsl` after GUP-550.
- Steffen, "A simple method for monotonic interpolation in one dimension"
  (1990): the tangent formula (implement from the formula; no new dependency).

## User Story

> "As an analyst showing a trend, I want `.curve(Curve::MonotoneX)`, so that my
> line is smooth but never invents peaks or dips between my data points."

## Acceptance Criteria

### AC1: Mechanism (decided by rule, recorded)

- [ ] The implementer first checks rule (A) below; the Implementation Summary
      and RFC-001 "GUP-551 findings" record which was used and why, with the
      slot count of the worst case (column x, y, width, colour).
- [ ] Straight and step lines are unchanged (GUP-550's AC2 snapshot still
      passes; no golden re-blessed).

### AC2: The curve

- [ ] `Curve::MonotoneX` exists and draws the curve described in Context through
      every row: unit tests of the tangent function on monotone data (the
      interpolant is monotone between every pair), a flat run (zero tangents), a
      local extremum (zero tangent at the extremum), two-row runs (straight); a
      pixel test that the drawn path passes within 0.5 px of every row and, for
      monotone data, never leaves `[min(y_i, y_{i+1}), max(..)]` between rows
      (sampled at 0.5 px).
- [ ] Non-increasing x within a run (`x_{i+1} <= x_i`) draws that segment
      straight (test); nulls break the curve and restart its tangents (test).
- [ ] On a log y scale the curve still passes through every row and stays within
      each segment's pixel y range (test).
- [ ] Appending rows: the curve after appends equals the curve of all rows built
      at once within 8 pixels at 2/255, or (if rule (A)'s mechanism cannot
      rewrite the previous last row) the difference is limited to the old last
      segment and documented on `Curve::MonotoneX` (test either way).

### AC3: Picking, SVG, visual

- [ ] CPU and GPU picks on the curve report the nearer row in x, as for straight
      lines (test).
- [ ] `SvgTarget` writes each run as a `<path>` of cubic Bezier commands (`C`)
      with the same control points (test on a 5-row line); the rasterised SVG
      matches the GPU render under `svg_marks.rs`'s tolerance.
- [ ] Golden `gup_core/line_monotone` (640×400): 12 noisy monthly values drawn
      `MonotoneX` over their points (circles), beside the same data `Linear` in
      a second, lighter layer. The worker reads it: smooth, through every point,
      no overshoot at the peaks.

### AC4: Cost

- [ ] The GPU pass time of a 100k-row `MonotoneX` line is measured against the
      same line `Linear` (`zoom_bench` style, release) and recorded; if it is
      more than 6× the linear line's, use 8 pieces instead of 16 and record both
      measurements.

## Decisions

- **API**: `Curve::MonotoneX` added to GUP-550's `Curve` (documented: "a
  monotone cubic in x through every row; never overshoots").
- **Tangents** (Steffen), per run of valid rows with strictly increasing x, in
  data units: interior
  `m_i = (sign(s_{i−1}) + sign(s_i)) · min(|s_{i−1}|, |s_i|, |p_i| / 2)` where
  `s` are the secant slopes and `p_i` the parabola slope
  `(s_{i−1} h_i + s_i h_{i−1}) / (h_{i−1} + h_i)`; ends use the one-sided rule
  of the same paper. Computed in `f64`.
- **Control values**: per row `i`, `c1_i = y_i + m_i h_i / 3` and
  `c2_i = y_{i+1} − m_{i+1} h_i / 3` (both belong to segment `i`, so no next-row
  read of them is needed), mapped through the plot's y scale like `y`; the x of
  the controls is taken at one and two thirds of the segment in pixels.
- **Mechanism rule**: (A) the layer computes `c1`, `c2` on the CPU when its
  columns are evaluated (from the evaluated x and y values, in data order) and
  stores them as two extra columns read only by monotone layers (straight layers
  neither allocate nor bind them), so a column-x, y, width, colour monotone line
  uses 8 slots. Use (A) unless it needs a change to the public column-store or
  `Selection` API, or exceeds 8 slots; then (B) the builder computes the
  controls and the curve is offered on `LineChart` only (not
  `Selection<T, Line>`), documented. Either way record it.
- **Tessellation**: 16 pieces per segment (96 vertices), each a capsule between
  consecutive Bezier points evaluated in pixel space, subject to AC4's rule.

## Escalation Triggers

- Neither (A) nor (B) fits without changing the column store's public API or the
  `Mark` trait: stop and report the obstacle.
- The worst-case slot count exceeds the device's `max_vertex_buffers` on the
  browser smoke test's adapter.
- AC4's measurement exceeds 10× with 8 pieces.

## Out of Scope

- Other smooth curves (natural, cardinal, Catmull–Rom, basis); curved areas;
  translucent joins (GUP-439).

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Tangent function with unit tests; commit.
- [ ] Mechanism (A) or (B) per the rule; first pixel test; commit.
- [ ] Remaining AC2 tests, picks, SVG, golden; commit.
- [ ] Cost measurement; RFC-001 findings; docs.

## Dependencies

### Prerequisite Stories

- GUP-550: Line curves part 1 📋.

### Enables Stories

- GUP-439: translucent joins (curves make beading more visible).
- Curved areas (a follow-up).

## Testing Strategy

- **Unit tests**: tangents and control values.
- **Integration tests**: path, monotonicity, log scale, nulls, appends, picks,
  SVG.
- **Visual validation**: `gup_core/line_monotone`, read by eye.
- **Performance**: AC4.

## Success Metrics

- [ ] Smooth monotone lines through every data point, in every target.

## Risk Assessment

- **High**: carrying per-row derived values through the column store is new.
  _Mitigation_: the rule gives a fallback (B) that keeps the change inside the
  builder.
- **Medium**: 16 pieces per segment multiplies vertex work. _Mitigation_: AC4's
  measurement and rule.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`gup_core/line_monotone`)
- [ ] `mask wasm-browser` passes
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
