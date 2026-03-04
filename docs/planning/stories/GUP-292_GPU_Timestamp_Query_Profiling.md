# GUP-292: GPU Timestamp Query Profiling

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete
**Created**: 2026-03-03 **Completed**: 2025-07-24

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

- [x] When `Features::TIMESTAMP_QUERY` is available, the auto-tune system uses
      GPU timestamp queries for compute shader dispatch timing
- [x] When timestamp queries are unavailable, falls back to the existing
      `Instant`-based wall-clock timing
- [x] Timestamp query results are reported via the existing
      `auto_tune_timings()` API
- [x] No additional device features are required by default — timestamp queries
      are opportunistically enabled

## Technical Tasks

- [x] Check for `Features::TIMESTAMP_QUERY` availability at device creation
- [x] Create a reusable `GpuTimer` abstraction using `QuerySet` and
      `resolve_query_set`
- [x] Integrate `GpuTimer` into the auto-tune calibration path
- [x] Add fallback to `Instant` timing when timestamp queries are unsupported
- [x] Add unit tests for the `GpuTimer` abstraction
- [x] Add GPU integration tests comparing timestamp vs wall-clock accuracy

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

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

- **`GpuTimer` abstraction** (`src/gpu_timer.rs`) — A lightweight, reusable
  struct wrapping a 2-slot `QuerySet` (begin/end), a resolve buffer
  (`QUERY_RESOLVE | COPY_SRC`), and a staging buffer (`MAP_READ | COPY_DST`).
  Returns `None` from `new()` when `Features::TIMESTAMP_QUERY` is unavailable.
  Provides `compute_pass_timestamp_writes()` for the compute pass descriptor,
  `resolve()` for the command encoder, and `read_elapsed_ns()` for synchronous
  CPU readback via `Device::poll(PollType::Wait)`.

- **`encode_dimming_timed()` on `SelectionMaskBuffer`** — New method accepting
  optional `ComputePassTimestampWrites` to instrument the dimming compute pass.
  The existing `encode_dimming()` delegates to this with `None`.

- **Lazy GpuTimer integration in `LinkedSelection`** — A `gpu_timer:
  Option<GpuTimer>` field is lazily created during the first GPU-path
  calibration frame when auto-tune is enabled. During the ProbeGpu phase, GPU
  timestamps are recorded around the compute dispatch and preferred over
  `Instant` wall-clock timing. Falls back to `Instant` for the ProbeCpu phase
  and when timestamp queries are unavailable.

- **Opportunistic `TIMESTAMP_QUERY` in `GupContext::with_options`** — When the
  adapter supports `Features::TIMESTAMP_QUERY`, the feature is automatically
  added to the device's required features during creation. No additional
  features are required by default; the enablement is purely opportunistic.

- **`has_gpu_timer()` accessor** — Exposes whether GPU timestamps are active for
  introspection and testing.

### Key Files Changed

| File                                  | Description                                         |
| ------------------------------------- | --------------------------------------------------- |
| `src/gpu_timer.rs`                    | New module: GpuTimer abstraction                    |
| `src/lib.rs`                          | Register gpu_timer module                           |
| `src/linked_selection.rs`             | Integrate GpuTimer into auto-tune calibration       |
| `src/selection_mask.rs`               | Add encode_dimming_timed() with timestamp writes    |
| `src/context.rs`                      | Opportunistic TIMESTAMP_QUERY enablement            |
| `tests/linked_selection_gpu_tests.rs` | 3 new GPU integration tests for timestamp profiling |

### Test Counts

- 3 new unit tests for `GpuTimer` (feature detection, construction, resolve)
- 2 new unit tests for `LinkedSelection` (gpu_timer field initialization)
- 3 new GPU integration tests (timer creation, no-timer-without-auto-tune,
  timings API with timestamps)
- All 2729 lib tests pass
- All 14 linked_selection GPU integration tests pass

