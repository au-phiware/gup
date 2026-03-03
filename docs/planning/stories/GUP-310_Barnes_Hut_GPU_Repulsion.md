# GUP-310: Barnes-Hut GPU Repulsion Approximation

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-19

## Context

GUP-259 delivered a GPU force-directed layout engine with O(n²) pairwise
repulsion. At 100K nodes the pairwise approach takes ~670ms per iteration,
making the ≤5s performance target unreachable without algorithmic improvement.
The Barnes-Hut algorithm approximates far-field repulsion using an octree/quadtree,
reducing per-iteration cost from O(n²) to O(n log n). On GPU this requires a
multi-pass compute pipeline: build tree → compute centres of mass → traverse tree
for each node.

## User Story

> "As a visualization developer, I want the force-directed layout to handle
> 100K+ nodes in ≤5 seconds so that I can interactively explore large graphs."

## Acceptance Criteria

- [ ] A GPU-side quadtree (2D) or octree (3D) is built from node positions each
      iteration using compute shaders
- [ ] Repulsion forces are computed by traversing the tree with a configurable
      theta (opening angle) parameter
- [ ] The `ForceDirected` builder gains a `.approximation_theta(f32)` method
      (default 0.5)
- [ ] At theta=0 the algorithm falls back to exact O(n²) pairwise computation
- [ ] A 100K-node / 300K-edge random graph lays out in ≤5 seconds on an
      integrated GPU (30 iterations)
- [ ] All existing layout tests continue to pass
- [ ] A benchmark compares exact vs Barnes-Hut performance

## Dependencies

### Prerequisite Stories

- GUP-259: GPU Force-Directed Graph Layout ✅

## Testing Strategy

- Unit tests for tree construction correctness (small known graphs)
- Integration test comparing Barnes-Hut result to exact result for small graphs
  (positions should be similar within tolerance)
- Performance benchmark at 10K, 100K, 500K node counts

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint clean
- [ ] Retrospective added
