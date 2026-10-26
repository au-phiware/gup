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

**Initiative**: Strategic Review 2026-11 (N2 Chart Vocabulary) **Status**: ✅
Complete (2026-10-26) **Created**: 2026-10-24

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

- [x] `Curve` (Decisions) is exported from the crate root and the prelude;
      `Selection<T, Line>::curve` and `LineChart::curve` exist with doctests
      from the `gup` crate.
- [x] The migration table row says `.curve(Curve::StepAfter)` (and names GUP-551
      for smooth curves).

### AC2: Straight lines are untouched

- [x] With `Curve::Linear` (the default) a line's glue source, glue signature,
      vertex count (6) and pixels are unchanged: `line_series`, `builder_line`
      and every golden pass without re-blessing, and a test asserts the default
      layer's glue equals the glue built before this story (compare with a
      snapshot taken at the story's first commit, or assert the signature
      string).
- [x] `mask perf-budget`'s exact counters are unchanged (timing rows are
      advisory).

### AC3: Steps

- [x] Pixel tests (`crates/gup-core/tests/line_curves.rs`), each on four rows
      with distinct y: `StepAfter` has ink along `y = y_i` from `x_i` to
      `x_{i+1}` and along `x = x_{i+1}` between the two y values, and none on
      the straight diagonal's midpoint; `StepBefore` the mirror; `Step` turns at
      the midpoint x. Opaque joins show no gap (coverage ≥ 0.9 along every 0.5
      px sample of the path).
- [x] On a log y scale the step's horizontal pieces stay exactly horizontal (the
      same pixel row at both ends, ±0.5 px).
- [x] Nulls break step lines as they do straight ones; a 10-row step line in
      4-row chunks matches the one-chunk render within 8 pixels at 2/255.

### AC4: Picking, SVG

- [x] CPU and GPU picks on a step line report: `StepAfter`, row `i` for any
      point on segment `i`'s path; `StepBefore`, row `i + 1`; `Step`, row `i`
      left of the midpoint, else `i + 1` (tests).
- [x] `SvgTarget` writes a step line as one `<polyline>` with the corner points
      inserted; the rasterised SVG matches the GPU render under `svg_marks.rs`'s
      tolerance.

### AC5: Visual

- [x] Golden `gup_core/line_steps` (640×400): the same eight points drawn as
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

- [x] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30 min.
- [x] Snapshot a linear line's glue; add `Curve`, the per-layer vertex count and
      the glue constant; AC2 green; commit.
- [x] Step pieces in `line.wgsl`; AC3 tests; commit.
- [x] Picks, SVG, golden, builder setter, docs, RFC findings; commit.

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

- [x] Step lines are one setter away and straight lines cost exactly what they
      did.

## Risk Assessment

