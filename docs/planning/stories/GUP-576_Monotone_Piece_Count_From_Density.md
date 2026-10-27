---
implementer: opus
implementer-reason:
  A per-layer vertex count chosen at prepare from pixel density, through the
  glue and the pipeline cache, without changing straight lines
touches:
  [
    crates/gup-core/src/selection.rs,
    crates/gup-core/src/marks.rs,
    crates/gup-wgsl/library/line.wgsl,
  ]
---

# GUP-576: Monotone Curves: Piece Count From Pixel Density

## Story Overview

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: 💡
New **Created**: 2026-10-27

## Context

GUP-551 draws every `Curve::MonotoneX` segment as 16 pieces (96 vertices). On a
100K-row random walk, its GPU pass takes 18x that of the same line `Linear`
(28.7 against 1.6 ms, Intel HD 630). The story's AC4 asked for 6x at most. With
8 pieces it is 8.8x, but the golden then visibly facets at normal size.

The cost is the vertex count. Collapsing flat segments inside the shader, with
their unused vertices sent outside the clip volume, saved under 10%. At 100K
rows in 1600 px a segment is 0.016 px wide, and a curve cannot be seen there.

See RFC-001 "GUP-551 findings", "Cost".

## User Story

> "As an analyst plotting 100K points with `.curve(Curve::MonotoneX)`, I want
> the chart to zoom as smoothly as a straight line, and a 12-point chart to stay
> perfectly smooth."

## Acceptance Criteria

- [ ] The layer chooses its pieces per segment, 1 to 16, at prepare from the
      data's pixel density: for example, the median segment width in pixels
      under the current x scale. The vertex count is `6 × pieces`. It changes
      only when the count changes, and the shader reads the count as a constant,
      not as `MONOTONE_PIECES`.
- [ ] `zoom_bench --line monotone` (100K rows) is at most 2x `--line linear`;
      `gup_core/line_monotone` is unchanged (16 pieces).
- [ ] `Curve::path` and the picks use the same count. The seam tests of
      `line_monotone.rs` pass at every count.

## Decisions

- Keep the Chebyshev nodes and the plane joints of GUP-551.

## Escalation Triggers

- The count needs a change to the `Mark` trait or to straight-line glue.

## Out of Scope

- Other smooth curves.

## Technical Tasks

- [ ] Count from density; uniform or glue constant; tests; measurement.

## Dependencies

### Prerequisite Stories

- GUP-551 ✅.

## Testing Strategy

- Unit test of the count rule; `line_monotone.rs` at 1, 4 and 16 pieces;
  `zoom_bench` for the cost.

## Success Metrics

- [ ] Dense monotone lines cost about what straight lines cost.

## Risk Assessment

- **Medium**: a count that changes while zooming relinks a pipeline. A uniform
  count with a fixed maximum vertex count avoids the relink, but costs vertices.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core -- --test-threads=1` and `mask all-fix` pass
- [ ] Measurements recorded in RFC-001
- [ ] Story status updated in the story file and INDEX.md
