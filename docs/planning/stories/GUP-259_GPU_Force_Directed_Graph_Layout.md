# GUP-259: GPU Force-Directed Graph Layout

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-01-31

## Context

Force-directed graph layout algorithms simulate physical forces between nodes —
repulsion between all node pairs, spring attraction along edges, and a centering
gravity — to iteratively converge on an aesthetically pleasing spatial
arrangement. The canonical CPU implementation (e.g. D3-force) performs O(n²)
pairwise repulsion computations per iteration and typically becomes impractical
beyond ~10K nodes. The GPU's massively parallel architecture allows all pairwise
force contributions for 100K+ nodes to be computed simultaneously in a single
compute pass, enabling layouts that would be infeasible on the CPU.

The implementation strategy document explicitly calls out GPU-accelerated layout
as a key advanced-scale deliverable, including a concrete `LayoutEngine` API
sketch with `force_directed_layout(nodes, edges, iterations) -> LayoutResult`
and the observation that convergence detection can be performed on-device with
an early-exit loop. This story realises that design.

The required GPU infrastructure is already in place. GUP-003 provides the typed
`GpuBuffer` abstraction used for node and edge data. GUP-004 supplies the
`RenderContext` / device handle needed to create and dispatch compute pipelines.
GUP-077 delivered the first general-purpose compute shader pipeline in the
project — GPU-side instance culling and sorting — establishing the patterns
(bind group layouts, dispatch sizing, readback) that this story will follow.

A Barnes-Hut tree approximation reduces the per-iteration work from O(n²) to O(n
log n) at the cost of a multi-pass build-and-query algorithm. For an initial
implementation, direct (pairwise) force computation in WGSL is simpler to
validate and already GPU-parallel; the risk section below addresses whether
Barnes-Hut is required to hit the 100K / 5-second target.

## User Story

> "As a visualization developer, I want a GPU-accelerated force-directed layout
> engine so that I can lay out graphs with 100K+ nodes in interactive time
> without saturating the CPU."

> "As a chart builder user, I want to call
> `chart.graph_layout(ForceDirected::new())` so that I can declaratively
> configure and apply force-directed layout within the existing ChartBuilder
> API."

## Acceptance Criteria

### AC1: LayoutEngine API

- [ ] `LayoutEngine` struct is publicly exported from `gup`
- [ ] `LayoutEngine::new(context: &RenderContext) -> Result<Self, GupError>`
      constructs a layout engine, compiling compute shaders at creation time
- [ ] `LayoutEngine::force_directed_layout(nodes, edges, iterations) -> LayoutResult`
      is `async` and returns final node positions
- [ ] `LayoutResult` contains a `Vec<NodePosition>` (at minimum `id`, `x`, `y`)
      accessible on the CPU after the layout completes
- [ ] The API is documented with a rustdoc example

### AC2: Force Configuration

- [ ] `ForceDirected` builder supports configuring repulsion strength
      (`repulsion_strength(f32)`)
- [ ] `ForceDirected` supports spring attraction along edges
      (`spring_strength(f32)`, `spring_rest_length(f32)`)
- [ ] `ForceDirected` supports a gravity term pulling nodes toward the centre
      (`gravity(f32)`)
- [ ] `ForceDirected` supports a velocity-damping coefficient (`damping(f32)`)
- [ ] All force parameters have documented defaults that produce sensible
      layouts without user configuration

### AC3: GPU Compute Implementation

- [ ] Force computation runs entirely in WGSL compute shaders; no per-iteration
      CPU involvement once the simulation starts
- [ ] Node positions and velocities are stored in GPU buffers that persist
      across iterations, avoiding round-trip copies
- [ ] Edge spring forces are accumulated in a separate compute pass reading from
      an edge-list buffer
- [ ] Position update (Euler integration, apply damping) is a dedicated compute
      pass writing back to the node buffer
- [ ] All compute shaders compile without `wgpu` validation errors in the test
      suite

### AC4: Convergence Detection

- [ ] A convergence-check compute pass calculates the maximum node displacement
      per iteration and writes a scalar result into a small readback buffer
- [ ] `force_directed_layout` exits the iteration loop early when the maximum
      displacement falls below a configurable threshold
      (`convergence_threshold(f32)`, default `0.5` pixels)
- [ ] Early exit is observable: a layout that has already converged completes in
      fewer GPU dispatches than `iterations`

### AC5: Performance Target

- [ ] A benchmark (Criterion or stand-alone async bench) demonstrates that a
      random graph of 100K nodes and ~300K edges completes layout in ≤5 seconds
      on a discrete GPU (documented hardware in the benchmark output)
- [ ] The benchmark is integrated into the existing performance suite or added
      as a named example that CI can invoke

### AC6: ChartBuilder Integration

- [ ] `ChartBuilder` (or equivalent high-level builder) exposes a
      `graph_layout(layout: impl GraphLayout)` method
- [ ] `ForceDirected` implements the `GraphLayout` trait
- [ ] An example (e.g., `examples/force_directed_graph.rs`) builds and renders a
      graph with 1K+ nodes using this API and compiles with
      `cargo check --examples`

## Technical Tasks

- [ ] Define `Node`, `Edge`, `NodePosition`, and `LayoutResult` data types;
      derive `bytemuck::Pod + bytemuck::Zeroable` for GPU buffer use
