# GUP-375: COPY_SRC Surface Capability Fallback

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 📋 Planned **Created**:
2025-03-05

## Context

GUP-317 added `COPY_SRC` to the surface texture configuration so that
screenshots can read back the rendered frame. However, not all GPU backends
guarantee that `COPY_SRC` is available on surface textures. If the adapter's
`SurfaceCapabilities::usages` does not include `COPY_SRC`, the surface
configuration will silently strip the flag and screenshots will fail.

This story adds a fallback path: when `COPY_SRC` is not supported on the
surface, the screenshot mechanism renders to an intermediate offscreen texture
(which always supports `COPY_SRC`) and copies from there.

## User Story

> "As a user of GupApp on a platform with limited surface capabilities, I still
> want the `S` key to capture a screenshot, even if the surface texture doesn't
> support `COPY_SRC`."

## Acceptance Criteria

- [ ] `GupApp::initialize` queries `SurfaceCapabilities::usages` and only
      requests `COPY_SRC` when supported.
- [ ] When `COPY_SRC` is not available, the screenshot path creates an offscreen
      render target, re-renders the frame to it, and reads back from there.
- [ ] A diagnostic message is logged at startup indicating which screenshot path
      is active.
- [ ] Screenshots produce correct output on both paths.
- [ ] All existing tests continue to pass.

## Technical Tasks

- [ ] In `GupApp::initialize`, check `SurfaceCapabilities::usages` before
      requesting `COPY_SRC`.
- [ ] Store a boolean flag `copy_src_supported` on `GupAppRunner`.
- [ ] When `copy_src_supported` is false, fall back to double-render: render to
      an offscreen texture with `COPY_SRC`, then read back.
- [ ] Add tests verifying the fallback path produces correct pixel data.

## Dependencies

### Prerequisite Stories

- GUP-317: Full-Content Screenshot Capture ✅

## Testing Strategy

- **Unit tests**: verify capability detection logic.
- **GPU integration test**: force the fallback path and verify pixel readback.

## Risk Assessment

- **Low**: The fallback path reuses existing `OffscreenTarget` infrastructure
  from the `export::png` module.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in story file and INDEX.md
- [ ] Retrospective added to story document
