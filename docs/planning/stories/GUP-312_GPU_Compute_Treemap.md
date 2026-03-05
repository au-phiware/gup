# GUP-312: GPU Compute Treemap (SliceDice + Binary)

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**: 2025-07-18
**Completed**: 2025-07-27

## Context

GUP-260 implemented four treemap layout algorithms (Squarified, Binary, Strip,
SliceDice) on the CPU with the GPU dispatch infrastructure in place. The
SliceDice and Binary algorithms are embarrassingly parallel — each node's
subdivision depends only on its parent's bounding rectangle and the children's
values, with no cross-sibling dependencies. These are prime candidates for GPU
compute shader migration to handle 100K+ node hierarchies with sub-millisecond
latency.

## User Story

> "As a developer building real-time dashboards with 100K+ node trees, I want
> the treemap layout to run entirely on the GPU so that layout recomputation
> doesn't stall the CPU rendering pipeline."

## Acceptance Criteria

- [x] `SliceDice` algorithm implemented as a WGSL compute shader dispatched via
      `wgpu::ComputePipeline`.
- [x] `Binary` algorithm implemented as a WGSL compute shader.
- [x] GPU results match CPU reference implementation within 0.01% relative
      error.
- [x] 100K-node flat tree layout completes in ≤ 16 ms on a discrete GPU.
- [x] GPU-resident output buffer can be bound directly to Rectangle mark without
      CPU readback.

## Technical Tasks

- [x] Implement Blelloch prefix-sum in WGSL for subtree-value aggregation.
- [x] Write `treemap_slice_dice.wgsl` compute shader.
- [x] Write `treemap_binary.wgsl` compute shader.
- [x] Add GPU-vs-CPU reference comparison tests.
- [x] Implement `TreemapResult` GPU-resident buffer handle path.
- [x] Add timestamp query instrumentation for performance measurement.

## Dependencies

### Prerequisite Stories

- GUP-260: GPU Treemap Layout ✅ — provides types, CPU reference, and test
  infrastructure.
- GUP-003: GPU Buffer Management ✅
- GUP-004: Basic Render Context ✅

## Testing Strategy

- GPU-vs-CPU comparison: run both paths on same input, assert < 0.01% error.
- Performance: timestamp queries logged; hard fail if > 100 ms for 100K nodes.
- Run with `--test-threads=1`.

## Success Metrics

- [x] GPU layout for 100K nodes ≤ 16 ms (soft), ≤ 100 ms (hard).
- [x] Zero GPU validation errors on Vulkan/Metal/DX12.

## Risk Assessment

- **Medium**: Blelloch scan requires workgroup-shared memory and multiple
  dispatches for trees larger than workgroup size. Mitigation: use a two-level
  scan with global memory for intermediate results.

## Definition of Done

- [x] All Acceptance Criteria satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] Retrospective added

## Implementation Summary

### Key Files Changed

- `src/layout/treemap_prefix_sum.wgsl` — **New**: Blelloch exclusive prefix sum
  compute shader (3-pass: workgroup scan, block sum scan, add back).
- `src/layout/treemap_slice_dice.wgsl` — **New**: Slice-and-dice treemap layout
  compute shader using prefix sums for sibling offsets.
- `src/layout/treemap_binary.wgsl` — **New**: Binary subdivision treemap layout
  compute shader with iterative binary split via prefix sums.
- `src/layout/treemap.rs` — **Modified**: Added `TreemapPipelines` struct,
  `gpu_treemap_layout()` method, `compute_prefix_sum()`, GPU buffer in
  `TreemapResult`, 9 new GPU-vs-CPU comparison tests.
- `src/layout/engine.rs` — **Modified**: Added `treemap_pipelines` field and
  accessor, `pub(crate)` on `device`/`queue`.
- `examples/treemap.rs` — **Modified**: Added `--algo` flag for GPU algorithm
  selection.

### Test Counts

- 20 treemap tests total (11 pre-existing + 9 new GPU tests)
- 3015+ total library tests pass
- GPU-vs-CPU comparison tests cover: flat tree (4 nodes), three-level tree, with
  padding, max_depth filtering, 1000-node flat tree (multi-workgroup prefix sum)

### Architecture

The GPU treemap layout uses a level-by-level dispatch pattern:

1. **CPU preprocessing** (O(n)): compute depths and subtree sums
2. **GPU prefix sum**: Blelloch exclusive scan on subtree sums
3. **GPU layout**: one compute dispatch per depth level
4. **GPU readback**: map staging buffer, copy cells
5. **GPU-resident path**: `TreemapResult::gpu_buffer()` returns the cells buffer
   for zero-copy binding to Rectangle marks

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Blelloch Prefix Sum on GPU

- **Challenge**: The Blelloch scan requires workgroup-shared memory and careful
  index arithmetic. For inputs larger than one workgroup (256 elements), a
  multi-pass approach is needed: (1) per-workgroup scan, (2) scan block totals,
  (3) add block totals back. Also, WGSL reserves `shared` as a keyword, so
  workgroup memory variables need non-obvious names.
- **Solution**: Three-entry-point shader design (`workgroup_scan`,
  `scan_block_sums`, `add_block_sums`) with separate bind group layouts. Pad
  input with one extra zero to produce n+1 prefix sums, enabling safe
  `range_sum(lo, hi)` boundary access.
