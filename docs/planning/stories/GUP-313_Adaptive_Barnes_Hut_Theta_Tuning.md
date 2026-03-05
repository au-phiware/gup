# GUP-313: Adaptive Barnes-Hut Theta Tuning

## Story Overview

**Initiative**: Advanced Scale **Status**: ✅ Complete **Created**: 2025-07-20
**Completed**: 2026-03-05

## Context

GUP-310 introduced a global theta parameter for Barnes-Hut repulsion. A fixed
theta works well for uniformly distributed graphs, but real-world graphs often
have dense clusters alongside sparse regions. An adaptive theta that varies by
region could improve accuracy in dense areas (lower theta) while maintaining
speed in sparse areas (higher theta), yielding better layout quality without
sacrificing overall performance.

## User Story

> "As a visualization developer, I want the Barnes-Hut algorithm to
> automatically adjust its approximation quality based on local graph density so
> that dense clusters are laid out accurately without slowing down the overall
> simulation."

## Acceptance Criteria

- [x] A per-node or per-cell adaptive theta mechanism is implemented
- [x] Denser regions use a smaller effective theta (more accurate forces)
- [x] Sparse regions use a larger effective theta (faster computation)
- [x] Layout quality for clustered graphs improves compared to fixed theta=0.5
- [x] Overall performance remains within 20% of fixed-theta Barnes-Hut
- [x] The feature can be enabled/disabled via a builder method

## Dependencies

### Prerequisite Stories

- GUP-310: Barnes-Hut GPU Repulsion Approximation ✅

## Testing Strategy

- Unit test: verify adaptive theta produces different effective theta values for
  nodes in dense vs sparse regions
- Integration test: layout a clustered graph and verify the layout separates
  clusters clearly
- Performance comparison: fixed vs adaptive theta at 10K and 100K nodes

## Risk Assessment

- **Medium**: Defining "density" in a way that's cheap to compute and meaningful
  for force accuracy is non-trivial. Cell mass/width ratio from the quadtree may
  suffice.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass
- [x] Lint clean
- [x] Retrospective added

## Implementation Summary

### What Was Implemented

Per-cell adaptive theta tuning for the Barnes-Hut repulsion approximation. When
enabled via `.adaptive_theta(true)`, each quadtree cell receives a density-based
effective theta instead of the global value. Dense cells get a smaller theta
(more accurate force calculation) while sparse cells get a larger theta (faster
approximation).

### Key Files Changed

- **`src/layout/types.rs`** — Added `effective_theta: f32` to `BHCell` (32→36
  bytes), added `adaptive_theta: bool` to `ForceDirected` config with builder
  method
- **`src/layout/quadtree.rs`** — Updated `build_quadtree()` to accept
  `base_theta` parameter, added `apply_adaptive_theta()` function with
  density-based tuning formula
- **`src/layout/barnes_hut.wgsl`** — Added `effective_theta` field to WGSL
  `BHCell` struct, changed theta comparison from `params.theta` to
  `cell.effective_theta`
- **`src/layout/engine.rs`** — Updated Barnes-Hut loop to pass theta config and
  optionally call `apply_adaptive_theta()`
- **`tests/layout_integration.rs`** — Added 4 new tests (builder, smoke, and
  clustered graph)

### Test Counts

- 8 quadtree unit tests (2 new: `effective_theta_set_to_base`,
  `adaptive_theta_varies_by_density`)
- 25 layout integration tests (4 new: `adaptive_theta_builder_defaults`,
  `adaptive_theta_builder_set`, `adaptive_theta_layout_produces_finite_positions`,
  `adaptive_theta_clustered_graph`)
- All 234 project tests pass
