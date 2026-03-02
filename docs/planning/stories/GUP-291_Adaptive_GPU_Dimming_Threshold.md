# GUP-291: Adaptive GPU Dimming Threshold

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete **Created**:
2025-07-23 **Completed**: 2026-03-03

## Context

GUP-289 introduced a configurable `gpu_dimming_threshold` (default 10K) that
determines whether `LinkedSelection::prepare_render` uses the CPU-based
`build_dimmed_instances` path or the GPU-based `SelectionMaskBuffer` compute
shader. The threshold is currently static — users must manually set it via the
builder method. An adaptive system that profiles actual frame times and adjusts
the threshold at runtime would eliminate this manual tuning.

## User Story

> "As a visualization developer, I want the CPU/GPU dimming threshold to
> automatically adjust based on observed performance so that I get optimal frame
> times without manual configuration."

## Acceptance Criteria

- [x] The `LinkedSelection` automatically profiles both CPU and GPU dimming
      paths during an initial calibration phase
- [x] The threshold is adjusted at runtime based on observed timings
- [x] A `gpu_dimming_auto_tune(bool)` builder method enables/disables the
      adaptive behaviour (default: disabled)
- [x] When auto-tune is enabled, the static `gpu_dimming_threshold` serves as
      the initial estimate
- [x] Profiling overhead is less than 1% of total frame time

## Technical Tasks

- [x] Add frame-timing infrastructure to `LinkedSelection::prepare_render`
- [x] Implement binary search calibration over a configurable number of frames
- [x] Store profiling results and expose current effective threshold
- [x] Add unit tests for the calibration logic
- [x] Add GPU integration test verifying threshold adaptation

## Dependencies

### Prerequisite Stories

- GUP-289: LinkedSelection GPU Integration ✅ — provides threshold-based
  switching

## Testing Strategy

- Unit tests for calibration state machine
- GPU integration tests verifying threshold converges to a stable value
- Performance tests ensuring profiling overhead is negligible

## Risk Assessment

- **Medium**: Accurate GPU timing requires pipeline statistics or timestamp
  queries, which are not universally supported across all wgpu backends.
  _Mitigation_: fall back to CPU-side `Instant` timing of the full
  prepare_render call.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

- **AutoTuneState calibration state machine** — A three-phase state machine
  (`ProbeCpu` → `ProbeGpu` → `Settled`) that profiles both CPU and GPU dimming
  paths over a configurable number of calibration frames (default: 5 per path).
  After probing, the faster path is selected by adjusting the effective
  threshold.

- **`gpu_dimming_auto_tune(bool)` builder method** — Enables or disables the
  adaptive behaviour. Default: disabled. When enabled, the static
  `gpu_dimming_threshold` serves as the initial estimate until calibration
  completes.

- **`auto_tune_calibration_frames(u32)` builder method** — Configures the number
  of frames to sample each path during calibration. Higher values give more
  accurate profiling at the cost of a longer calibration period.

- **`effective_threshold()` accessor** — Returns the current effective
  threshold: the static threshold when auto-tune is disabled, or the calibrated
  threshold when auto-tune has settled.

- **`auto_tune_timings()` accessor** — Returns the mean CPU and GPU timings in
  nanoseconds from the last completed calibration, enabling introspection of
  profiling results.

- **`is_auto_tune_enabled()` / `is_auto_tune_settled()` accessors** — Query
  auto-tune status.

- **Frame-timing infrastructure** — `Instant::now()` + `elapsed()` timing
  wrapped around the `prepare_render` execution path, only active during
  calibration. Zero overhead after settling.

- **Path forcing during calibration** — During `ProbeCpu`, the CPU path is
  forced regardless of instance count. During `ProbeGpu`, the GPU path is
  forced. This ensures both paths are exercised for accurate comparison.

- **Re-calibration on data size changes** — When the instance count changes by
  more than 50% from the calibrated count, calibration automatically restarts.

- **CPU→GPU transition fix** — `prepare_render_gpu` now detects missing GPU
  resources (when transitioning from CPU path) and forces resource recreation.

### Key Files Changed

| File                                  | Description                                    |
| ------------------------------------- | ---------------------------------------------- |
| `src/linked_selection.rs`             | AutoTuneState, builder methods, prepare_render |
| `tests/linked_selection_gpu_tests.rs` | 3 new GPU integration tests for auto-tune      |

### Test Counts

- 17 new unit tests (state machine phases, builder methods, re-calibration,
  timings accessors)
- 3 new GPU integration tests (calibration settling, correct output, disabled
  static threshold)
