---
implementer: opus
implementer-reason:
  per-layer vertex counts and a curve constant in the generated glue are new
  infrastructure in selection, render and glue code; API and step geometry are
  fixed below, the mechanism needs judgment
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/lib.rs,
    crates/gup-core/src/builder/line.rs,
  ]
---

# GUP-550: Line Curves, Part 1: the `Curve` Option and Step Curves

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

`Line` draws straight segments only; the builder module's migration table says
"`.interpolate`, `.smooth`, `.monotone`, `.step`: dropped: straight segments
only". Step lines are the right chart for values that hold until they change
(prices set at times, inventory, states, cumulative counts), and smooth lines
are what many users expect of a trend. The review puts "line curves with
GUP-439" in N2.

A step from row `i` to row `i + 1` needs only those two rows, which the
`Polyline` geometry already reads (`x`, `y`, `x_next`, `y_next`), so steps can
be drawn exactly in the vertex shader in pixel space, under any scale. A step
segment is two or three straight pieces, so its instance needs 12 or 18 vertices
where a straight segment needs 6. The vertex count today is the mark's constant
(`Mark::VERTICES_PER_INSTANCE`, passed to `LayerUniforms::build` in
`Selection::prepare`); a straight line must keep 6 (the 4M-row budgets in
`PERF_BUDGETS.md` are lines and scatters).

This story adds the `Curve` option, the per-layer machinery and step curves.
GUP-551 adds a smooth monotone curve on the same machinery.

## Read First

- `crates/gup-core/src/marks.rs` lines 195–255 (`Line`) and
  `crates/gup-wgsl/library/line.wgsl` (whole) and `coverage.wgsl`'s `line_quad`.
- `crates/gup-core/src/selection.rs` lines 1480–1565 (`prepare`, the vertex
  count) and the `Selection` struct (lines 180–240).
- `crates/gup-core/src/render.rs` lines 200–220, 770–830 and 975–985
  (`vertices_per_instance`, the draw).
- `crates/gup-core/src/shader/glue.rs` lines 1–130 (`GlueSpec`, `Glue`) and
  355–510 (the signature).
- `crates/gup-core/src/pick.rs` lines 30–160 and 290–305 (`Shape::Capsule`,
  polyline hits) and `crates/gup-core/src/vector.rs` lines 353–400 (the `Line`
  arm).
- RFC-001 "GUP-432 findings" (lines 3170–3300).
- `crates/gup-core/src/builder/line.rs` (whole).

## User Story

> "As an analyst plotting a price that holds between changes, I want
> `.curve(Curve::StepAfter)` on my line, so that the chart shows levels, not
> slopes that never happened."

## Acceptance Criteria

### AC1: API

- [ ] `Curve` (Decisions) is exported from the crate root and the prelude;
      `Selection<T, Line>::curve` and `LineChart::curve` exist with doctests
      from the `gup` crate.
- [ ] The migration table row says `.curve(Curve::StepAfter)` (and names GUP-551
      for smooth curves).

### AC2: Straight lines are untouched

- [ ] With `Curve::Linear` (the default) a line's glue source, glue signature,
      vertex count (6) and pixels are unchanged: `line_series`, `builder_line`
      and every golden pass without re-blessing, and a test asserts the default
      layer's glue equals the glue built before this story (compare with a
      snapshot taken at the story's first commit, or assert the signature
      string).
- [ ] `mask perf-budget`'s exact counters are unchanged (timing rows are
      advisory).

### AC3: Steps

- [ ] Pixel tests (`crates/gup-core/tests/line_curves.rs`), each on four rows
      with distinct y: `StepAfter` has ink along `y = y_i` from `x_i` to
      `x_{i+1}` and along `x = x_{i+1}` between the two y values, and none on
      the straight diagonal's midpoint; `StepBefore` the mirror; `Step` turns at
      the midpoint x. Opaque joins show no gap (coverage ≥ 0.9 along every 0.5
      px sample of the path).
- [ ] On a log y scale the step's horizontal pieces stay exactly horizontal (the
      same pixel row at both ends, ±0.5 px).
- [ ] Nulls break step lines as they do straight ones; a 10-row step line in
      4-row chunks matches the one-chunk render within 8 pixels at 2/255.

### AC4: Picking, SVG

- [ ] CPU and GPU picks on a step line report: `StepAfter`, row `i` for any
      point on segment `i`'s path; `StepBefore`, row `i + 1`; `Step`, row `i`
      left of the midpoint, else `i + 1` (tests).
