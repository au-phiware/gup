# GUP-313: Adaptive Barnes-Hut Theta Tuning

## Story Overview

**Initiative**: Advanced Scale **Status**: 🚧 In Progress **Created**: 2025-07-20

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

- [ ] A per-node or per-cell adaptive theta mechanism is implemented
- [ ] Denser regions use a smaller effective theta (more accurate forces)
- [ ] Sparse regions use a larger effective theta (faster computation)
- [ ] Layout quality for clustered graphs improves compared to fixed theta=0.5
- [ ] Overall performance remains within 20% of fixed-theta Barnes-Hut
- [ ] The feature can be enabled/disabled via a builder method

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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint clean
- [ ] Retrospective added