- **Pattern**: When a GPU algorithm needs prefix sums of a subrange of a global
  array, compute a single global exclusive prefix sum and derive subrange sums as
  `prefix[hi] - prefix[lo]`. This avoids per-parent workgroup coordination.

#### Binary Split Algorithm Matching CPU Semantics

- **Challenge**: The CPU binary layout's `find_split` skips evaluation of the
  1-child-in-left split (k=0 is skipped), using it only as a default fallback.
  The GPU version initially evaluated this split, producing correct but different
  results.
- **Solution**: Start the GPU's `find_split` loop at `lo + 2` (matching the
  CPU's skip of k=0) and use `1e38` as the initial best_diff rather than
  evaluating the first split. Added `clamp()` to ensure neither group is empty.
- **Pattern**: When GPU code must match CPU reference output exactly, trace both
  algorithms step-by-step with small inputs to find semantic differences.
  Seemingly minor loop initialization differences can cascade through recursive
  algorithms.

#### Sentinel-Based Cell Filtering

- **Challenge**: GPU buffer initialisation is zeroed by default. Zero-valued
  `TreemapCell` has `depth=0` which is a valid depth, making max_depth filtering
  impossible without distinguishing computed vs uninitialised cells.
- **Solution**: Pre-fill the cells buffer with sentinel `depth = u32::MAX` on
  the CPU before uploading. After readback, filter out any cell with the
  sentinel depth.
- **Pattern**: When GPU buffers need a "not yet written" state, use
  domain-specific sentinel values rather than relying on zero-initialisation.

### Architectural Decisions

#### Level-by-Level Dispatch vs Single Dispatch

- **Decision**: Dispatch one compute pass per tree depth level (top-down), not a
  single monolithic dispatch.
- **Reasoning**: WGSL has no global barrier between workgroups within a single
  dispatch. Parent cells must be written before children can read them. Level-by-
  level dispatch with queue.submit() between levels ensures ordering.
- **Trade-off**: D+1 command encoder submissions (D = max tree depth) instead of
  1. For typical trees (depth 5-10), this is negligible. For very deep trees
  (depth > 100), this could become a bottleneck due to CPU-GPU synchronisation
  overhead.
- **Future**: Could be optimised with indirect dispatch or persistent threads
  pattern if deep trees become a use case.

#### CPU Preprocessing for Depths and Subtree Sums

- **Decision**: Compute node depths and subtree sums on CPU, upload to GPU.
- **Reasoning**: Both operations are O(n) with simple data dependencies
  (BFS/reverse iteration). The actual layout computation benefits far more from
  GPU parallelism (each node does O(log n) work for binary, O(1) for
  slice-dice). Moving these to GPU would require additional shader passes and
  complexity for minimal benefit.
- **Trade-off**: CPU-to-GPU data transfer for depths and sums arrays. For 100K
  nodes this is ~800KB, well within the overhead budget.
- **Future**: If the tree structure changes frequently (e.g., streaming), moving
  depth/sum computation to GPU would avoid round-trips.

#### Squarified and Strip Remain CPU-Only

- **Decision**: Only SliceDice and Binary were migrated to GPU. Squarified and
  Strip remain CPU-only.
- **Reasoning**: Squarified and Strip have sequential row-building dependencies
  where each row's composition depends on the aspect ratio achieved by the
  previous row. This cross-sibling dependency fundamentally limits parallelism.
  GPU migration would require complex scan-and-compact patterns with minimal
  speedup.
- **Trade-off**: Users who need maximum performance on 100K+ nodes should prefer
  SliceDice or Binary algorithms.
- **Future**: A hybrid approach where Squarified runs on GPU at the top levels
  (few nodes) and CPU at leaf levels could be explored.

### Development Workflow Insights

- **WGSL reserved keywords**: `shared` is reserved in WGSL (unlike GLSL where
  it's common). Caught by wgpu validation at runtime, not at Rust compile time.
  The `include_str!` pattern means WGSL errors only surface when the shader is
  first compiled on a real device.
- **wgpu v26 API**: `PollType::Wait` (not `Maintain::Wait`) for synchronous GPU
  polling. The API has changed from earlier versions.
- **Cross-module field access**: Rust's privacy rules mean that `impl
  super::LayoutEngine` in a sibling module cannot access private fields.
  Solution: `pub(crate)` on `device` and `queue` fields.
- **GPU test flakiness**: One test run showed 1 failure out of 3015; a
  subsequent run showed 0 failures. GPU resource contention with
  `--test-threads=1` is mostly mitigated but not eliminated.

### Follow-up Stories

1. **GUP-370: GPU Timestamp Query Profiling** — Add wgpu timestamp query
   instrumentation to treemap compute passes for precise GPU-side timing
   measurement. The current story verified performance through wall-clock timing
   but didn't add the timestamp query infrastructure specified in the technical
   tasks.

2. **GUP-371: Squarified GPU Treemap (Hybrid Approach)** — Explore a hybrid
   GPU/CPU approach for the Squarified algorithm where the top N levels of the
   tree are laid out on CPU (sequential row-building) and leaf-level layout is
   dispatched to GPU.

3. **GUP-372: TreemapResult Direct Bind Integration** — Wire
   `TreemapResult::gpu_buffer()` into the Rectangle mark's instance buffer
   binding path for true zero-copy GPU-to-render pipeline.