- [ ] `SvgTarget` writes a step line as one `<polyline>` with the corner points
      inserted; the rasterised SVG matches the GPU render under `svg_marks.rs`'s
      tolerance.

### AC5: Visual

- [ ] Golden `gup_core/line_steps` (640×400): the same eight points drawn as
      `StepAfter`, `StepBefore` and `Step` in three colours with a legend (three
      layers). The worker reads it: square corners, no gaps, correct turn
      positions.

## Decisions

- **API**:

  ```rust
  #[non_exhaustive]
  #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
  pub enum Curve {
      #[default]
      Linear,
      /// Horizontal to the midpoint between rows, vertical, then horizontal.
      Step,
      /// Vertical at each row, then horizontal to the next row.
      StepBefore,
      /// Horizontal from each row to the next row's x, then vertical.
      StepAfter,
  }

  impl<T: Send + Sync + 'static> Selection<T, Line> {
      pub fn curve(&mut self, curve: Curve) -> &mut Self;
  }
  impl<T: Send + Sync + 'static> LineChart<T> {
      pub fn curve(self, curve: Curve) -> Self;
  }
  ```

  ```rust
  let chart = gup::line(prices).x(|p| p.t).y(|p| p.price).curve(Curve::StepAfter);
  ```

  Why: `curve` is the common name for a line's interpolation; the variants use
  the names users meet in other plotting tools for the same shapes. It is a
  setter on the line, not a new mark, so `layers_mut()` keeps its type and a
  step line keeps every `Line` feature (colour by key, windows, appenders).

- **Pieces**: `StepAfter` and `StepBefore` are 2 pieces (12 vertices), `Step` 3
  (18). Each piece is a capsule built like today's segment (`line_quad`, round
  caps), from the endpoints computed in pixel space from `(x, y)` and
  `(x_next, y_next)`; consecutive capsules overlap at corners as segments do at
  rows (translucent beading is GUP-439's).
- **Per-layer vertex count**: `Selection` holds the vertex count it passes to
  `LayerUniforms::build` (default `M::VERTICES_PER_INSTANCE`); `curve` sets it.
  The `Mark` trait and `#[derive(Mark)]` do not change.
- **The curve reaches WGSL through the glue**, as a constant specific to the
  layer that is part of the glue signature (so linear lines keep their pipeline
  and step lines get their own), and `line.wgsl`'s `vertex` reads the piece
  index `vertex_index / 6`. `LineIn` does not change. How the constant is
  emitted (a generated `const`, a WGSL `override`, or a uniform word) is the
  implementer's choice, recorded in RFC-001 as "GUP-550 findings", subject to
  AC2.
- **Picking**: `HitShape::Polyline` gains per-curve piece capsules on the CPU
  (`Shape` gets the pieces, or a new variant), with the row rule in AC4; the GPU
  `hit_distance`/`hit_row` use the same pieces.

## Escalation Triggers

- AC2 cannot hold (a linear line's glue or pipeline must change): stop and
  report what forces it.
- The per-layer vertex count needs a change to the public `Mark` trait or the
  derive: stop and report.
- Chunk culling (`cull.rs`) or the bridges (GUP-432) treat a step line's
  instances differently from a straight line's (they should not: a step stays
  inside its two rows' bounding box).
- `mask wasm-size` scatter grows by more than 4 KB gz.

## Out of Scope

- Smooth curves (GUP-551); translucent joins (GUP-439); curves on areas
  (follow-up after GUP-551).

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] Snapshot a linear line's glue; add `Curve`, the per-layer vertex count and
      the glue constant; AC2 green; commit.
- [ ] Step pieces in `line.wgsl`; AC3 tests; commit.
- [ ] Picks, SVG, golden, builder setter, docs, RFC findings; commit.

## Dependencies

### Prerequisite Stories

- GUP-432: the `Line` mark ✅.

### Enables Stories

- GUP-551: monotone curves on the same mechanism.
- GUP-439: translucent joins (its design must cover pieces too).

## Testing Strategy

- **Integration tests**: `line_curves.rs` (paths, log scale, nulls, chunks,
  picks); the glue snapshot.
- **Visual validation**: `gup_core/line_steps`, read by eye.
- **Performance**: `mask perf-budget` exact counters unchanged.

## Success Metrics

- [ ] Step lines are one setter away and straight lines cost exactly what they
      did.

## Risk Assessment

- **Medium**: the glue generator gains a per-layer specialisation that later
  marks may want too. _Mitigation_: keep it generic ("a per-layer constant in
  the signature") and record it in RFC-001.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (`gup_core/line_steps`)
- [ ] `mask wasm-browser` passes (glue change)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
