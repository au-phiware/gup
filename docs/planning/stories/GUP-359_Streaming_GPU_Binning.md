# GUP-359: Streaming GPU Binning

## Story Overview

**Initiative**: Performance  
**Status**: 📋 Planned  
**Created**: 2025-07-18

## Context

GUP-297's `GpuBinner` re-uploads the entire dataset on every `bin()` call. For
real-time streaming scenarios (e.g. live sensor data feeding a heatmap), this
means O(N) upload cost per frame even if only a few new records arrived. An
incremental path that appends new records to the existing GPU buffers and
re-dispatches only the new range would reduce per-frame cost to O(delta).

## User Story

> "As a developer building a real-time monitoring dashboard, I want the GPU
> heatmap to update incrementally as new data arrives so that frame latency stays
> constant regardless of total dataset size."

## Acceptance Criteria

- [ ] `GpuBinner` supports an `append()` method that uploads only new records.
- [ ] Accumulator buffers are preserved across appends (no re-zeroing).
- [ ] A `reset()` method clears accumulators for a fresh binning pass.
- [ ] Integration with `StreamingDataSource` for automatic incremental updates.
- [ ] Equivalence test: incremental result matches full re-bin result.
- [ ] Per-append latency is O(delta) not O(N_total).

## Technical Tasks

- [ ] Extend `GpuBinner` with persistent accumulator buffers.
- [ ] Implement `append()` that writes new data to the end of input buffers and
      dispatches only the new range.
- [ ] Implement `reset()` to clear accumulators.
- [ ] Add `readback()` method to extract current `BinGrid` without dispatching.
- [ ] Wire into `StreamingDataSource` callback.
- [ ] Add equivalence and latency tests.

## Dependencies

### Prerequisite Stories

- GUP-297: GPU Compute Shader 2D Binning ✅

## Testing Strategy

- Equivalence: append 1000 records in 10 batches of 100 vs single batch of 1000.
- Latency: measure append time for batches of 100, 1k, 10k records.

## Risk Assessment

- **Medium**: Persistent GPU buffers need careful lifecycle management to avoid
  leaks or stale data.
- **Low**: Buffer re-allocation on growth may cause frame stutters; ring-buffer
  or pre-allocated capacity can mitigate.

## Definition of Done

- [ ] Incremental append API implemented and tested
- [ ] StreamingDataSource integration working
- [ ] Latency benchmarks documented
