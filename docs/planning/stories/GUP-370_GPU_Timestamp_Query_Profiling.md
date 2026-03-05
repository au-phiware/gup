# GUP-370: GPU Timestamp Query Profiling

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-27

## Context

GUP-312 implemented GPU compute shaders for treemap layout but verified
performance through wall-clock timing rather than GPU-side timestamp queries.
wgpu supports timestamp queries that provide nanosecond-precision GPU-side timing
for compute and render passes. Adding this instrumentation would enable precise
performance measurement, regression detection, and comparison across GPU vendors.

## User Story

> "As a developer optimising GPU compute workloads, I want timestamp query
> instrumentation on all compute passes so that I can measure GPU execution time
> precisely and detect performance regressions."

## Acceptance Criteria

- [ ] Timestamp queries before and after each treemap compute dispatch.
- [ ] Results readable via `TimestampQuerySet` readback.
- [ ] Performance report includes per-pass timing (prefix sum, layout per depth).
- [ ] Instrumentation has < 1% overhead when enabled.
- [ ] Feature-gated so it can be disabled in production builds.

## Technical Tasks

- [ ] Create `GpuTimingContext` struct for managing timestamp query sets.
- [ ] Add timestamp writes to treemap compute pass descriptors.
- [ ] Add readback and reporting for timestamp results.
- [ ] Feature-gate behind `gpu-profiling` feature flag.

## Dependencies

### Prerequisite Stories

- GUP-312: GPU Compute Treemap ✅
- GUP-015: GPU Debugging Tools ✅

## Testing Strategy

- Verify timestamp values are monotonically increasing.
- Ensure disabling profiling has zero overhead.
- Run with `--test-threads=1`.

## Risk Assessment

- **Low**: wgpu timestamp queries are well-documented and supported on all
  backends. Main risk is per-backend variance in timestamp resolution.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
