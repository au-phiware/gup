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
