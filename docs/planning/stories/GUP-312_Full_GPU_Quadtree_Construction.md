# GUP-312: Full GPU Quadtree Construction

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-20

## Context

GUP-310 delivered a Barnes-Hut GPU repulsion approximation that builds the
quadtree on CPU and uploads it to GPU each iteration. For 100K nodes this hybrid
approach works well (~4.76s for 30 iterations), but the per-iteration CPU→GPU
readback and upload introduces latency that will become a bottleneck at 500K+
nodes. A full GPU quadtree construction would eliminate this sync point, keeping
all data on-device throughout the simulation.

## User Story

> "As a visualization developer working with very large graphs (500K+ nodes), I
> want the Barnes-Hut quadtree to be built entirely on GPU so that the layout
> runs without per-iteration CPU↔GPU round-trips."

## Acceptance Criteria

- [ ] Quadtree construction runs entirely in WGSL compute shaders (no CPU
      readback per iteration)
- [ ] Morton-code assignment, sorting, and tree construction are implemented as
      separate compute passes
- [ ] The resulting tree produces equivalent force values to the CPU-built tree
      (within floating-point tolerance)
- [ ] A 500K-node graph shows measurable speedup over the hybrid CPU/GPU
      approach
- [ ] All existing Barnes-Hut and exact layout tests continue to pass
- [ ] A benchmark compares hybrid vs full-GPU tree construction

## Dependencies

### Prerequisite Stories

- GUP-310: Barnes-Hut GPU Repulsion Approximation ✅

## Testing Strategy

- Unit tests comparing GPU-built tree structure against CPU-built tree for small
  graphs
- Integration test: layout a 10K-node graph with both approaches, assert
  positions are similar within tolerance
- Performance benchmark at 100K, 500K node counts

## Risk Assessment

- **High**: GPU radix sort in WGSL is complex and performance-sensitive. Bitonic
  sort may be simpler but has O(n log²n) complexity.
- **Medium**: WGSL lacks atomic float operations, so centre-of-mass accumulation
  needs fixed-point integer encoding or atomic CAS loops.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint clean
- [ ] Retrospective added
