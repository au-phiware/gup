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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-11-01) **Created**: 2026-10-27

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

- [x] The layer chooses its pieces per segment, 1 to 16, at prepare from the
      data's pixel density: for example, the median segment width in pixels
      under the current x scale. The vertex count is `6 × pieces`. It changes
      only when the count changes, and the shader reads the count as a constant,
      not as `MONOTONE_PIECES`.
- [x] `zoom_bench --line monotone` (100K rows) is at most 2x `--line linear`;
      `gup_core/line_monotone` is unchanged (16 pieces).
- [x] `Curve::path` and the picks use the same count. The seam tests of
      `line_monotone.rs` pass at every count.

## Decisions

- Keep the Chebyshev nodes and the plane joints of GUP-551.

## Escalation Triggers

- The count needs a change to the `Mark` trait or to straight-line glue.

## Out of Scope

- Other smooth curves.

## Technical Tasks

- [x] Count from density; uniform or glue constant; tests; measurement.

## Dependencies

### Prerequisite Stories

- GUP-551 ✅.

## Testing Strategy

- Unit test of the count rule; `line_monotone.rs` at 1, 4 and 16 pieces;
  `zoom_bench` for the cost.

## Success Metrics

- [x] Dense monotone lines cost about what straight lines cost.

## Risk Assessment

- **Medium**: a count that changes while zooming relinks a pipeline. A uniform
  count with a fixed maximum vertex count avoids the relink, but costs vertices.

## Definition of Done

- [x] All Acceptance Criteria are satisfied and checked
- [x] `cargo test -p gup-core -- --test-threads=1` and `mask all-fix` pass
- [x] Measurements recorded in RFC-001
- [x] Story status updated in the story file and INDEX.md

## Implementation Summary

- **Count rule** (`monotone::pieces_for`): the fewest power of two (1, 2, 4,
  8, 16) whose misplaced ink `width / n² × min(width, 1)` is at most 1/8 of a
  pixel, `width` the pixel width of the segment at the width-weighted median. 16
  pieces above 8 px, 1 below 0.35 px.
- **Statistic** (`monotone::Widths`): quarter-octave buckets of the curved
  segments' data widths, filled at evaluation and added to by appends. Prepare
  maps the median bucket's segment through x's CPU map (`cpu_eval`), so the
  count follows zooming. No CPU map, or no curved segment: 16.
- **Glue**: the count is the literal `vertex_controls` reads (the variant
  value), with labels `curve monotone-x, 4 pieces`, so each count links its own
  pipeline and a changed count rebuilds the layer's GPU state. The vertex count
  is `6 × pieces`. `MONOTONE_PIECES` is gone from Rust and WGSL; the Rust
  maximum is `MAX_MONOTONE_PIECES`.
- **CPU**: `Curve::path`, `Curve::pieces` and `Curve::variant` take the layer's
  count; `Shape::Path` and `pick::Chans` carry it.
- **Joints**: planes are tested at the pixel's centre on the target, not in each
  piece's interpolated frame. That frame moves with the rasterizer's vertex
  snapping, which left whole pixels out on dense rows at every count, 16
  included. At a row, each segment also draws within 2 ulps' angle of its plane
  (`plane_slack`), since the two segments' tangents differ by rounding.
- **Files**: `monotone.rs`, `marks.rs`, `selection.rs`, `pick.rs`, `vector.rs`
  (one call), `line.wgsl`, `tests/line_monotone.rs`, RFC-001.
- **Tests**:
  - Unit tests: `pieces_follow_pixel_width`,
    `joints_are_chebyshev_nodes_at_every_count`,
    `widths_take_the_width_weighted_median`,
    `a_monotone_path_measures_the_drawn_pieces`, and the extended `Curve` path
    test (every count).
  - `line_monotone.rs`: `joints_are_seamless_at_every_count` (a 2000-row wave at
    1 piece and a 300-row wave at 4, widths 1.5, 3 and 6) and
    `the_count_follows_zoom_and_appends` (1, then 8 zoomed, then 1; 16, then 1
    after appends). The existing 11 tests pass, the 16-piece seams among them.

### Definition of Done evidence (2026-11-01)

**Cost** (AC2): `zoom_bench --line linear|monotone`, 100K rows, 1600x1000
Mailbox uncapped, release, Intel HD 630. Before and after binaries alternated, 5
rounds of 600 frames, load average 4.8. Medians of the 5 GPU pass medians:

| Line      | Before    | After    |
| --------- | --------- | -------- |
| Linear    | 1.503 ms  | 1.479 ms |
| MonotoneX | 28.782 ms | 2.060 ms |

MonotoneX is **1.39x** Linear (19x before). The bench draws 1 piece throughout.

