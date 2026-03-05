# GUP-317: Full-Content Screenshot Capture

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 🚧 In Progress **Created**:
2025-03-04

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

- [ ] `SurfaceConfigBuilder` gains an optional `usage: Option<TextureUsages>`
      field that is merged with the default `RENDER_ATTACHMENT` when the surface
      is configured.
- [ ] `GupApp` configures its surface with `RENDER_ATTACHMENT | COPY_SRC`.
- [ ] `RenderFrame` gains a `capture_texture_copy` method (or equivalent) that
      encodes a `copy_texture_to_buffer` command using the internal command
      encoder, returning a staging buffer handle.
- [ ] Pressing `S` in a `GupApp` window saves a PNG that faithfully reproduces
      the most recently rendered frame content (not a blank frame).
- [ ] The screenshot mechanism works on Vulkan, Metal, and DX12 backends.
- [ ] All existing tests continue to pass.

## Dependencies

### Prerequisite Stories

- GUP-265: winit Application Shell ✅

### Enables Stories

- Any story requiring framebuffer readback (visual regression testing, server-
  side rendering confirmation).

## Technical Tasks

- [ ] Add `usage: Option<TextureUsages>` to `SurfaceConfigBuilder`.
- [ ] In `add_surface_with_config`, merge builder usage with
      `RENDER_ATTACHMENT`.
- [ ] Add `RenderFrame::capture_texture_copy(&mut self, device, width, height)`
      that creates a staging buffer and encodes a copy command.
- [ ] In `GupAppRunner::render_frame`, when `screenshot_requested`, call
      `capture_texture_copy` before `finish()`, then map the buffer and save.
- [ ] Add tests for `SurfaceConfigBuilder` with custom usage.
- [ ] Add a test that creates an offscreen frame with `COPY_SRC` and reads back
      known pixel data.

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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated in story file and INDEX.md
- [ ] Retrospective added to story document
