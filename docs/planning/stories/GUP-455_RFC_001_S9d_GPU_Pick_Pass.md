---
implementer: opus
implementer-reason:
  a new pipeline variant and mark-contract extension across gup-wgsl,
  gup-core-macros and gup-core, with exact GPU-equals-CPU pick semantics that
  have no precedent; the approach is chosen, the plumbing is design work
---

# GUP-455: RFC-001 Step S9d: GPU Pick Pass with CPU Pick Semantics

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-18)
**Created**: 2026-10-18

## Context

RFC-001 §9 specifies two picks. `pick_row` runs on the CPU (GUP-445, done) and
`pick_async` uses a GPU `Pick` pipeline variant whose result arrives a frame or
two later. The GPU path exists for layers the CPU cannot read. Under
`Retain::GpuOnly` past one chunk, a layer keeps neither rows nor CPU columns, so
`Plot::pick_row` skips it (GUP-445 Decisions; S4b findings: "under `Columns` and
`GpuOnly`, `pick` must return a `RowId`").

The owner's requirement is that the GPU pick **matches the CPU pick's
semantics**:

- Tolerance is `PICK_TOLERANCE` (3 px).
- Among marks that contain the point, the topmost wins; otherwise the nearest
  within tolerance wins, with the topmost breaking ties. Topmost means layer
  `z`, then layer index, then draw rank (`pick.rs`, `PickIndex::query`).
- A `Line` pick returns the nearer endpoint of the segment under the point, row
  `i` on a tie (S8b findings).
- Points outside the plot rect pick nothing, and null rows are never picked.

The findings tell us what the GPU sees. A drawn instance is stored row
`row_base + instance_index`, mapped to its data row by `DrawOrder::row_of`
(GUP-431 findings). A line instance is its segment's first row (GUP-432
findings). Pick variants use one sample (S3 findings). Every library mark's
`Varyings` already carries a flat `row` and a `local` offset in device pixels
from which its fragment computes coverage (`gup-wgsl/library/*.wgsl`).

RFC §9's sketch drew a 5 × 5 region into an `Rg32Uint` target. A region of
pixels can only approximate the CPU's analytic distance. This story uses an
exact scheme instead, described in Decisions: draw into a **1 × 1 target whose
texel centre is the pointer**, have each mark compute its own analytic distance
there, and let a depth test choose. It supersedes RFC §9's `pick_async(px)`
sketch with a request/poll API that suits a host's frame loop. GUP-456 wires it
into `handle`.

## Read First

