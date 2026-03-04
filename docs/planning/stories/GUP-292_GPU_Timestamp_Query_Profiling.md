# GUP-292: GPU Timestamp Query Profiling

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress **Created**:
2026-03-03

## Context

GUP-291 introduced adaptive auto-tune calibration for the CPU/GPU dimming
threshold, using `std::time::Instant` for wall-clock timing. While this is
portable and sufficient for coarse-grained decisions, wgpu's timestamp query
feature (`Features::TIMESTAMP_QUERY`) can provide precise GPU-side timing of
compute shader dispatches. Using timestamp queries where available would improve
calibration accuracy, especially on systems where CPU-GPU synchronisation
latency skews wall-clock measurements.

## User Story

> "As a visualization developer running on hardware that supports GPU timestamp
> queries, I want the auto-tune calibration to use precise GPU-side timing so
> that the threshold selection is more accurate."

## Acceptance Criteria

- [ ] When `Features::TIMESTAMP_QUERY` is available, the auto-tune system uses
      GPU timestamp queries for compute shader dispatch timing
- [ ] When timestamp queries are unavailable, falls back to the existing
      `Instant`-based wall-clock timing
- [ ] Timestamp query results are reported via the existing
      `auto_tune_timings()` API
- [ ] No additional device features are required by default — timestamp queries
      are opportunistically enabled

## Technical Tasks

- [ ] Check for `Features::TIMESTAMP_QUERY` availability at device creation
- [ ] Create a reusable `GpuTimer` abstraction using `QuerySet` and
      `resolve_query_set`
- [ ] Integrate `GpuTimer` into the auto-tune calibration path
- [ ] Add fallback to `Instant` timing when timestamp queries are unsupported
- [ ] Add unit tests for the `GpuTimer` abstraction
- [ ] Add GPU integration tests comparing timestamp vs wall-clock accuracy

## Dependencies

### Prerequisite Stories

- GUP-291: Adaptive GPU Dimming Threshold ✅ — provides the auto-tune system

## Testing Strategy

- Unit tests for `GpuTimer` creation and query management
- GPU integration tests verifying timestamp query accuracy
- Fallback tests on backends without timestamp query support

## Risk Assessment

- **Low**: Timestamp queries are a well-supported wgpu feature on desktop
  backends. The fallback to Instant timing ensures universal compatibility.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
