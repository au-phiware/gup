# GUP-356: InteractionSystem Dynamic Result Buffer

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 💡 New **Created**:
2025-07-27

## Context

The `InteractionSystem` allocates a fixed `max_results = 100_000` buffer at
creation time. When a region query returns more hits than this limit the GPU
compute pass either silently truncates or triggers a device-lost validation
error. GUP-286 discovered this limitation when testing GPU-accelerated brush
queries on 500K–1M mark datasets with selection regions covering >10% of the
data space.

## User Story

> "As a visualization developer selecting large regions from million-point
> datasets, I want the GPU query to return all matching marks regardless of the
> result count, so that brush selection is accurate at any scale."

## Acceptance Criteria

- [ ] `InteractionSystem` dynamically grows the result buffer when a query
      produces more hits than the current capacity.
- [ ] Existing queries with ≤100K results see no performance regression.
- [ ] A 25% region query on 1M marks returns the same hit count as the CPU
      `filter_by_rect` path.
- [ ] No GPU validation errors for result counts up to 1M.

## Technical Tasks

- [ ] Detect result overflow in `download_results` (e.g. via an atomic counter
      written by the compute shader).
- [ ] Re-allocate a larger result buffer and re-dispatch the query on overflow.
- [ ] Add a configurable `max_result_capacity` upper bound to prevent unbounded
      GPU memory growth.
- [ ] Add tests with 500K and 1M mark region queries covering >20% of the data
      space.

## Dependencies

### Prerequisite Stories

- GUP-286: GPU-Accelerated Brush Region Query ✅
- GUP-012: GPU Interaction System ✅

## Testing Strategy

- Integration test: 1M marks, 25% region query, verify result count matches CPU
  path.
- Performance test: Ensure no regression for small result sets.

## Risk Assessment

- **Medium**: Re-dispatching a GPU query on overflow adds latency; may need
  heuristic pre-sizing based on region area.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
