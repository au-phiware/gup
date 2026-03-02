# GUP-288: GPU Selection Mask Buffer

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 📋 Planned **Created**:
2025-07-19

## Context

GUP-279's `build_dimmed_instances` rebuilds the entire instance buffer on the
CPU whenever the selection changes. For small datasets (< 10K points) this is
fast, but for 100K+ points the CPU-side iteration becomes a bottleneck. This
story introduces a GPU-side selection mask buffer and a compute shader that
applies dimming directly on the GPU, avoiding the CPU rebuild entirely.

## User Story

> "As a visualization developer working with large datasets, I want selection
> dimming to be applied on the GPU so that selecting 10K items across two charts
> of 100K points each causes no frame-time regression exceeding 2 ms."

## Acceptance Criteria

- [ ] A `SelectionMaskBuffer` type maintains a GPU buffer of per-instance
      selection flags (0 or 1)
- [ ] A compute shader reads the mask buffer and multiplies the alpha channel
      of each instance's fill_color and stroke_color by dim_opacity when the
      flag is 0
- [ ] The mask buffer is updated incrementally: only changed flags are uploaded
      rather than rebuilding the entire buffer
- [ ] Performance: applying a 10K-item selection to a 100K-point chart completes
      in under 2 ms (GPU + upload)
- [ ] Integrates with `SharedSelectionState<K>` and the `DimInstance` pattern

## Technical Tasks

- [ ] Define `SelectionMaskBuffer` struct with GPU buffer management
- [ ] Write compute shader for alpha dimming
- [ ] Implement incremental mask update (diff against previous selection)
- [ ] Integrate with SharedSelectionState generation counter
- [ ] Benchmark with criterion: 100K points, 10K selection
- [ ] Write unit and integration tests

## Dependencies

### Prerequisite Stories

- GUP-279: Linked View Coordination ✅ — provides SharedSelectionState
- GUP-003: GPU Buffer Management ✅ — buffer pool for mask buffer

## Testing Strategy

- Unit tests for mask buffer CRUD
- GPU integration test: verify dimming applied correctly
- Performance benchmark: 100K points, 10K selection under 2 ms

## Risk Assessment

- **Medium**: Compute shader dispatch adds a pipeline synchronisation point.
  _Mitigation_: batch mask updates and dispatch once per frame.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Performance benchmark meets 2 ms target
