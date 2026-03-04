# GUP-353: Android Chart Rendering Integration

## Story Overview

**Initiative**: Mobile **Status**: 📋 Planned **Created**: 2025-07-25

## Context

GUP-271 delivered the Android platform shim (JNI bridge, surface lifecycle,
touch translation) but the `gup_render_frame()` function is a stub that
returns `true` without producing visible output. This story wires the chart
builder pipeline into the Android render path so that `GupSurfaceView`
displays actual GPU-accelerated data visualisations.

This mirrors GUP-272 (iOS Chart Rendering Integration) which does the same
for the iOS platform.

## User Story

> "As an Android app developer, I want to see actual chart content rendered
> in my `GupSurfaceView` so that I can use Gup for real data visualisation
> rather than a blank surface."

## Acceptance Criteria

- [ ] `gup_render_frame()` acquires the current surface texture, renders a
      chart scene, and presents the frame.
- [ ] A Kotlin-side API allows setting the chart configuration (data source,
      mark type, scales) on the `GupSurfaceView`.
- [ ] The example app in `examples/android/` displays a visible line chart
      with simulated streaming data.
- [ ] Frame timing remains under 16ms (60 fps) on the CI emulator with
      ≤ 1,000 data points.

## Dependencies

### Prerequisite Stories

- GUP-271: Android Platform Support ✅ — provides the JNI bridge and surface
  lifecycle.
- GUP-018: Chart Builders ✅ — provides the fluent chart builder API.
- GUP-013: Event Handling System 📋 — provides unified event dispatch.

## Testing Strategy

- Unit tests for chart scene setup and tear-down.
- Integration test on emulator verifying a non-black frame is produced.
- Performance benchmark: frame time ≤ 16ms with 1,000 data points.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Example app shows visible chart on emulator
- [ ] Story status updated to ✅ Complete