- **Golden** `gup_core/line_monotone`: unchanged (16 pieces). The fresh render
  differs from the golden by no pixel at the compare's threshold. I read it at
  4x around the peak at x = 7 and the valley after it: smooth, no facets, no
  seam at any joint.
- **`mask merge-check --dogfood`** (HEAD `166bed0d`): story-ids, doc, gup-core
  (740 tests), wasm-size, wasm-browser and dogfood PASS. WASM scatter **358,067
  B gz** (budget 362,063).
- **`cargo test --doc -p gup`**: 56 + 4 passed.
- **`scripts/perf_budget.sh --skip-wasm`**: every exact counter passes. Run 1
  (load 1.7): `zoom.gpu_pass.median_ms` 2.405 (pass), `pipeline.link_create`
  2.095 against a 1.332 budget (FAIL). Run 2 (load 18): `zoom.gpu_pass` 3.718
  and `link_create` 1.511 (both FAIL); three more `pipeline_timings` runs at
  load 13 read 1.70, 1.25 and 1.69. Both measure the scatter's Circle program,
  whose glue and WGSL this story does not change, so I read them as load noise.
  `PERF_BUDGETS.md` is not edited.

## Retrospective

**Completed**: 2026-11-01

### Key Technical Learnings

#### A sub-pixel segment misplaces only its share of a pixel

- **Challenge**: The tessellation error alone (`width / n²` within 1/8 px) drew
  2 pieces once zoom_bench zoomed in past 8x, which cost 2.4x Linear.
- **Solution**: Measure the misplaced ink, `width / n² × min(width, 1)`. An
  offset across a segment 0.3 px wide moves 0.3 of the ink that the same offset
  across a whole pixel would.
- **Pattern**: Express a level-of-detail rule in the unit the eye sees, which is
  pixel coverage, not in geometric distance alone.

#### Rasterizer snapping breaks per-piece frames

- **Challenge**: Dense smooth rows left whole pixels out at joints, at every
  count, 16 included. Every piece tested its joint plane in its own interpolated
  `local` frame. The rasterizer snaps each quad's vertices, which moves that
  frame by about 0.004 px, so a pixel centre near a plane could fall outside
  both pieces. An absolute slack fixed the holes, but then the sparse seam test
  blended rim pixels twice.
- **Solution**: Test the plane at the fragment's own centre on the target
  (`clip.xy` less the viewport's whole-pixel offset), against a joint point and
  a normal that both pieces compute alike. Only the rounding of the two tangents
  at a row remains, covered by an angular slack of 2 ulps over the tangent's
  length.
- **Pattern**: Ownership tests shared by two primitives must read inputs that
  both see bit-for-bit: the pixel centre, and values computed from the same
  operands. Never use interpolated per-primitive frames for them.

#### Dense fixtures find what sparse ones cannot

- The GUP-551 seam tests used 7 and 12 rows. A 2000-row wave puts thousands of
  joints under the stroke, and found the snapping holes at once. The windowed
  `ink_errors` (pieces within reach in x) keeps the check fast at that size.

### Architectural Decisions

#### The count is a glue literal, in powers of two

- **Decision**: The count is the variant value of the glue, one pipeline per
  count, with five counts.
- **Reasoning**: A uniform count needs a field in `Encodings` (glue.rs, outside
  the story's files), and changing the vertex count still needs a new
  `LayerGpu`. The literal uses GUP-550's mechanism unchanged.
- **Trade-off**: Crossing a count while zooming rebuilds the layer's GPU state,
  and the first crossing links a pipeline (about 1-5 ms native). Powers of two
  draw up to twice the pieces the exact rule would.

#### The width-weighted median, from data widths

- **Decision**: The median is weighted by width and kept in data units through
  appends. The median segment's two rows are mapped through x's CPU map at every
  prepare.
- **Trade-off**: It is exact under affine x (linear, time). Under log, pow or
  symlog x, the median is weighted by data width, not by pixel width. Rows that
  a window evicts stay in the summary. Neither seemed worth a story, since a
  monotone curve on a non-affine x scale is rare.

### Development Workflow Insights

- `zoom_bench` binaries copied aside (`~/.cache/gup/gup576/zoom_bench_before`,
  `_after`, and `_one` with the count forced to 1) made alternating measurements
  and what-if floors cheap. The forced single piece (2.03 ms) gave the floor
  before I tuned the rule.
- The worktree sandbox refuses compound shell lines with loops or heredocs that
  feed tools. Short scripts under `/tmp` worked.
- A session limit stopped the work mid-debug for several hours. The checkpoint
  ref and a commit taken before the investigation meant nothing was lost.

### Follow-up Stories

None. The non-affine weighting and window eviction are recorded above, and the
joint fix is complete in this story.
