# GUP-260: GPU Treemap Layout

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-01-31

## Context

Treemap layouts are a classical technique for visualising hierarchical data: a
root rectangle is recursively subdivided so that each node's area is
proportional to its associated value. They are widely used for file-system
explorers, portfolio analysis, and any domain where a hierarchy of magnitudes
must be compared at a glance. The canonical Squarified algorithm (Bruls, Huizing
& van Wijk, 1999) produces aspect-ratio-minimising cells but does so
level-by-level in a sequential pass that does not map naturally to a GPU
workgroup model.

For small hierarchies (< 10 K nodes) a CPU implementation is fine, but for
continuously-updated or very large trees — organisation charts, genomic
taxonomies, filesystem snapshots — CPU-side layout becomes a bottleneck. Gup's
`LayoutEngine` (sketched in the implementation strategy under "GPU-Accelerated
Layouts") is the right home for a compute-shader–based treemap that can
recalculate 100 K-node layouts in a single GPU dispatch, keeping the result on
the GPU and handing it directly to the Rectangle mark without a CPU round-trip.

GUP-003 (GPU Buffer Management) and GUP-004 (Basic Render Context) provide the
buffer and pipeline infrastructure needed for compute dispatch. GUP-067
(Rectangle and Line Mark Implementations) provides the `Rectangle` mark that
consumes the layout output; treemap cells are just styled, positioned
rectangles.

## User Story

> "As a visualization developer, I want to call
> `LayoutEngine::treemap_layout(nodes, values, viewport)` and bind the result
> directly to a `Rectangle` mark selection so that I can render real-time
> treemaps of 100 K-node hierarchies without CPU layout bottlenecks."

> "As an end user viewing a live dashboard, I want the treemap to redraw
> smoothly when the underlying data changes so that I can explore hierarchy
> dynamics without lag."

## Acceptance Criteria

### AC1: Public API — `LayoutEngine::treemap_layout`

- [ ] `LayoutEngine` struct exists in `src/layout/` (or an equivalent module)
      and is publicly re-exported from the crate root.
- [ ] `treemap_layout(nodes: &[TreeNode], values: &[f32], viewport: Rect, options: TreemapOptions) -> TreemapResult`
      is callable from safe Rust with no `unsafe` at the call site.
- [ ] `TreemapOptions` carries at minimum: `algorithm: TreemapAlgorithm`,
      `max_depth: Option<u32>`, and `padding: f32`.
- [ ] `TreemapAlgorithm` is an enum with at least the variants `Squarified`,
      `Binary`, `Strip`, and `SliceDice`.
- [ ] `TreemapResult` exposes a `cells()` method returning a slice (or GPU
      buffer handle) of
      `TreemapCell { x: f32, y: f32, width: f32, height: f32,     depth: u32, value: f32, node_index: u32 }`.

### AC2: GPU Compute Shader — Squarified Layout

- [ ] A WGSL compute shader (`treemap_squarified.wgsl` or equivalent) is
      compiled and dispatched via a `wgpu::ComputePipeline` without GPU
      validation errors on both Vulkan/Metal/DX12 backends.
- [ ] The shader produces cell rectangles whose areas are proportional to the
      input values (≤ 1 % relative error for any single cell in the test
      dataset).
- [ ] No cell overlaps its sibling cells within the same parent (verified by the
      integration test described in AC5).
- [ ] All cells are fully contained within their parent's bounding rectangle.

### AC3: Algorithm Coverage

- [ ] `SliceDice` and `Strip` variants produce correct, non-overlapping
      subdivisions and pass the containment property test.
- [ ] `Binary` variant subdivides each row into two groups of roughly equal
      total value, recursively, and passes the same tests.
- [ ] All four variants are exercised by a parameterised unit test.

### AC4: Depth-Limited Rendering

- [ ] When `TreemapOptions::max_depth` is `Some(n)`, only nodes at depth ≤ n are
      emitted in `TreemapResult::cells()`.
- [ ] A unit test confirms that with `max_depth: Some(1)` only the immediate
      children of the root are returned for a two-level test tree.

### AC5: Integration with Rectangle Mark

- [ ] An example `examples/treemap.rs` compiles and runs without panics or GPU
      errors; it renders a synthetic 1 000-node tree using `Rectangle` marks
      coloured by depth.
- [ ] The example demonstrates wiring `TreemapResult::cells()` into a
      `Selection<_, Rectangle>` via the existing attribute-binding API.
- [ ] The example accepts an optional `--nodes <N>` CLI argument to test with
      larger trees (defaults to 1 000, tested at 100 000 in CI where a GPU is
      available).

### AC6: Color Coding

- [ ] `TreemapCell` exposes `depth` and `value` fields usable as inputs to a
      `ColorScale` (or equivalent shader function) for per-cell fill colour.
- [ ] The example renders cells with two colour modes selectable at runtime:
      colour-by-depth and colour-by-value.

### AC7: Performance

- [ ] For a flat 100 K-node tree (all children of a single root), GPU layout
      dispatch completes in ≤ 16 ms on a discrete GPU (measured with
      `wgpu::QuerySet` timestamp queries and logged; hard failure only if > 100
      ms).
- [ ] A benchmark entry is added under `benches/` covering the 100 K-node case.

## Technical Tasks

- [ ] Create `src/layout/mod.rs` and `src/layout/treemap.rs`; add
      `pub mod layout` to `src/lib.rs`.
- [ ] Define
      `TreeNode { parent: Option<u32>, child_start: u32, child_count: u32 }` as
      a GPU-friendly flat-tree representation (matches a standard BFS/DFS
      linearisation).
- [ ] Define `TreemapOptions`, `TreemapAlgorithm`, `TreemapCell`, and
      `TreemapResult` in `src/layout/treemap.rs`.
- [ ] Implement `LayoutEngine` struct holding pre-compiled
      `wgpu::ComputePipeline` instances (one per algorithm variant, or a single
      pipeline with a uniform flag).
- [ ] Write `src/layout/shaders/treemap_squarified.wgsl`: - Pass 1 (prefix-sum):
      compute per-node subtree-sum values using a parallel scan workgroup. -
      Pass 2 (subdivision): each workgroup handles one parent node, iterating
      over its children to greedily assign rows following the Squarified rule. -
      Output buffer: one `TreemapCell` per node.
- [ ] Write `src/layout/shaders/treemap_slice_dice.wgsl` (simple alternating
      horizontal/vertical cuts; suitable as a correctness baseline).
- [ ] Implement `Strip` and `Binary` either as additional shaders or as
      compile-time constants/uniforms in a shared shader.
- [ ] Add depth-filter pass: a second compute dispatch (or conditional in the
      output pass) that zeroes out cells beyond `max_depth`.
- [ ] Implement `TreemapResult::cells()` with both CPU-readable (mapped staging
      buffer) and GPU-resident (direct bind) access paths.
- [ ] Write unit tests in `src/layout/treemap.rs` covering: - Area
      proportionality (≤ 1 % error) - Non-overlap of sibling cells - Containment
      within parent rectangle - `max_depth` filtering - All four algorithm
      variants
- [ ] Create `examples/treemap.rs` with depth- and value-based colour modes and
      the `--nodes` argument.
- [ ] Add `benches/treemap_layout.rs` with Criterion benchmarks for 1 K, 10 K,
      and 100 K node counts.
- [ ] Document public API with `///` doc-comments; add a module-level doc
      explaining the flat-tree input format.

## Dependencies

### Prerequisite Stories

- GUP-003: GPU Buffer Management ✅ — provides `GpuBuffer` and staging-buffer
  infrastructure used to upload tree node data and read back layout results.
- GUP-004: Basic Render Context ✅ — provides `GupContext` / `wgpu::Device` and
  `wgpu::Queue` needed to compile and dispatch the compute pipeline.
- GUP-067: Rectangle and Line Mark Implementations ✅ — provides the `Rectangle`
  mark and its `RectangleAttributes`; treemap cells are rendered as positioned,
  sized rectangles.

### Enables Stories

- Future story: Interactive Treemap Drill-Down — `TreemapResult` cell indices
  map back to input nodes, enabling click-to-zoom interaction once an
  interaction layer is in place.
- Future story: GPU Force-Directed Layout — the `LayoutEngine` scaffolding and
  multi-pass compute pipeline pattern established here provide the template for
  an iterative force simulation.

## Testing Strategy

- **Unit tests**: Area proportionality, sibling non-overlap, parent containment,
  and `max_depth` filtering are all verifiable on CPU by mapping the result
  staging buffer back. Run with `cargo test -- --test-threads=1`.
- **Integration tests**: `examples/treemap.rs` with `--nodes 1000` runs
  headlessly in CI (software renderer via `wgpu` Vulkan/GL fallback) and asserts
  zero GPU validation errors.
- **Visual validation**: Manual inspection of `examples/treemap.rs` output with
  a known hierarchical dataset (e.g., a directory tree snapshot); screenshot
  comparison can be added in a follow-up story.
- **Performance**: Criterion benchmark in `benches/treemap_layout.rs`; the 100 K
  node case is gated at ≤ 100 ms (soft goal ≤ 16 ms on discrete GPU).

## Success Metrics

- [ ] `cargo test -- --test-threads=1` passes with all new tests green.
- [ ] `examples/treemap.rs` renders a 1 000-node tree without GPU validation
      errors or panics.
- [ ] GPU layout dispatch for 100 K nodes completes in ≤ 100 ms (measured via
      timestamp queries logged to stdout; no hard CI failure unless > 1 s).
- [ ] All four `TreemapAlgorithm` variants pass the area-proportionality and
      non-overlap assertions.
- [ ] Benchmark entry appears in `benches/` and compiles cleanly.

## Risk Assessment

- **Medium**: The Squarified algorithm has a sequential data dependency between
  rows within a single parent: each row's aspect-ratio decision depends on the
  remaining area after prior rows are committed. A naïve GPU mapping forces one
  workgroup per parent, limiting parallelism for wide, flat trees. For extremely
  wide parents (> workgroup size children) the shader must loop internally.
  _Mitigation_: Cap parallelism at the parent level for the Squarified variant;
  `SliceDice` and `Binary` are embarrassingly parallel and serve as a fast-path
  for flat trees. Document the trade-off in the module doc.

- **Medium**: Prefix-sum (parallel scan) for subtree-value aggregation is
  non-trivial to implement correctly in WGSL without atomics on older backends.
  _Mitigation_: Use a two-pass Blelloch scan pattern which requires only
  workgroup-shared memory; fall back to a CPU-side prefix-sum for the first
  iteration if shader validation fails on constrained backends, with a TODO to
  remove the fallback.

- **Low**: The Rectangle mark (GUP-067) uses `RectangleAttributes` centred on
  `(cx, cy)` with separate `width` / `height`. The treemap outputs
  `(x, y, width, height)` as top-left origin. A thin adapter (trivial
  arithmetic) is needed at the binding site. _Mitigation_: Document the
  coordinate convention in `TreemapCell` and provide a helper
  `TreemapCell::to_rectangle_attributes()` conversion.

- **Low**: `wgpu::QuerySet` timestamp queries require the `TIMESTAMP_QUERY`
  feature, which is not universally supported (notably absent on some WebGL
  backends). _Mitigation_: Gate timestamp instrumentation behind a
  `cfg(feature = "gpu_timing")` feature flag; the benchmark falls back to
  wall-clock timing when the feature is absent.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
