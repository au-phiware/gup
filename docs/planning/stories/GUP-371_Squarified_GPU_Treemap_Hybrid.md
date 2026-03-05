# GUP-371: Squarified GPU Treemap (Hybrid Approach)

## Story Overview

**Initiative**: Advanced Scale **Status**: 💡 New **Created**: 2025-07-27

## Context

GUP-312 migrated SliceDice and Binary treemap algorithms to GPU compute shaders.
The Squarified algorithm was left CPU-only because it has sequential
row-building dependencies where each row's composition depends on the aspect
ratio achieved by the previous row. However, a hybrid approach could lay out
top-level nodes on CPU (few nodes, sequential) and dispatch leaf-level layout to
GPU (many nodes, parallel).

## User Story

> "As a developer using Squarified treemaps with 100K+ nodes, I want the layout
> to be partially GPU-accelerated so that it completes faster than purely
> CPU-based computation."

## Acceptance Criteria

- [ ] Squarified algorithm uses GPU for leaf-level layout of large sibling
      groups.
- [ ] Threshold for CPU-to-GPU handoff is configurable.
- [ ] Results match pure CPU reference within 0.01%.
- [ ] Measurable speedup over CPU-only for 100K+ node trees.

## Dependencies

### Prerequisite Stories

- GUP-312: GPU Compute Treemap ✅
- GUP-260: GPU Treemap Layout ✅

## Testing Strategy

- GPU-vs-CPU comparison tests.
- Performance benchmarks comparing hybrid vs CPU-only.
- Run with `--test-threads=1`.

## Risk Assessment

- **High**: The CPU-GPU handoff overhead may negate speedup for typical tree
  structures. The approach is only beneficial for very large trees with wide
  branching factors.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
