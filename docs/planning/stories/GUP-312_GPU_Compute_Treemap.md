# GUP-312: GPU Compute Treemap (SliceDice + Binary)

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-07-18

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

- [ ] `SliceDice` algorithm implemented as a WGSL compute shader dispatched via
      `wgpu::ComputePipeline`.
- [ ] `Binary` algorithm implemented as a WGSL compute shader.
- [ ] GPU results match CPU reference implementation within 0.01% relative
      error.
- [ ] 100K-node flat tree layout completes in ≤ 16 ms on a discrete GPU.
- [ ] GPU-resident output buffer can be bound directly to Rectangle mark without
      CPU readback.

## Technical Tasks

- [ ] Implement Blelloch prefix-sum in WGSL for subtree-value aggregation.
- [ ] Write `treemap_slice_dice.wgsl` compute shader.
- [ ] Write `treemap_binary.wgsl` compute shader.
- [ ] Add GPU-vs-CPU reference comparison tests.
- [ ] Implement `TreemapResult` GPU-resident buffer handle path.
- [ ] Add timestamp query instrumentation for performance measurement.

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

- [ ] GPU layout for 100K nodes ≤ 16 ms (soft), ≤ 100 ms (hard).
- [ ] Zero GPU validation errors on Vulkan/Metal/DX12.

## Risk Assessment

- **Medium**: Blelloch scan requires workgroup-shared memory and multiple
  dispatches for trees larger than workgroup size. Mitigation: use a two-level
  scan with global memory for intermediate results.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
