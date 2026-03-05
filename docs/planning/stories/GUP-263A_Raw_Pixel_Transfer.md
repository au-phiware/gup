# GUP-263A: Raw Pixel Transfer for egui/Bevy Integration

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-07-22

## Context

Both `gup-egui` and `gup-bevy` currently render charts by calling
`ComposedChart::render_to_png`, then decoding the PNG bytes back to RGBA pixels
before uploading to the host framework's texture system. This PNG encode/decode
round-trip adds ~1–2ms of unnecessary overhead per re-render.

A dedicated `render_to_rgba` method on `ComposedChart` would eliminate the
round-trip by returning tightly-packed RGBA pixel data directly. This also
provides the raw pixel buffer that GUP-268 (PNG Export) needs, reducing
duplicated infrastructure.

## User Story

> "As a gup-egui or gup-bevy user, I want chart re-renders to be as fast as
> possible so that interactive visualizations remain fluid even at high update
> frequencies."

## Acceptance Criteria

- [ ] `ComposedChart::render_to_rgba(width, height) -> GupResult<Vec<u8>>`
      returns tightly-packed RGBA pixels without PNG encoding.
- [ ] `DynChart` traits in both `gup-egui` and `gup-bevy` expose the new method.
- [ ] Both integration crates use `render_to_rgba` instead of the PNG
      round-trip.
- [ ] The `render_to_png` method internally calls `render_to_rgba` then encodes,
      avoiding code duplication.
- [ ] All existing tests continue to pass.

## Technical Tasks

- [ ] Add `ComposedChart::render_to_rgba(width, height) -> GupResult<Vec<u8>>`
      using `OffscreenTarget::readback_pixels`.
- [ ] Refactor `render_to_png` to call `render_to_rgba` + `encode_png`.
- [ ] Update `DynChart` trait in `gup-egui` and `gup-bevy`.
- [ ] Update `gup-egui` widget to use `render_to_rgba` directly.
- [ ] Update `gup-bevy` render system to use `render_to_rgba` directly.
- [ ] Add benchmark comparing PNG round-trip vs. raw pixel transfer.

## Dependencies

### Prerequisite Stories

- GUP-263: egui Integration ✅ — Established the PNG round-trip pattern.
- GUP-262: Bevy Integration ✅ — Uses the same PNG round-trip pattern.

### Enables Stories

- GUP-268: PNG Export — Shares the `render_to_rgba` infrastructure.

## Testing Strategy

- **Unit tests**: Verify `render_to_rgba` returns correct pixel dimensions and
  RGBA format.
- **Integration tests**: Confirm both gup-egui and gup-bevy produce identical
  visual output with the new path.
- **Performance benchmark**: Measure render latency with and without PNG
  round-trip.

## Success Metrics

- [ ] PNG round-trip overhead eliminated (measurable in benchmarks).
- [ ] All existing tests pass unchanged.
- [ ] No visual regression in either integration crate.

## Risk Assessment

- **Low**: Straightforward refactor with well-understood components.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete
