# GUP-291: Adaptive GPU Dimming Threshold

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress
**Created**: 2025-07-23

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

- [ ] The `LinkedSelection` automatically profiles both CPU and GPU dimming
      paths during an initial calibration phase
- [ ] The threshold is adjusted at runtime based on observed timings
- [ ] A `gpu_dimming_auto_tune(bool)` builder method enables/disables the
      adaptive behaviour (default: disabled)
- [ ] When auto-tune is enabled, the static `gpu_dimming_threshold` serves as
      the initial estimate
- [ ] Profiling overhead is less than 1% of total frame time

## Technical Tasks

- [ ] Add frame-timing infrastructure to `LinkedSelection::prepare_render`
- [ ] Implement binary search calibration over a configurable number of frames
- [ ] Store profiling results and expose current effective threshold
- [ ] Add unit tests for the calibration logic
- [ ] Add GPU integration test verifying threshold adaptation

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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
