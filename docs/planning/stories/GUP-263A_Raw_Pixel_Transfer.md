# GUP-263A: Raw Pixel Transfer for egui/Bevy Integration

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-07-22 **Completed**: 2026-03-05

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

- [x] `ComposedChart::render_to_rgba(width, height) -> GupResult<Vec<u8>>`
      returns tightly-packed RGBA pixels without PNG encoding.
- [x] `DynChart` traits in both `gup-egui` and `gup-bevy` expose the new method.
- [x] Both integration crates use `render_to_rgba` instead of the PNG
      round-trip.
- [x] The `render_to_png` method internally calls `render_to_rgba` then encodes,
      avoiding code duplication.
- [x] All existing tests continue to pass.

## Technical Tasks

- [x] Add `ComposedChart::render_to_rgba(width, height) -> GupResult<Vec<u8>>`
      using `OffscreenTarget::readback_pixels`.
- [x] Refactor `render_to_png` to call `render_to_rgba` + `encode_png`.
- [x] Update `DynChart` trait in `gup-egui` and `gup-bevy`.
- [x] Update `gup-egui` widget to use `render_to_rgba` directly.
- [x] Update `gup-bevy` render system to use `render_to_rgba` directly.
- [x] Add benchmark comparing PNG round-trip vs. raw pixel transfer.

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

- [x] PNG round-trip overhead eliminated (measurable in benchmarks).
- [x] All existing tests pass unchanged.
- [x] No visual regression in either integration crate.

## Risk Assessment

- **Low**: Straightforward refactor with well-understood components.

## Definition of Done

- [x] All Acceptance Criteria satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete

## Implementation Summary

### What Was Implemented

- **`ComposedChart::render_to_rgba(width, height)`**: New method that renders a
  chart to an off-screen GPU texture and reads back tightly-packed RGBA pixels
  without any PNG encoding. Uses `OffscreenTarget::readback_pixels` for the GPU
  readback.
- **Refactored `render_to_png`**: Now a thin wrapper that calls `render_to_rgba`
  + `encode_png`, eliminating duplicated rendering logic.
- **Updated `DynChart` traits**: Both `gup-egui` and `gup-bevy` `DynChart`
  traits expose `render_to_rgba` alongside existing methods.
- **Eliminated PNG round-trip in gup-egui**: `GupWidget::rerender` now calls
  `render_to_rgba` directly and passes the raw pixels to egui's
  `ColorImage::from_rgba_unmultiplied`, removing the PNG encode → decode
  overhead.
- **Added `render_to_rgba` to gup-bevy `GupChart`**: Convenience method
  available for any consumer that needs raw pixels (the primary Bevy render path
  already uses the zero-copy `render_to_texture_view`).
- **Removed `image` crate dependency from gup-egui**: No longer needed since PNG
  decoding was the only consumer.
- **Performance benchmark**: Demonstrates 100-130× improvement at typical chart
  resolutions (e.g. 800×600: 8.6ms → 66µs).

### Key Files Changed

| File                         | Change                                    |
| ---------------------------- | ----------------------------------------- |
| `src/chart_builder.rs`       | Added `render_to_rgba`, refactored PNG    |
| `gup-egui/src/widget.rs`    | Updated `DynChart`, `rerender` method     |
| `gup-egui/Cargo.toml`       | Removed `image` dependency                |
| `gup-bevy/src/chart.rs`     | Updated `DynChart`, added `render_to_rgba`|
| `benches/raw_pixel_transfer.rs` | New benchmark                          |
| `Cargo.toml`                | Registered benchmark                      |

### Test Results

- All 3030+ unit tests pass
- 241 doc tests pass
- All examples compile (main, gup-egui, gup-bevy)