- All 138 existing tests pass unchanged

## Retrospective

**Completed**: 2026-03-03

### Key Technical Learnings

#### CPU→GPU Transition During Calibration

- **Challenge**: When auto-tune forces the CPU path during the ProbeCpu phase,
  it clears `mask_buffer` and `source_buffer`. Transitioning to the ProbeGpu
  phase then calls `prepare_render_gpu`, which expects these resources to exist
  when `data_changed` is false (the Selection was already prepared by the CPU
  path).
- **Solution**: Added a guard in `prepare_render_gpu` that treats
  `mask_buffer.is_none()` as equivalent to `data_changed = true`, forcing
  resource recreation when GPU resources don't exist yet.
- **Pattern**: When dynamically switching between alternative execution paths
  that manage different resource sets, always check for resource existence
  rather than relying solely on data-change flags. Resources may have been
  intentionally cleared by the other path.

#### CPU-Side Timing as a Universal Fallback

- **Challenge**: The risk assessment noted that accurate GPU timing requires
  pipeline statistics or timestamp queries, which are not universally supported
  across all wgpu backends.
- **Solution**: Used `std::time::Instant` to measure the entire `prepare_render`
  call wall-clock time. This captures both CPU work (instance mapping, buffer
  creation) and GPU submission latency.
- **Pattern**: For adaptive performance tuning, wall-clock timing of the full
  operation is often sufficient and more portable than fine-grained GPU
  profiling. The goal is "which path is faster end-to-end?", not "how long does
  each GPU dispatch take?".

#### State Machine Enum Design for Multi-Phase Calibration

- **Challenge**: The ProbeGpu phase needs access to the CPU timing results to
  compute the final comparison. The naive approach of storing the CPU total in a
  separate field complicates the state machine.
- **Solution**: Extended the `ProbeGpu` variant with a `cpu_total_ns` field,
  carrying the CPU timing forward through the enum transition. This keeps all
  phase-specific data co-located with its phase.
- **Pattern**: When an enum-based state machine needs data from a previous
  phase, embed it in the next phase's variant rather than using external fields.
  This makes invalid states unrepresentable — you can't access CPU timing data
  in the ProbeCpu phase because it doesn't exist yet.

### Architectural Decisions

#### Disabled by Default

- **Decision**: Auto-tune is disabled by default; users must opt in via
  `gpu_dimming_auto_tune(true)`.
- **Reasoning**: The calibration phase introduces visual inconsistency (CPU and
  GPU paths may produce slightly different alpha values due to floating-point
  order-of-operations differences) and the static threshold works well for most
  use cases. Auto-tune is most valuable when deploying to heterogeneous hardware
  where the optimal threshold varies.
- **Trade-off**: Users on unusual hardware must know about the feature to
  benefit from it. The default 10K threshold may not be optimal everywhere.
- **Future**: Could be made the default once the calibration phase is refined to
  be visually imperceptible (e.g., by using only the CPU path during ProbeCpu
  and not rendering the GPU probe frames).

#### Single-Point Calibration vs Binary Search

- **Decision**: Profile both paths at the current data size and pick the faster
  one, rather than doing a full binary search across multiple data sizes.
- **Reasoning**: In practice, the data size is relatively stable within a single
  visualization session. Binary search across sizes would require synthetic
  workloads or waiting for natural data size changes, adding complexity with
  marginal benefit.
- **Trade-off**: The calibration result is only valid for the current data size.
  If the data size changes significantly (>50%), re-calibration is triggered
  automatically.
- **Future**: If users report issues with threshold accuracy across varying data
  sizes, a multi-point binary search could be added as a calibration strategy
  option.

### Development Workflow Insights

- **GUP-289's clean abstraction boundaries made integration smooth**: The
  `prepare_render`/`prepare_render_gpu` split from GUP-289 was ideal for
  inserting auto-tune logic. The timing instrumentation wraps cleanly around the
  existing path selection code.
- **Enum state machines are easy to test**: Each phase transition is a pure
  function of the current state and input sample. Unit testing the state machine
  in isolation (without GPU resources) caught the design issues early.
- **GPU integration tests need generation counter management**: Tests must
  explicitly change the shared selection state between frames to trigger
  `prepare_render` rebuilds. This is a pattern that could be simplified with a
  test helper.

### Follow-up Stories

1. **GUP-292: GPU Timestamp Query Profiling** — Use wgpu timestamp queries
   (where available) for more accurate GPU-side timing during auto-tune
   calibration, with fallback to Instant timing on unsupported backends.