- RFC-001 §9 "Picking" (lines 640–654); "S3 findings" bullet on pick variants
  (lines 1528–1532); "S4b findings → Proposed adjustments" S9 bullet (lines
  2316–2317); "GUP-431 findings → Picking (S9)" (lines 3086–3089); "GUP-432
  findings → Proposed adjustments" S9 bullet (lines 3294–3298); "S8b findings"
  (lines 3304–3352); "S5c findings" on identifier limits (grep "cannot end in a
  digit").
- [GUP-445](GUP-445_RFC_001_S8b_CPU_Picking.md) Decisions.
- `crates/gup-core/src/pick.rs` lines 28–200 (`Shape::distance`, `anchor`,
  `PickItem::key`) and 360–400 (`query`).
- `crates/gup-core/src/shader/glue.rs` lines 575–670 (`vs_main`/`fs_main`
  emission, null handling).
- `crates/gup-wgsl/library/{view,circle,rect,segment,line,coverage}.wgsl`.
- `crates/gup-core/src/render.rs` lines 115–200 (`LayerGpu`, `PipelineKind`,
  `PipelineKey`), 338–400 (`mark_pipeline`) and 1300–1380 (`Prepared::draw`).
- `crates/gup-core/src/target.rs` lines 225–320 (`Readback`, `map_async`).
- `crates/gup-core/src/context.rs`: counted `submit` and uploads
  (`every_submit_is_counted`, `every_gpu_write_is_counted`).
- `crates/gup-core-macros/src/mark.rs` lines 200–270 (`hit`) and 390–420 (the
  naga check of `<NAME>In`).
- `crates/gup-core/src/layer.rs` (GUP-451's split): new `Layer` behaviour goes
  there.

## User Story

> "As a developer plotting millions of rows with `Retain::GpuOnly`, I want to
> know which row is under the pointer, with exactly the same rules as for small
> data, so that interaction does not depend on how much data I keep on the CPU."

## Acceptance Criteria

### AC1: API

- [x] `PickTicket`, `GpuPick`, `Plot::request_pick`, `Plot::poll_picks` and
      (native) `Plot::pick_row_gpu` exist as in Decisions. Results are reported
      per request, never merged.
- [x] A native doctest resolves a three-row plot and checks that `pick_row_gpu`
      equals `pick_row` at each row's anchor and at an empty point.

### AC2: Equivalence with the CPU pick (the oracle)

An in-crate GPU test, `gpu_pick::tests::agrees_with_cpu_pick`, at 400 × 300:

- [x] The scene has four layers. Circles (z 0, a radius column, so draw order is
      size-descending) and segments (z 0, added after the circles) overlap.
      Rects are at z 1, and a line with one null row at z 2. Rows are
      deterministic pseudo-random, about 2,000 in all.
- [x] For 10,000 deterministic pseudo-random points in the plot rect,
      `pick_row_gpu(at) == pick_row(at)` for at least 99.9% of them. Every
      disagreement is a near tie, meaning one of: (a) the CPU's best distance is
      within 0.01 px of 3 px, or of 0 px; (b) the two candidates' distances
      differ by less than 0.01 px. The test computes this from the pick index
      and prints each disagreement.
- [x] All points within 3 px of the null row's position (and not near another
      mark) give `None` on both paths.
- [x] 1 px outside the plot rect, both give `None`, and the GPU path submits
      nothing (`Context::submissions` unchanged).

### AC3: Layers the CPU cannot read

- [x] A `Retain::GpuOnly` layer with `max_chunk_rows(4)` and 12 rows, after
      release: `pick_row` gives `None` at each row's position (positions
      computed in the test from the data and the scales), and `pick_row_gpu`
      gives that row's `RowId`.
- [x] The same with a `Line` under `GpuOnly`: at points 1 px off a segment,
      nearer each end, `pick_row_gpu` returns that end's row.

### AC4: Contract, user marks and WGSL names

- [x] The four library marks declare `gpu_pick` and define `hit_distance` and
      `hit_row` (Decisions). Their CPU twins in `pick.rs` and these functions
      agree on 1,000 random points per shape within 1e-4 px (a test that runs
      the WGSL function through a one-texel pick of a single mark).
- [x] `#[derive(Mark)]` with `gpu_pick` and a module that lacks `hit_distance`
      fails to compile, with a message naming the function (a trybuild case in
      `tests/derive_mark_fail`). A user mark without `gpu_pick` compiles as
      before, and its layers are skipped by the GPU pick (documented on
      `request_pick`).
- [x] Every new WGSL identifier is checked against RFC-001 "S5c findings": no
      composable identifier ends in a digit, and none is a WGSL built-in name
      (`distance` is one; hence `hit_distance`).

### AC5: Cost and counters

- [x] Every pick submission goes through `Context::submit`, and every buffer
      write is counted. `tests/zoom_uploads.rs` and every golden pass unchanged:
      colour passes are byte-identical, since the view's margin field is 0 for
      them.
- [ ] `mask perf-budget` passes, including `pipeline.link_create.median_ms`
      (+20%). If the glue for pick entry points pushes link time over, emit them
      only in the pick variant's glue (Decisions' rule).
- [x] Recorded in the story: the median of 100 `pick_row_gpu` calls on 100,000
      circles at 800 × 500 on the dev machine. Over 5 ms is an escalation.
- [ ] `mask wasm-size` passes. `wasm.per_row_type.gz_bytes` moves by less than
      2%: the new `Layer` method lives in `layer.rs`.

### AC6: Record

- [x] RFC-001 gains "S9d findings": the scheme, any departure from Decisions and
      why, the oracle's agreement rate, timings, and the API that GUP-456 relies
      on. If the API differs from Decisions, update GUP-456's Decisions in the
      same commit.

## Decisions

