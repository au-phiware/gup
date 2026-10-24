---
implementer: sonnet
implementer-reason:
  a new hit shape following the Polyline/Capsule pattern on the CPU and the Line
  mark's hit_distance/hit_row pattern on the GPU; rules fixed below
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/channel.rs,
    crates/gup-core-macros/src/mark.rs,
  ]
---

# GUP-546: Hover, Tooltips and Brushing on Areas

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 📋
Planned **Created**: 2026-10-24

## Context

GUP-545's `Area` mark is `Unpickable`, so an area layer has no hover, no tooltip
and no click events, and a stacked area chart cannot say which series the
pointer is over. Every other library mark is pickable on the CPU (the pick
index, `pick.rs`) and on the GPU (`gpu_pick`, for layers whose rows were
released). `Line`'s polyline hit (a capsule per segment, the nearer row
reported) is the closest precedent.

## Read First

- `crates/gup-core/src/pick.rs` lines 30–160 (`Shape`, distances, anchors) and
  205–310 (`hit_items` per `HitShape`).
- `crates/gup-core/src/selection.rs` lines 1600–1700 (`hit_items`: wanted
  channels, polyline ranges).
- `crates/gup-core/src/channel.rs` lines 310–325 (`HitShape`).
- `crates/gup-core-macros/src/mark.rs` lines 228–280 (the `hit` allow-list and
  its checks).
- `crates/gup-wgsl/library/line.wgsl` (`hit_distance`, `hit_row`) and the
  `area.wgsl` from GUP-545.
- `crates/gup-core/tests/pick.rs` (the polyline cases) and
  `crates/gup-core/src/gpu_pick/tests.rs` lines 1–80.

## User Story

> "As a dashboard user, I want to hover over a stacked area and see which series
> and value I am pointing at."

## Acceptance Criteria

### AC1: CPU picking

- [ ] `HitShape::Area` exists (documented: "the trapezoid from each row to the
      next between `y` and `y_end`, `Geometry::Polyline`"), the derive accepts
      `hit = Area` for a polyline mark with `x`, `y`, `y_end` `Px` channels and
      rejects it otherwise (a `derive_mark_fail` case), and `Area` uses it.
- [ ] `pick.rs` gains `Shape::Area { a, b, a0, b0 }`: the distance is negative
      inside the segment's region (the two GPU triangles of GUP-545's rule,
      crossing included) and the distance to its outline outside. The row
      reported is the segment's first row when the point's x is at or before the
      segment's midpoint in x, else the next row; the anchor is that row's top
      point `(x, y)`.
- [ ] Tests (`tests/pick.rs`): a point inside segment 2 left of its middle picks
      row 2, right of it row 3; a point between the top edge and the baseline of
      a crossing segment but outside both triangles picks nothing; of two
      stacked area layers, the point inside the upper band picks the upper
      layer's row.

### AC2: GPU picking

- [ ] `Area` is `gpu_pick`: `area.wgsl` defines `hit_distance` (−1 inside, the
      fragment exists only inside the triangles) and `hit_row` (from a varying
      `t`, 0 at row `i`'s vertices and 1 at row `i + 1`'s, `c` at its own `t`:
      row `i` when `t <= 0.5`).
- [ ] A test in `gpu_pick/tests.rs` (or `tests/pick.rs` with `Retain::GpuOnly`)
      agrees with the CPU pick on the three AC1 points.

### AC3: The user-visible result

- [ ] A tooltip test: an area layer with `.tooltip(|r| ..)` shows the tooltip
      text of the row under the pointer after `Input::PointerMoved`
      (`Layout::tooltip` is `Some`).
- [ ] `Drag::Brush` over part of an area selects the rows whose top point lies
      in the rectangle, as for lines (test).

## Decisions

- **Row rule = Line's**: the nearer end of the segment in x, so a tooltip shows
  the data point closest to the pointer horizontally, which is what a time
  series reader expects.
- **Brushing uses the top point** of each row (`x`, `y`), consistent with the
  anchor.
- **No pick margin** for areas: the region is already large; the GPU pick draws
  the triangles unchanged.

## Escalation Triggers

- The pick index's 32 px grid cells need a bounding box per item and the area
  shape's box is large (tall areas cover many cells): if indexing 100k area rows
  takes more than 3× a 100k-row line's index build, report the timing.
- The GPU and CPU picks disagree at a triangle edge by more than 1 px.

## Out of Scope

- The `gup::area` builder's tooltip setter (GUP-547 adds it on top of this).
- Pick margins or snapping to the nearest series.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [ ] `HitShape::Area`, the macro allow-list and fail case; commit.
- [ ] `Shape::Area`, `hit_items`, CPU tests; commit.
- [ ] GPU hit functions and test; tooltip and brush tests; commit.

## Dependencies

### Prerequisite Stories

- GUP-545: The `Area` mark 📋.

### Enables Stories

- GUP-547: the area builder's tooltips.

## Testing Strategy

- **Unit tests**: `Shape::Area` distance inside, outside, at a crossing.
- **Integration tests**: CPU and GPU picks, tooltip, brush.

## Success Metrics

- [ ] Hover and tooltips work on areas as on every other library mark.

## Risk Assessment

- **Low**: the CPU distance outside a trapezoid is more code than a capsule's.
  _Mitigation_: only its sign is needed inside; outside, the distance to the
  nearest of four edges (or three per triangle) is enough for the tolerance
  check.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: tooltip test only (no new golden)
- [ ] `mask wasm-browser` passes (input handling)
- [ ] WASM size: `mask wasm-size` scatter gz bytes reported
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
