# GUP-263D: Pipeline Format Validation

## Story Overview

**Initiative**: Developer Experience **Status**: 💡 New **Created**: 2025-07-22

## Context

During GUP-263B implementation, a texture format mismatch between chart render
pipelines (`Bgra8UnormSrgb`) and the render target (`Rgba8UnormSrgb`) caused
data marks to silently not render. wgpu did not produce a validation error — the
draw commands were simply skipped. This class of bug is hard to diagnose because
axis infrastructure (compiled per-frame) renders correctly while lazily-compiled
mark pipelines use a stale format.

## User Story

> "As a developer building Gup integrations, I want debug-mode assertions that
> catch pipeline-format/render-target mismatches at draw time so that I get an
> immediate error instead of silently missing content."

## Acceptance Criteria

- [ ] Debug-mode assertion added to `ComposedChart::render_to_texture_view` that
      checks the pipeline's target format matches the supplied `surface_format`.
- [ ] Assertion fires with a clear error message indicating the mismatch.
- [ ] Assertion is `debug_assert!` only — no runtime cost in release builds.
- [ ] Existing tests continue to pass (no false positives).

## Technical Tasks

- [ ] Add format tracking to pipeline structs (if not already stored).
- [ ] Insert `debug_assert_eq!` checks before draw commands.
- [ ] Add a test that intentionally mismatches formats and verifies the
      assertion fires.

## Dependencies

### Prerequisite Stories

- GUP-263B: Shared wgpu Device for egui ✅

## Testing Strategy

- Unit test with `#[should_panic]` verifying the assertion fires on mismatch.
- Existing test suite passes without regressions.

## Risk Assessment

- **Low**: Debug-only assertions with no release impact.

## Definition of Done

- [ ] Debug assertions added and tested
- [ ] No false positives in existing test suite
- [ ] Story status updated to ✅ Complete