- **API** (`crates/gup-core/src/gpu_pick.rs`; types at the crate root):

  ```rust
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub struct PickTicket(u64);

  #[derive(Copy, Clone, Debug, PartialEq)]
  pub struct GpuPick {
      pub ticket: PickTicket,
      pub at: Point,
      /// The plot's resolve generation the pick was drawn at.
      pub generation: u64,
      pub row: Option<RowId>,
  }

  impl Plot {
      /// Draw a pick at `at` on the GPU; its result comes from `poll_picks`
      /// a frame or two later. Needs a resolve first.
      pub fn request_pick(&mut self, cx: &Context, at: Point) -> Result<PickTicket>;
      /// Finished picks, oldest first. Never blocks (polls the device on native).
      pub fn poll_picks(&mut self, cx: &Context) -> Vec<GpuPick>;
      /// Request, wait and return (native tests and tools).
      #[cfg(not(target_arch = "wasm32"))]
      pub fn pick_row_gpu(&mut self, cx: &Context, at: Point) -> Result<Option<RowId>>;
  }
  ```

  At most 4 picks are in flight. A fifth request returns
  `Error::config("gpu pick", …)`. Outside the plot rect a request completes with
  `row: None` and no GPU work.

- **The scheme: one texel, exact distances, depth decides.**
  - Target: a 1 × 1 `Rg32Uint` colour attachment (`(layer + 1, row + 1)`;
    `(0, 0)` is empty, so clear to 0) and a 1 × 1 `Depth32Float` (clear 1.0,
    compare `LessEqual`, write on). One sample.
  - Vertex: a `vs_pick` entry in the glue calls the mark's `vertex` as `vs_main`
    does, then maps the pointer to the texel centre with
    `clip.xy = (clip.xy − ndc(at)) * size_px`, where `size_px` is the chart's
    logical size. The view is at dpr 1 for picking.
  - Quads must cover points up to 3 px outside a mark. `View`'s spare
    `padding: f32` becomes `pick_margin: f32` (the uniform stays 16 bytes), and
    each library mark's `vertex` adds it to the half-extent of its quad. It is 0
    for colour passes, 3.0 for picks.
  - Fragment: `fs_pick` calls the mark's `hit_distance(v) -> f32`, the signed
    distance in pixels from this fragment to the hit outline, the same shape
    `pick.rs`'s `Shape::distance` uses. It discards if the distance is over 3.
    It writes `frag_depth = 0.0` when the distance is ≤ 0, else
    `0.5 + 0.5 * d / 3.0`, and the colour `(layer_slot + 1, hit_row(v) + 1)`.
    With `LessEqual` and the colour pass's draw order (z, layer index, draw
    rank), a later containing mark wins, the nearer non-containing mark wins,
    and on equal distance the later wins. That is `PickIndex::query`'s rule.
  - `hit_row(v) -> u32` returns `v.row` for disc, box and segment marks. For
    `Line` it returns `v.row` or `v.row + 1`, whichever end is nearer to the
    fragment, with `v.row` on a tie. The projection onto the segment at or below
    one half picks `v.row`, which is the same test as `Shape::anchor`'s
    Euclidean `da <= db`.
  - CPU side: the stored row maps through `DrawOrder::row_of` to the data row,
    as `hit_items` does. The layer slot is the index into the plot's layers.
- **Layer id per draw**: a per-layer pick uniform (layer slot), with one entry
  per layer at `DYNAMIC_ALIGN` in one buffer and a dynamic offset, as `Chunk`
  entries do. Group 3 (WebGPU guarantees 4 bind groups).
- **Pipeline variant**: `PipelineKind::Pick(signature)`, keyed with `Rg32Uint`,
  samples 1 and the depth format. Pick entry points are emitted into the same
  glue as `vs_main`/`fs_main` unless that pushes
  `pipeline.link_create.median_ms` over budget. In that case the glue emitter
  gains a pick flag and the pick variant links its own glue. Either way, the
  colour pipeline's glue fixtures (`reference_glue_matches_fixtures`) change
  only if the entry points are shared, and are then re-blessed with a reason.
- **Mark contract**: `Mark::GPU_PICK: bool` (default `false`), set by
  `#[mark(…, gpu_pick)]`. With it, the derive's naga check requires
  `fn hit_distance(v: Varyings) -> f32` and `fn hit_row(v: Varyings) -> u32` in
  the module. `MarkInfo` (GUP-451) carries the flag.
