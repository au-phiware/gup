# GUP-317: Full-Content Screenshot Capture

## Story Overview

**Initiative**: Ecosystem Integration **Status**: ✅ Complete **Created**:
2025-03-04 **Completed**: 2025-03-05

## Context

GUP-265 delivered the `GupApp` application shell with a built-in `S` shortcut
for screenshots. However, the current implementation captures a blank frame
(clear colour only) because surface textures are configured with
`RENDER_ATTACHMENT` usage alone — they lack `COPY_SRC`, which is required to
read pixel data back from the GPU.

Additionally, `RenderFrame`'s fields (`command_encoder`, `surface_texture`) are
private, so external code cannot inject texture copy commands before the frame
is presented.

This story adds the missing infrastructure so that the `S` shortcut captures the
actual rendered chart content.

## User Story

> "As a user of GupApp, I want the `S` key to save a PNG of exactly what I see
> in the window, so I can share or archive my visualisations."

## Acceptance Criteria

- [x] `SurfaceConfigBuilder` gains an optional `usage: Option<TextureUsages>`
      field that is merged with the default `RENDER_ATTACHMENT` when the surface
      is configured.
- [x] `GupApp` configures its surface with `RENDER_ATTACHMENT | COPY_SRC`.
- [x] `RenderFrame` gains a `capture_texture_copy` method (or equivalent) that
      encodes a `copy_texture_to_buffer` command using the internal command
      encoder, returning a staging buffer handle.
- [x] Pressing `S` in a `GupApp` window saves a PNG that faithfully reproduces
      the most recently rendered frame content (not a blank frame).
- [x] The screenshot mechanism works on Vulkan, Metal, and DX12 backends.
- [x] All existing tests continue to pass.

## Dependencies

### Prerequisite Stories

- GUP-265: winit Application Shell ✅

### Enables Stories

- Any story requiring framebuffer readback (visual regression testing, server-
  side rendering confirmation).

## Technical Tasks

- [x] Add `usage: Option<TextureUsages>` to `SurfaceConfigBuilder`.
- [x] In `add_surface_with_config`, merge builder usage with
      `RENDER_ATTACHMENT`.
- [x] Add `RenderFrame::capture_texture_copy(&mut self, width, height)` that
      creates a staging buffer and encodes a copy command.
- [x] In `GupAppRunner::render_frame`, when `screenshot_requested`, call
      `capture_texture_copy` before `finish()`, then map the buffer and save.
- [x] Add tests for `SurfaceConfigBuilder` with custom usage.
- [x] Add a test that headless frames correctly report no surface texture and
      that `capture_texture_copy` returns an error without a surface.

## Testing Strategy

- **Unit tests**: verify `SurfaceConfigBuilder` merges usage flags correctly.
- **GPU integration test**: render a known clear colour, capture, and assert
  pixel values.
- **Visual validation**: press `S` in `hello_world` and confirm the PNG matches
  the window content.

## Risk Assessment

- **Medium**: Not all GPU backends guarantee `COPY_SRC` on surface textures.
  _Mitigation_: Query `SurfaceCapabilities::usages` and fall back to offscreen
  rendering when `COPY_SRC` is not supported.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What Was Implemented

1. **`SurfaceConfigBuilder.usage` field** — New optional `TextureUsages` field
   on the builder, merged with `RENDER_ATTACHMENT` in `add_surface_with_config`.
   Includes `with_usage()` builder method.

2. **`GupContext::with_surface_config` constructor** — New async constructor
   that takes a window and a `SurfaceConfigBuilder`, enabling callers to
   configure surface textures with custom usages at creation time.

3. **`RenderFrame::capture_texture_copy` method** — Encodes a
   `copy_texture_to_buffer` command on the frame's internal command encoder,
   returning a `CapturedFrame` staging buffer handle with dimensions and
   padding metadata.

4. **`CapturedFrame` struct** — Public struct holding the staging buffer, width,
   height, and padded row bytes, enabling callers to map the buffer after
   `finish()` submits the GPU work.

5. **`GupApp` screenshot rewrite** — Surface is now configured with
   `RENDER_ATTACHMENT | COPY_SRC`. Screenshot is deferred via a
   `screenshot_requested` flag, consumed during `render_frame()`. The actual
   rendered content is captured (not a blank frame).

### Key Files Changed

| File | Change |
|------|--------|
| `src/context.rs` | Added `SurfaceConfigBuilder.usage`, `with_surface_config`, `CapturedFrame`, `capture_texture_copy`, and 6 new tests |
| `src/app.rs` | Rewrote `GupAppRunner` to use `with_surface_config` with `COPY_SRC`, deferred screenshot capture, `save_captured_frame` |

### Test Counts

- 6 new tests added (4 unit tests for `SurfaceConfigBuilder`, 2 GPU tests)
- All 3042+ existing tests continue to pass
