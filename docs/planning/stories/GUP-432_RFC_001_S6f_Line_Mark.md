---
implementer: opus
implementer-reason:
  design work - binding each row with its successor across column chunks with
  per-chunk origins, within the vertex-buffer slot budget, plus join geometry
---

# GUP-432: RFC-001 Step S6f: `Line` Mark

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-16

## Context

`Line` connects consecutive rows into a polyline. RFC-001 §3 says the geometry
"bind[s] the same column twice, offset by one element, which gives a
previous/next pair per instance". It was split out of
[GUP-423](GUP-423_RFC_001_S6b_Segment_Mark.md) on 2026-10-16 because that
sentence hides three design problems no current mark has:

- **Chunk boundaries.** A layer draws one instanced draw per column chunk
  (`render.rs` `ChunkDraw`), and S4a stores each chunk's positions relative to
  that chunk's own origin (RFC-001 "S4a findings → Precision at a chunk
  boundary"). The last row of a chunk has its successor in another buffer with
  another origin, so a plain offset binding leaves a gap at every chunk
  boundary.
- **Slots.** Binding `x` and `y` twice takes four instance-rate vertex buffers
  before colour, width or validity; RFC-001 §12 risk 7 calls the 8-slot budget
  tight.
- **Joins.** Two segments meeting at an angle leave a wedge gap on the outer
  side. A capsule per segment (distance to the segment, round caps; `line_quad`
  with `cap = width / 2` gives the quad) closes it with round joins using only
  the two endpoints, but overlaps at joins double-blend a translucent line.
  Miter and bevel joins need the neighbours' directions (four rows per
  instance).

Nulls break the line: RFC-001 S4b hides a row with a null position by a
degenerate quad (`glue.rs`, the validity and `NULL_CODE` checks), which for a
line must hide both segments touching the null row.
[GUP-421](GUP-421_Analytic_AA_Rules_Rects_1x_MSAA.md)'s `gup::coverage`
(`line_quad`, `box_coverage`) and GUP-423's `segment.wgsl` are the starting
point for the shader.

## Read First

- RFC-001 §3 Design (lines 160–258), "S4a findings" (lines 2042–2195) and "S4b
  findings → Validity bits" (lines 2203–2235).
- RFC-001 §12 risk 7 (in lines 796–866).
- RFC-001 "GUP-421 findings" (lines 3016–3055).
- `crates/gup-wgsl/library/coverage.wgsl`,
  `crates/gup-wgsl/library/segment.wgsl` (GUP-423).
- `crates/gup-core/src/render.rs` lines 74–120 and
  `crates/gup-core/src/shader/glue.rs` lines 330–365 and 480–500.

## User Story

> "As a visualization developer, I want a `Line` mark that connects consecutive
> rows with a real width and clean joins, and breaks at missing values, so that
> I can draw time series without approximating them."

## Acceptance Criteria

### AC1: Design recorded

- [ ] A "GUP-432 findings" section appended to RFC-001 records how the successor
      is bound (including across chunks with different origins), the slot count
      for a line with per-row colour and width, the join style and why, and the
      append behaviour.

### AC2: Connected rows

- [ ] `gup_core::Line` (derive) draws `N - 1` segments for `N` rows; a test with
      `max_chunk_rows(4)` and 10 rows has no gap at the chunk boundaries: every
      pixel on the polyline's centreline (sampled every 0.5 px) has coverage ≥
      0.9.

### AC3: Joins

- [ ] A "V" at 30° and a shallow 170° turn, width 6 px, show no wedge gap
      (centreline-and-outer-edge samples covered) and, for an opaque colour, no
      visible seam; the join style's behaviour with translucent colour is
      documented.

### AC4: Null rows break the line

- [ ] A series with a NaN in the middle draws two disconnected pieces: no ink
      within `width` of the straight line between the null's neighbours, except
      where the pieces themselves are.

### AC5: Golden

- [ ] Golden `gup_core/line_series`: a 200-point time series, width 2 px, with
      one null gap, read by eye: smooth, clean joins, visible break.

## Decisions

- `Line`'s channels include `x`, `y` (roles `X`, `Y`), `width: Px` and
  `color: Color`; further channels and the binding are this story's design.
- Caps at the two ends of a piece are butt or round (recorded in AC1), not
  square.

## Escalation Triggers

- The design needs a change to the column store's chunk format or to `Retain`.
- A line with per-row colour and width exceeds 8 vertex-buffer slots.

## Out of Scope

- Area marks (MSAA, `Antialias::Msaa`): not yet a story.
- Curve interpolation (monotone, step): not planned.
- A line builder: RFC-001 S10.

## Technical Tasks

- [ ] Prototype the successor binding across chunk boundaries.
- [ ] Implement `Line` and its WGSL; record findings.
- [ ] Tests for AC2–AC4; golden `line_series`.

## Dependencies

### Prerequisite Stories

- GUP-423: `Segment` Mark 📋 — `segment.wgsl`'s shading and the derive in use
  for a line-shaped mark.
- GUP-424: Multi-Layer Plot and Shared Scales 📋 — domain union, and a line
  layer over a point layer for realistic goldens.

## Testing Strategy

- **Integration tests**: AC2–AC4 pixel checks, including forced chunking.
- **Visual validation**: `gup_core/line_series`.
- **Performance**: record slot usage and bytes per row in the findings.

## Success Metrics

- [ ] A chunked time series draws as one continuous line with clean joins.

## Risk Assessment

- **High**: the cross-chunk successor may need duplicated rows per chunk or a
  storage-buffer read. _Mitigation_: measure both against AC2 and record the
  cost.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye (golden image or PNG read), if visual
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
