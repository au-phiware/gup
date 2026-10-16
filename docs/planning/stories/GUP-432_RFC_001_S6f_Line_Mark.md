---
implementer: opus
implementer-reason:
  design work - binding each row with its successor across column chunks with
  per-chunk origins, within the vertex-buffer slot budget, plus join geometry
---

# GUP-432: RFC-001 Step S6f: `Line` Mark

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-17)
**Created**: 2026-10-16

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

- [x] A "GUP-432 findings" section appended to RFC-001 records how the successor
      is bound (including across chunks with different origins), the slot count
      for a line with per-row colour and width, the join style and why, and the
      append behaviour.

### AC2: Connected rows

- [x] `gup_core::Line` (derive) draws `N - 1` segments for `N` rows; a test with
      `max_chunk_rows(4)` and 10 rows has no gap at the chunk boundaries: every
      pixel on the polyline's centreline (sampled every 0.5 px) has coverage ≥
      0.9.

### AC3: Joins

- [x] A "V" at 30° and a shallow 170° turn, width 6 px, show no wedge gap
      (centreline-and-outer-edge samples covered) and, for an opaque colour, no
      visible seam; the join style's behaviour with translucent colour is
      documented.

### AC4: Null rows break the line

- [x] A series with a NaN in the middle draws two disconnected pieces: no ink
      within `width` of the straight line between the null's neighbours, except
      where the pieces themselves are.

### AC5: Golden

- [x] Golden `gup_core/line_series`: a 200-point time series, width 2 px, with
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

- [x] Prototype the successor binding across chunk boundaries.
- [x] Implement `Line` and its WGSL; record findings.
- [x] Tests for AC2–AC4; golden `line_series`.

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

- [x] A chunked time series draws as one continuous line with clean joins.

## Risk Assessment

- **High**: the cross-chunk successor may need duplicated rows per chunk or a
  storage-buffer read. _Mitigation_: measure both against AC2 and record the
  cost.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (golden image or PNG read), if visual
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **`gup_core::Line`** (`crates/gup-core/src/marks.rs`), derived with
  `#[mark(module = "gup::marks::line", geometry = Polyline)]`. Its channels are
  `x`, `y` (roles `X`, `Y`), `width: Px` (default 1.5) and `color: Color`, and
  it is in the prelude. `crates/gup-wgsl/library/line.wgsl` draws each segment
  as a capsule (`line_quad` with `cap = width / 2`, coverage
  `span(distance to the segment, width / 2)`), which gives round caps and round
  joins.
- **`Geometry`** (`channel.rs`) is `Quad` or `Polyline`, read through
  `Mark::GEOMETRY`. The derive (`gup-core-macros/src/mark.rs`) accepts
  `geometry = Polyline`. It checks that the input struct ends with
  `<channel>_next` for each position channel, and errors otherwise (new trybuild
  case `derive_mark_fail/polyline_without_next`).
- **Glue** (`shader/glue.rs`). For a polyline, each position channel also gets
  `m.<ch>_next`: the same chain over `col.<ch>_next` with
  `chunk.<ch>_next_base`. The next-row vertex columns sit at the locations after
  the columns (`Glue::successors`). With nulls, the glue reads a second validity
  binding (2) and the `Chunk` members `valid_offset` and `next_offset`. The
  `Quad` glue is unchanged, and its fixtures are byte-identical.
- **Draws** (`order.rs`, `selection.rs`, `render.rs`). `draw_entries` lists one
  entry per chunk, plus a bridge per chunk boundary for a polyline.
  `polyline_spans` draws them in data order. `ChunkDraw` binds a buffer and
  range per slot (`slots`) and carries `next_validity`. `chunk_uniform_bytes`
  writes each entry's row base, validity offsets and bases, taking the next-row
  base from the next chunk in a bridge. A polyline never sorts by size. Prepare
  now errors, for any mark, when its vertex columns exceed `max_vertex_buffers`,
  or when a polyline with nulls lacks two vertex-stage storage buffers.
- **Tests**: 6 in `crates/gup-core/tests/line_png.rs` (AC2–AC5 and the
  translucent-join behaviour). Crate tests cover Line's input struct, bridge
  `Chunk` entries, spans and slot bindings, a cleared validity bit breaking a
  line across chunk boundaries, and append. The `gup-core`, `gup-core-macros`
  and `gup-wgsl` tests pass: 263 tests, run single-threaded.
- **Docs**: RFC-001 "GUP-432 findings". `PERF_BUDGETS.md` re-records the WASM
  scatter at 321,457 B gz (+4,474).

### Definition-of-Done evidence

- **Golden** `tests/golden/gup_core/line_series.png` (new), read by eye. It
  shows 200 points at 2 px in 64-row chunks (three bridges), with a gap at rows
  120–125: a smooth line, round joins, and a clean break with round caps either
  side. Crops at 8× around rows 64 and 128 show no seam or gap at the chunk
  boundaries.
- **Seeded checks.** Dropping the bridges fails AC2 and the chunked null test
  (coverage 0). Decoding a bridge's next row with its own chunk's origin fails
  AC2 and the chunked null test. Butt ends fail AC3. Ignoring the next row's
  validity bit fails `a_cleared_validity_bit_breaks_a_line`; the NaN image tests
  pass regardless, because Mesa culls NaN vertices.
- `mask all-check` passes. `cargo check --examples` passes.
  `mask visual-regression` passes. `mask old-path-loc` is 28,910.
  `mask wasm-size`: scatter 321,457 B gz, gup-core 279,744 B gz over bare wgpu
  (pass).
- `mask wasm-browser` fails on
  `GUP FAIL diverging: (2.0, -0.424…) drew [199, 224, 237], not the mirror's [196, 222, 236]`
  (SwiftShader). The parent tree (main `692375f`) fails identically, so the
  failure predates this story.
- The root-crate tests were left to CI, as the orchestrator asked.
  `mask perf-budget` was not run: the load average stayed above 4. No `gup-core`
  scatter path is affected beyond shared code.
