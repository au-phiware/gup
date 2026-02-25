# GUP-184: GPU Radix Sort for Z-Order

**Story ID**: GUP-184 **Title**: GPU Radix Sort for Z-Order **Status**: 📋
Planned **Priority**: Low **Effort**: — **Created**: 2026-07-19
**Dependencies**: GUP-077 (Compute Shader Instance Sorting and Filtering)

## Overview

Implement a parallel GPU radix sort pass in the compute shader instance
filtering pipeline to sort instances by Z-depth after culling and before
compaction. This enables correct depth-based rendering for 3D visualizations and
2D scenes where instance Z-order varies dynamically.

## Context

GUP-077's compute shader pipeline preserves input order through stable stream
compaction. For 2D visualizations where Z-order is determined by draw order,
this is sufficient. However, 3D visualization support and depth-varying 2D
scenes (e.g., fisheye projections, animated depth transitions) require GPU-side
sorting by Z-depth to ensure correct back-to-front rendering.

## User Story

As a developer rendering 3D scatter plots or depth-varying 2D scenes, I want
instances to be GPU-sorted by Z-depth so that transparent marks render correctly
without CPU intervention.

## Acceptance Criteria

- [ ] GPU radix sort pass sorts visible instances by Z-depth key
- [ ] Sort is activated via the `enable_sort` flag in `FilterConfig`
- [ ] Correct back-to-front ordering verified with readback tests
- [ ] Sort adds <1ms overhead for 1M instances
- [ ] Existing non-sorted path unaffected when `enable_sort` is false

## Technical Tasks

1. Implement 4-pass radix sort in WGSL (1 bit per pass × 32 bits)
2. Use the existing prefix sum infrastructure for scatter offsets
3. Add Z-depth key extraction from `InstanceAttributes`
4. Add sort-specific benchmarks
5. Integration test comparing sorted output with CPU-sorted reference

## Dependencies

- GUP-077: Compute Shader Instance Sorting and Filtering
- GUP-183: Pooled GPU Instance Filter Buffers (recommended for buffer reuse)

## Testing Strategy

- GPU tests comparing sorted output with CPU std::sort reference
- Performance benchmarks at 100K and 1M scales
- Visual tests with overlapping transparent marks

## Success Metrics

- Correct Z-ordering verified against CPU reference
- <1ms sort overhead for 1M instances
- No regression in non-sorted path performance

## Risk Assessment

- **Risk**: Radix sort requires many passes (32 for 32-bit keys)
  - **Mitigation**: Use 8-bit digits (4 passes) or hybrid approach

## Definition of Done

- [ ] GPU radix sort implementation compiles and runs
- [ ] Sorted output matches CPU reference
- [ ] Benchmarks show acceptable overhead
- [ ] Documentation updated
