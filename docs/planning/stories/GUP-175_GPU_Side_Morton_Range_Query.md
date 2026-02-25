# GUP-175: GPU-Side Morton Range Query

**Priority**: Medium **Complexity**: High **Created**: 2025-08-06 **Status**: 📋
Planned

## Overview

Implement Morton-based spatial query entirely on GPU using sorted buffers and
binary search in compute shaders, eliminating the CPU roundtrip for candidate
narrowing.

## Context

GUP-078 implemented Morton and Hierarchical spatial indices that narrow
candidates on the CPU before dispatching GPU hit testing. This works well but
requires a CPU-GPU data transfer step. Moving the Morton range query to GPU
would keep the entire query hot path on the GPU.

## User Story

As a developer building interactive visualisations, I want spatial queries to
execute entirely on the GPU so that query latency is minimised for million-point
datasets.

## Acceptance Criteria

- [ ] Implement GPU compute shader that performs binary search on a sorted
      Morton key buffer
- [ ] Spatial queries run entirely on GPU (no CPU candidate narrowing)
- [ ] Performance improvement over CPU-side narrowing for >100K elements
- [ ] Maintain correctness for point and region queries
- [ ] Compatible with existing InteractionSystem API

## Technical Tasks

1. Upload sorted Morton entries to a GPU storage buffer
2. Implement binary search in WGSL compute shader
3. Wire up the GPU-side query as an alternative path in InteractionSystem
4. Benchmark against CPU-side narrowing at various scales

## Dependencies

- **Requires**: GUP-078 (Morton implementation and integration)

## Testing Strategy

- GPU integration tests comparing results with CPU implementation
- Performance benchmarks at 10K, 100K, 1M elements

## Risk Assessment

- **Medium**: WGSL compute shaders lack recursion; iterative binary search is
  straightforward but range queries over Z-curves require careful handling of
  non-contiguous key ranges.

## Definition of Done

- [ ] GPU compute shader implements Morton binary search
- [ ] All existing spatial index tests pass
- [ ] Performance benchmarks show improvement at >100K scale
- [ ] `mask all-fix` passes