- [ ] Implement `ForceDirected` builder with configurable force parameters and
      sensible defaults
- [ ] Define `GraphLayout` trait with an `async fn apply(...)` method
- [ ] Write WGSL compute shader: repulsion pass — for each node, sum repulsion
      contributions from every other node (initial O(n²) pairwise approach)
- [ ] Write WGSL compute shader: spring pass — for each edge, compute
      displacement toward rest length and accumulate into force buffer
- [ ] Write WGSL compute shader: integration pass — apply accumulated forces,
      add gravity, multiply by damping, update velocities, update positions
- [ ] Write WGSL compute shader: convergence pass — reduce max displacement into
      a 4-byte staging buffer; read back to CPU for early-exit check
- [ ] Implement `LayoutEngine` struct managing pipeline creation and buffer
      allocation; compile all shaders in `new()`
- [ ] Implement the async iteration loop in `force_directed_layout`; dispatch
      each compute pass, poll convergence, break on threshold
- [ ] Add `graph_layout()` method to `ChartBuilder` (or appropriate entry point)
- [ ] Implement the `GraphLayout` trait for `ForceDirected`
- [ ] Write unit tests: force parameter defaults, convergence threshold, small
      synthetic graphs (4–8 nodes) with known expected layout symmetry
- [ ] Write integration test: layout a 1K-node random graph, assert all node
      positions are finite and within a reasonable bounding box
- [ ] Write the `examples/force_directed_graph.rs` example
- [ ] Write or extend a benchmark for the 100K-node performance target
- [ ] Document public API with rustdoc; add inline WGSL comments explaining
      force model

## Dependencies

### Prerequisite Stories

- GUP-003: GPU Buffer Management ✅ — provides `GpuBuffer<T>` typed buffer
  abstraction used for node, edge, force-accumulation, and staging buffers
- GUP-004: Basic Render Context ✅ — provides `RenderContext` / `wgpu::Device`
  and `wgpu::Queue` handles required to create compute pipelines and submit
  command encoders
- GUP-077: Compute Shader Instance Filtering ✅ — established compute shader
  pipeline patterns (bind group layout construction, workgroup dispatch sizing,
  GPU-to-CPU readback) that this story follows

### Enables Stories

- Any story building interactive graph visualisation (node selection, zooming,
  real-time streaming layout) will build on the `LayoutEngine` and `GraphLayout`
  trait introduced here

## Testing Strategy

- **Unit tests**: Verify `ForceDirected` builder defaults are in range; verify
  convergence threshold logic with a mock that returns predetermined
  displacement values; verify 4-node ring graph converges to a symmetric square
  arrangement (within floating-point tolerance)
- **Integration tests**: Lay out a randomly generated 1K-node Erdős-Rényi graph;
  assert all output positions are finite, not NaN, and lie within a 4096×4096
  bounding box; assert iteration count is ≤ configured maximum
- **Compile check**: `cargo check --examples` passes for
  `examples/force_directed_graph.rs`
- **Visual validation**: Run the example and inspect the rendered graph by eye
  for plausible clustering and edge-crossing minimisation
- **Performance**: Benchmark a 100K-node random graph; record wall time and GPU
  iteration count; assert ≤5 s on the target hardware (document hardware spec in
  benchmark output)

## Success Metrics

- [ ] 100K-node random graph lays out in ≤5 seconds on a discrete GPU
- [ ] All unit and integration tests pass with `cargo test -- --test-threads=1`
- [ ] The `force_directed_graph` example compiles and renders without GPU
      validation errors
- [ ] `ForceDirected::new()` with default parameters produces a visually
      coherent layout for a real-world graph (e.g., a social-network adjacency
      list with ~1K nodes)

## Risk Assessment

- **Medium**: O(n²) pairwise repulsion may not reach 100K in ≤5 s even on GPU.
  At 100K nodes there are 5×10⁹ pairs per iteration; even at 10 TFLOP/s a full
  repulsion pass takes ~0.5 ms, and dozens of iterations may be needed.
  _Mitigation_: Profile first. If pairwise is sufficient, ship it. If not,
  implement a Barnes-Hut octree approximation in a multi-pass compute shader
  (build BVH → query forces). The story should note in the implementation which
  path was taken.

- **Medium**: Convergence readback introduces a GPU→CPU sync point per
  iteration, which stalls the GPU pipeline. For very large graphs this may
  dominate over compute time. _Mitigation_: Poll convergence only every N
  iterations (e.g., every 10) rather than every iteration. Expose
  `convergence_check_interval(u32)` as a configuration knob.

- **Low**: `bytemuck` Pod layout of `Node`/`Edge` structs must exactly match
  WGSL struct layout (alignment, padding). Mismatches cause silently wrong
  results. _Mitigation_: Add a compile-time size assertion (`static_assertions`)
  and a round-trip test that writes known values to a GPU buffer and reads them
  back.

- **Low**: Shader compilation errors surface at runtime in wgpu; a mistake in
  WGSL will panic or return an error only when `LayoutEngine::new()` is called.
  _Mitigation_: Exercise `LayoutEngine::new()` in a dedicated unit test so CI
  catches shader compilation failures immediately.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
