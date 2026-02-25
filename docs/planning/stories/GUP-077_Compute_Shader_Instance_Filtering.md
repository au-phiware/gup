# GUP-077: Compute Shader Instance Sorting and Filtering

**Story ID**: GUP-077 **Title**: Compute Shader Instance Sorting and Filtering
**Status**: 🚧 In Progress **Priority**: Medium **Effort**: — **Created**:
2026-02-25 **Dependencies**: GUP-074 (Mark Performance Optimization)

## Overview

Move instance culling, LOD classification, and Z-order sorting to GPU compute
shaders for datasets exceeding 1M instances, where CPU-side filtering becomes a
bottleneck. Builds on the `InstanceAttributes` common format from GUP-074.

## Context

GUP-074 performs culling and LOD classification on the CPU. For typical
visualization sizes (up to 100K marks), CPU processing is fast enough
(benchmarks show <1ms for 100K instances). However, for streaming data or very
large datasets (>1M instances), the CPU becomes the bottleneck. Moving this work
to a compute shader keeps the entire pipeline on the GPU.

## User Story

As a developer rendering 1M+ streaming data points, I want culling and LOD
classification to run on the GPU so that CPU overhead does not limit frame rate.

## Acceptance Criteria

- [ ] Compute shader performs frustum culling on GPU
- [ ] Compute shader classifies instances by LOD level
- [ ] Compute shader sorts instances by Z-order for correct rendering
- [ ] Output is a compact buffer of visible instances (indirect draw)
- [ ] CPU overhead for 1M instances reduced by >10x compared to GUP-074's CPU
      path
- [ ] Falls back to CPU path when compute shaders are unavailable

## Technical Tasks

1. Define `InstanceAttributes` storage buffer layout for compute shaders
2. Implement culling compute shader (frustum test per instance)
3. Implement LOD classification compute shader
4. Implement prefix-sum compaction for visible instance output
5. Integrate with `wgpu::indirect_draw` for zero-CPU draw calls
6. Add fallback path for platforms without compute shader support
7. Benchmarks at 100K, 1M, 10M scales

## Dependencies

- GUP-074: Mark Performance Optimization (provides `InstanceAttributes`,
  `CullingManager`, `Viewport2D`, `LodLevel`)

## Testing Strategy

- GPU integration tests comparing CPU vs compute shader culling results
- Visual regression tests ensure identical output
- Performance benchmarks at 1M and 10M instance scales

## Success Metrics

- 10x reduction in CPU time for >1M instance culling
- Identical visual output to CPU path
- <2ms total compute shader time for 10M instances

## Risk Assessment

- **Risk**: Not all platforms support compute shaders (e.g. WebGL fallback)
  - **Mitigation**: CPU fallback path always available
- **Risk**: Prefix-sum compaction is complex to implement correctly
  - **Mitigation**: Use well-known parallel scan algorithm

## Definition of Done

- [ ] Compute shader implementation compiles and runs
- [ ] Results match CPU path within floating-point tolerance
- [ ] Performance benchmarks show improvement at 1M+ scales
- [ ] Fallback path works on non-compute platforms
- [ ] Documentation updated