- **Medium**: the glue generator gains a per-layer specialisation that later
  marks may want too. _Mitigation_: keep it generic ("a per-layer constant in
  the signature") and record it in RFC-001.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1` and
      `cargo test -p gup-core -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye (`gup_core/line_steps`)
- [x] `mask wasm-browser` passes (glue change)
- [x] WASM size: `mask wasm-size` scatter gz bytes reported
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- **API**: `Curve` (`Linear`, `Step`, `StepBefore`, `StepAfter`;
  `#[non_exhaustive]`) in `marks.rs`, exported from the crate root and the
  prelude. `Selection<T, Line>::curve` (in `selection.rs`) and
  `LineChart::curve` (add-only in `builder/line.rs`), with doctests in gup-core
  and a "Step lines" section in the `gup` crate docs (`src/lib.rs`). The
  migration table row now names `.curve(Curve::StepAfter)` and GUP-551.
- **Mechanism** (RFC-001 "GUP-550 findings"): `GlueSpec::variant` is a per-layer
  constant. With one, the glue calls
  `vertex_variant(m, vertex_index, row, u_view, <value>u)` and appends the label
  (`curve step-after`) to the signature. Without one, the glue is byte-identical
  to the glue before the story. `Selection` holds its vertex count
  (`6 × Curve::pieces()`) and passes it to `LayerUniforms::build`. The `Mark`
  trait and the derive are unchanged.
- **WGSL** (`line.wgsl`): a shared `capsule`. `vertex_variant` draws piece
  `vertex_index / 6` between `step_point`s computed in pixel space.
  `Varyings.side` drives `hit_row` for straight and step lines alike.
- **CPU**: `Curve::path` gives the segment's 2 to 4 points. `Shape::Step` uses
  it for CPU picks (`pick.rs`), and `MarkKind::Line(Curve)` uses it for SVG
  polylines (`vector.rs`). The curve reaches `build_items` through `Chans`.
- **Joints** (after review): each pixel at a joint between two pieces is drawn
  by one piece. One is butt; the other is butt too where the two are collinear,
  or keeps the outer half of its round cap at a corner. The layer's stored row
  count (`enc.row_count`) gives the first and last segments round caps. Step
  lines add half their width to the x overhang, so the edge risers are whole.
  This folds in GUP-558, whose story file was deleted.
- **Tests**: `crates/gup-core/tests/line_curves.rs` has 11 tests (3 added after
  review: `collinear_pieces_join_without_a_seam`, `corners_are_drawn_once` and
  `edge_risers_are_drawn_whole`). The first 8: the glue snapshot against
  fixtures `line_glue.wgsl` and `line_glue_nulls.wgsl` (written at the first
  commit), paths and diagonals, log scale, chunks, nulls, CPU and GPU picks, SVG
  point counts, and the golden. There is also a `Curve::path` unit test, 3
  gup-core doctests and 1 `gup` doctest section.
  `cargo test -p gup-core -- --test-threads=1`: 593 tests ok. Root crate: all
  ok.

### Definition of Done evidence (2026-10-26)

- **Golden `gup_core/line_steps.png`** (640×400, new): three step lines over the
  same eight y values, raised by 0, 6 and 12 so all three show.
  - Orange `StepAfter` holds each level to the next x, then rises.
  - Blue `StepBefore` rises at each row's x first.
  - Green `Step` turns at the midpoints (0.5, 1.5, …).
  - Legend "curve" with the three entries; title "Step curves".

  Re-blessed after the orchestrator's review, and read at full size and at 3×
  and 8× zoom.
  - The corners are square, with round outer edges 2.5 px wide, and no corner
    has a darker rim pixel.
  - Where Step's green level runs through a row (x = 1 to 6), the stroke is
    uniform along its length, with no notch.
  - The x = 0 blue riser and the x = 7 orange riser are drawn whole, inset from
    the axes, and the line ends are round.
  - The SVG rasterisation matches the GPU render (`svg_marks::assert_matches`).

- **No other golden changed.** Only `line_steps.png` was re-blessed.
- **`mask merge-check --dogfood`**: PASS for story-ids, doc, gup-core,
  wasm-size, wasm-browser and dogfood, both before review and after the joint
  fix (HEAD `bdca61d7`).
- **Other checks**: `mask all-check` exit 0, `cargo check --examples` ok,
  `mask smoke-examples` ok. `cargo test --doc -p gup`: 51 passed after the fix.
- **`scripts/perf_budget.sh --skip-wasm`**: every exact counter passes on every
  run (`zoom.*` bytes and writes, `zoom_culled.draws.median` 14,
  `zoom_culled.columns.bytes` 0).
  - Before review, every timing passed. For example, `zoom.gpu_pass.median_ms`
    was 2.694 and `pipeline.link_create.median_ms` 1.140.
  - After the fix, three runs under load average 4 to 6 (other agents building)
    each failed a different timing row. The fails were the CPU medians
    (`zoom.cpu.median_ms` 0.839 against 0.828, `zoom_culled.cpu.median_ms`) and
    `pipeline.link_create.median_ms` (1.387, 1.133 and 1.511 against 1.332).
  - That timing links the reference Circle glue, which this story does not
    touch. `zoom.gpu_pass.median_ms` passed at 1.758, so I read the fails as
    noise.
  - The 4 WASM rows read "not measured" under `--skip-wasm`.
- **WASM scatter**: 339,778 B gz after the fix (339,448 before it), against
  338,629 B gz at main `514ec9c6` measured the same way. That is +1,149 B, under
  the 4 KB trigger.

## Retrospective

**Completed**: 2026-10-26

### Key Technical Learnings

#### A per-layer shader variant without touching the default glue

- **Challenge**: The curve had to reach `line.wgsl` and give each curve its own
  pipeline, while a straight line's glue stayed byte-identical. Library modules
  are flattened at build time, so they cannot see a constant the glue declares.
- **Solution**: Pass the constant as a literal argument to a second entry,
  `vertex_variant`, and put a label in the signature. The default variant
  (`None`) emits exactly the old text.
- **Pattern**: Snapshot the old glue _before_ changing anything. The fixtures
  were written by running the new test against the unchanged code, then
  committed alone.

#### Pick rows from one interpolated varying

- **Challenge**: The GPU pick's `hit_row` must apply three rules (nearer end,
  always this row, side of the midpoint). Overlapping pieces tie at depth 0
  inside the stroke.