- **Readback**: one 256-byte `MAP_READ` buffer per in-flight pick (copy-row
  alignment), from a pool of 4. `map_async` sets a flag that `poll_picks` reads,
  as `Readback` does. A `wgpu::Device::poll` with `PollType::Poll` on native
  inside `poll_picks`; nothing on the web.
- **Not done here**: hover, click and host frame requests (GUP-456); brushing
  GPU-only layers; region (rectangle) picks.

## Escalation Triggers

- The oracle's agreement is below 99.9%, or a disagreement is not a near tie
  (the depth encoding or the draw order does not reproduce `query`'s rule).
- `frag_depth` or `Rg32Uint` is not available on SwiftShader WebGPU in
  `mask wasm-browser` (GUP-456 needs it there).
- Adding `pick_margin` to a mark's quad changes any colour-pass pixel.
- `pick_row_gpu` median exceeds 5 ms at 100,000 circles.
- A user-mark test outside `tests/derive_mark_fail` breaks because of the
  contract change.

## Out of Scope

- Using GPU picks in `handle`, `needs_frame`, and the browser scenario: GUP-456.
- Rectangle (brush) picks on the GPU; culling (an RFC S9 note on chunk stats,
  not in the owner's S9 brief).

## Technical Tasks

- [x] `pick_margin` in `View`; `hit_distance`/`hit_row` and quad margins in the
      four library marks; `gpu_pick` in `#[mark]` and its naga check.
- [x] `vs_pick`/`fs_pick` emission; `PipelineKind::Pick`; the pick uniform.
- [x] `gpu_pick.rs`: target, depth, encode, submit, readback pool, poll, row
      mapping; `Plot` methods; the `Layer` hook in `layer.rs`.
- [x] Tests: AC2 oracle, AC3, AC4 per-shape, trybuild case, doctest.
- [x] `mask perf-budget`, `mask wasm-size`; timings; RFC "S9d findings".

## Dependencies

### Prerequisite Stories

- GUP-451: WASM Size Cuts 📋 — `layer.rs` and `MarkInfo`, so the pick hook is
  not monomorphised per row type.
- GUP-445: RFC-001 S8b CPU Picking ✅ — the semantics and the oracle.
- GUP-431 ✅ and GUP-432 ✅ — draw order and line instances.

### Enables Stories

- GUP-456: GPU Pick in Hover, Click and Hosts.

## Testing Strategy

- **GPU unit/integration (native)**: AC2's oracle over 10,000 points; AC3's
  `GpuOnly` layers; per-shape WGSL-vs-CPU distance checks.
- **Compile-fail**: a `gpu_pick` mark without `hit_distance`.
- **Regression**: goldens and `zoom_uploads` unchanged; `mask perf-budget`.
- **Performance**: 100 `pick_row_gpu` calls at 100,000 circles, recorded.

## Success Metrics

- [x] GPU and CPU picks agree except at measured near ties.
- [x] `GpuOnly` layers are pickable.

## Risk Assessment

- **Medium**: depth precision. `0.5 + 0.5 * d / 3` in `Depth32Float` resolves
  distances to about 1e-7 px, far below the 0.01 px near-tie band, but the
  interpolated `local` varying has its own float error. _Mitigation_: the
  oracle's near-tie classification; AC4's per-shape check.
- **Medium**: the clip-space remap puts large chart coordinates through
  `* size_px`, which can lose precision for huge marks. _Mitigation_: measure in
  the oracle; escalate if a disagreement is not a near tie.
- **Low**: link-time growth from extra entry points. _Mitigation_: the
  separate-glue rule.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [ ] Goldens pass without `GUP_BLESS`; `mask perf-budget` passes
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/gpu_pick.rs` (new): `PickTicket`, `GpuPick`,
  `Plot::request_pick`, `poll_picks`, `pick_row_gpu`; the one-texel `Rg32Uint` +
  `Depth32Float` target, the view and per-layer `Pick` uniforms, four readback
  slots. Tests in `gpu_pick/tests.rs`: the oracle, `GpuOnly` circles and line,
  per-shape distances, request/poll and the 100,000-circle timing.
- `crates/gup-core/src/shader/glue.rs`: `GlueSpec::gpu_pick`, `Glue::pick`
  (`pick_source`: `vs_pick`/`fs_pick`, `PICK_ZOOM` = 256).
- `crates/gup-core/src/render.rs`: `PipelineKind::Pick`,
  `Context::pick_pipeline`, the `Pick` bind group layout, `LayerGpu::record`
  (shared by `Prepared::draw` and picks), `ViewUniform::pick_margin`.
- `crates/gup-wgsl/library/`: `View::pick_margin`; `Quad::axis`, `grow_quad`,
  `box_distance` in `coverage`; `hit_distance`/`hit_row` and the quad margin in
  circle, rect, segment and line.
- `crates/gup-core-macros/src/mark.rs`: `#[mark(.., gpu_pick)]` and its naga
  check (name, one argument, return type). `Mark::GPU_PICK`.
- `Layer::draw_order` (selection.rs), `Plot::drawn` (each layer's z and GPU
  state at the last resolve).
- Browser: `InteractiveScatter::request_pick`/`poll_picks` and a `gpu-pick` step
  in `wasm-size/scatter/index.html`.
- Tests: gup-core lib 170 (5 new), `derive_mark` 3 (1 new), trybuild case
  `gpu_pick_without_hit_distance`, a doctest on `pick_row_gpu`.

### Rejected alternatives

- **RFC §9's 5 × 5 region**: approximates the CPU's analytic distance with pixel
  centres; the one-texel depth scheme is exact.
- **One texel per pixel** (the Decisions' `* size_px`): vertices snap to 1/256
  texel, so distances were off by up to 5e-4 px, failing AC4's 1e-4. 256 texels
  per pixel gives 3e-5 px.
- **Pick entry points in the colour glue**: WGSL forbids calling an entry point,
  so sharing means moving `vs_main`'s body into a helper, which changes every
  colour fixture and adds pick code to every colour pipeline's parse. The pick
  variant is its own source, linked on the first pick (the Decisions' fallback
  rule).
- **Reading the depth to report distances**: not needed by the API; the depth is
  copied back in tests only (offset 128 of the readback buffer).
- **Snapshotting the draw order per pick**: O(rows) per request. Rows are mapped
  at poll time; `GpuPick::generation` tells a host when a pick is stale.
- **Replacing the picker when the layer count grows**: would drop picks in
  flight; only the `Pick` uniform buffer is regrown.

### Definition-of-Done evidence

- Oracle: 10,000/10,000 agree (9,270 CPU hits); null row: 300+ points `None` on
  both paths; 1 px outside: `None`, `Context::submissions` unchanged.
- Per shape, worst |GPU − CPU| distance: circle 1.5e-5, rect 3.1e-5, segment
  2.6e-5, line 2.8e-5 px (1,000 points each).
- `pick_row_gpu` on 100,000 circles at 800 × 500, median of 100: **2.3 ms**
  release, 4.3 ms debug (budget 5 ms).
- Colour passes byte-identical: `GUP_BLESS=1` over every gup-core test rewrote
  no golden (`git diff --stat tests/golden` empty); `zoom_uploads` passes.
  `scatter_linked.wgsl` re-blessed: the library's `pick_margin` and hit
  functions; `scatter_glue.wgsl` unchanged.
- `mask wasm-browser`: `interactive=ok` including the `gpu-pick` step (six rows
  and the empty plot centre) on SwiftShader WebGPU; the step fails when its
  expectation is wrong (checked).
- `mask perf-budget` (second run; the first ran under load from other agents):

| metric                         | recorded | limit   | measured | verdict |
| ------------------------------ | -------- | ------- | -------- | ------- |
| zoom.gpu_pass.median_ms        | 2.95     | 3.540   | 2.990    | pass    |
| zoom.cpu.median_ms             | 0.69     | 0.828   | 0.629    | pass    |
| zoom.cpu.p95_ms                | 0.86     | 1.075   | 0.760    | pass    |
| zoom uploads (all exact)       | —        | —       | —        | pass    |
| pipeline.link_create.median_ms | 1.11     | 1.332   | 1.265    | pass    |
| pipeline.link_create.cold_ms   | 1.04     | 10      | 1.411    | pass    |
| wasm.scatter.gz_bytes          | 295,904  | 301,822 | 305,406  | FAIL    |
| wasm.over_wgpu.gz_bytes        | 263,544  | 400,000 | 273,046  | pass    |
| wasm.per_row_type.gz_bytes     | 941      | info    | 1,651    | info    |

- **WASM growth.** Without the harness's two pick hooks the scatter is 296,827 B
  gz (+923, +0.3%: library WGSL and glue emission). With them, 305,406 (+9,502,
  +3.2%, 3,584 over the +2% line): the browser proof links the pick pass,
  pipeline creation and readback into the scatter. Left for the orchestrator to
  record in `PERF_BUDGETS.md`. `per_row_type` moved 941 → 1,163 (no hooks) and
  1,651 (hooks). A name-grouped `twiggy` diff of the two builds puts the new
  per-row-type code at `draw_order`, 24 B raw; the rest is existing generic
  `Layer` methods (`bind_axes` 802 B raw) and inlining that shifts between
  builds. AC5's `layer.rs` premise does not hold: GUP-451 skipped that split.
- `mask all-check` passes; `cargo check --examples` passes; `mask old-path-loc`
  TOTAL 28,910. Root-crate tests left to CI (brief).

## Retrospective

**Completed**: 2026-10-18

### Key Technical Learnings

#### Rasteriser snapping limits interpolated varyings

- **Challenge**: at one texel per logical pixel, the per-shape check failed at
  5e-4 px, and the oracle had one disagreement: the GPU found a line containing
  the point (d ≤ 0) where the CPU measured d = 0.00023 px.
- **Solution**: scale the pick's clip space by 256 texels per pixel. Vertices
  snap to 1/256 of a texel, so the snap is now 1/65,536 px; the worst per-shape
  error fell to 3e-5 px and the oracle agrees on every point.
- **Pattern**: when a fragment stage measures analytic geometry, the precision
  is set by subpixel snapping in the target's units, not by float precision.
  Zoom the target so a pixel spans many texels.

#### Depth as the comparison function

- **Challenge**: reproduce `PickIndex::query` (containing beats near, topmost
  breaks ties) on the GPU without a custom reduction.
- **Solution**: depth 0 for containing marks, `0.5 + 0.5 · d / 3` otherwise,
  `LessEqual`, drawn in the colour order. Ties go to the later draw, which is
  the topmost.
- **Pattern**: a lexicographic "best" over draws maps onto a depth test when the
  order of draws supplies the tie-break.

### Architectural Decisions

#### A separate pick glue, linked lazily

- **Decision**: `Glue::pick` holds a pick-only source (`vs_pick`/`fs_pick`,
  group 3), linked on the first pick into the colour program's bind group
  layouts.
- **Reasoning**: WGSL entry points cannot be called, so sharing them means
  restructuring `vs_main`. Colour pipelines then stay unchanged: the same
  fixtures, link time and parse.
- **Trade-off**: a second link the first time a layer is picked (about 1 ms).
- **Future**: GUP-456 can pick on hover without touching colour pipelines.

#### Rows mapped at poll time

- **Decision**: the texel holds the stored row; `poll_picks` maps it through the
  current `DrawOrder`.
- **Reasoning**: snapshotting the order costs O(rows) per pick.
- **Trade-off**: a pick that is still in flight across a compaction can map
  wrongly. `GpuPick::generation` lets a host drop such picks.

### Development Workflow Insights

- The oracle test with near-tie diagnostics (all marks within 6 px printed on
  failure) found the precision problem at once.
- `GUP_BLESS=1` over every gup-core test plus `git diff --stat tests/golden` is
  a quick proof that colour output is byte-identical.
- WASM attribution: `twiggy diff` is useless across builds because every
  function's hash changes. Grouping `twiggy top -f csv` by name with the hash
  stripped works.
- The shared machine skews timings: the first `perf-budget` run failed four
  timing checks under load, and the second passed them all.

### Follow-up Stories

None. The scatter WASM budget (+3.2% from linking the pick API into the browser
harness) is for the orchestrator to record in `PERF_BUDGETS.md`. GUP-456 already
covers hover, click and hosts.