- **Solution**: Add `side: f32` to `Varyings`, set per vertex and affine in
  screen space, with `hit_row = side > 0 ? row + 1 : row`. Every piece of a
  segment computes the same side for the same pixel, so ties do not matter.
- **Pattern**: Encode a pick rule as a signed linear function of position.
  Interpolation then evaluates it exactly, and the CPU mirror is one comparison
  (`Curve::picks_next`).

### Architectural Decisions

#### The curve on `Selection` for every mark

- **Decision**: `Selection` stores `curve` and `vertices` for all marks. Only
  `Selection<T, Line>::curve` can change them.
- **Reasoning**: The Decisions put the vertex count on `Selection`, and the
  glue, pick and vector paths are generic over `M`.
- **Trade-off**: Two fields that are dead for other marks.
- **Future**: GUP-551 adds a variant and pieces. Any library mark can take a
  variant by defining `vertex_variant`.

#### Polyline geometry shared through `Curve::path`

- **Decision**: One CPU description of a segment's path, which SVG and CPU picks
  read. The GPU computes the same points in `step_point`.
- **Reasoning**: GUP-439's joins and later area outlines need the same points.
  GUP-551 generalises the path from 4 points to its cubic's pieces.
- **Trade-off**: The WGSL and Rust copies must agree. `line_curves.rs` checks
  both against a third, independent path in the test.

### Development Workflow Insights

- The tests' first failures were the tests' own geometry: pixel y runs down, and
  a step's "other" corner lies on the neighbour's riser. Check expected points
  against the rendered PNG before suspecting the shader.
- A session limit hit mid-story. The coordinator's checkpoint kept the tree, and
  committing as soon as the tree built was the right reflex.
- To measure WASM size at main: `git worktree add --detach`, then run the
  checkout's own `mask wasm-size` _from that directory_. Run from elsewhere,
  mask builds the current checkout.
- I left `plot.rs`'s `pick_row` docs alone, though they could mention step rows,
  to keep clear of GUP-541's file.

#### Joints drawn once (after review)

- **Challenge**: The orchestrator saw darker rim pixels at normal size: notches
  where a level runs through a row, and dots at corners. Overlapping round caps
  blend the antialiased rim twice. A segment's pieces cannot see the neighbour
  segment, nor whether it exists.
- **Solution**: Make each pixel near a joint belong to one piece. A butt piece
  owns the pixels past its joint line, and a piece that keeps its cap drops the
  cap pixels on the butt side. Who keeps the cap follows from what each side
  knows. A 0.01 px slack turns rounding into a rare double-blended pixel, never
  a hole. The layer's row count gives the true ends round caps.
- **Pattern**: Test both checks against the old behaviour by forcing every joint
  to round caps; both fail (0.098 along the seam, 0.29 at a corner). Over
  blending is exact where two straight edges cross at right angles, because
  their uncovered fractions multiply.

### Follow-up Stories

None. GUP-558 (edge risers) was folded into this story and its file deleted.
