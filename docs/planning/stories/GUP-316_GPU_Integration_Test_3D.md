# GUP-316: GPU Integration Test for 3D Marks

## Story Overview

**Initiative**: Quality **Status**: ✅ Complete **Created**: 2026-03-04

## Context

GUP-261 delivered Sphere3D, Box3D, and Line3D marks with unit tests for geometry
and bytemuck layout. However, it lacks a GPU-level integration test that
actually renders instances through the full wgpu pipeline and verifies the
output. Such a test would catch shader compilation errors, bind-group
mismatches, and depth buffer issues in CI.

## User Story

> "As a library maintainer, I want GPU integration tests for 3D marks so that
> shader regressions and pipeline mismatches are caught automatically in CI."

## Acceptance Criteria

- [x] A headless integration test renders 1000+ `Sphere3D` instances and asserts
      no wgpu validation layer errors
- [x] The colour attachment is verified as non-zero (something was drawn)
- [x] A headless integration test renders `Box3D` and `Line3D` instances
- [x] Tests run with `--test-threads=1` without segfaults

## Technical Tasks

- [x] Add `tests/three_d_integration.rs` with headless GPU tests
- [x] Create a headless 3D render helper that sets up DepthBuffer, Camera,
      Light, and draws instances
- [x] Assert non-zero pixel output by reading back the colour attachment
- [x] Add a 100K instance performance assertion (< 16ms per frame)

## Dependencies

### Prerequisite Stories

- GUP-261: 3D Visualization Support ✅

## Testing Strategy

- GPU integration tests using `GupContext::headless()`
- Pixel readback to verify non-zero output

## Risk Assessment

- **Low**: Headless GPU testing is well-established in the project.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] Tests pass in CI with `--test-threads=1`
- [x] Story status updated in INDEX.md

## Implementation Summary

**Completed**: 2025-07-18

### What Was Implemented

A comprehensive GPU integration test file (`tests/three_d_integration.rs`)
containing 4 tests that exercise the full wgpu render pipeline for all three 3D
mark types:

1. **`sphere3d_headless_1000_instances`** — Renders 1,000 Sphere3D instances
   with full Phong lighting through the SDF billboard pipeline. Verifies
   non-zero pixel output via OffscreenTarget readback.
2. **`box3d_headless_render`** — Renders 100 Box3D instances (axis-aligned cubes
   with 24 vertices/36 indices per box) with Phong lighting. Verifies pixel
   readback.
3. **`line3d_headless_render`** — Renders 50 Line3D instances (unlit
   camera-facing quads) with camera-only uniform bind group. Verifies pixel
   readback.
4. **`sphere3d_100k_performance`** — Renders 100,000 Sphere3D instances with 3
   warm-up frames and 10 timed frames, asserting < 16ms average frame time.

### Key Files Changed

| File | Change |
|------|--------|
| `tests/three_d_integration.rs` | **New** — 878-line integration test file |
| `docs/planning/stories/GUP-316_GPU_Integration_Test_3D.md` | Updated status |
| `docs/planning/stories/INDEX.md` | Updated status |

### Test Infrastructure

- **Headless context**: Direct `wgpu::Instance` → `Adapter` → `Device` creation
  (no window surface required)
- **Offscreen rendering**: Uses `gup::export::png::OffscreenTarget` for
  render-target texture + `readback_pixels()` for GPU→CPU pixel transfer
- **Depth buffer**: Uses `gup::depth::DepthBuffer` for correct 3D occlusion
- **Non-zero pixel assertion**: Custom `assert_non_zero_pixels()` helper scans
  RGBA data for any channel > 10 (avoiding false positives from near-black
  clear colour)

### Test Counts

- 4 new integration tests (all passing)
- 0 regressions in existing test suite

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### Headless GPU Rendering Without GupContext

- **Challenge**: The story suggested using `GupContext::headless()` but
  integration tests that exercise the raw wgpu pipeline don't need the
  full `GupContext` abstraction — they just need a device and queue.
- **Solution**: Created a minimal `headless_context()` helper that directly
  requests an adapter and device via `wgpu::Instance`. This is faster to
  initialise and avoids coupling tests to the higher-level context API.
- **Pattern**: For low-level GPU pipeline tests, use raw wgpu device/queue.
  Reserve `GupContext::headless()` for tests that exercise the context's
  own APIs (surface management, buffer pools, etc.).

#### Lit vs Unlit Bind Group Layouts

- **Challenge**: Line3D is unlit (no light uniform in its fragment shader),
  but Sphere3D and Box3D both need camera + light uniforms. Using a single
  bind-group layout for all three would cause a validation error on Line3D
  because its shader doesn't declare a light binding.
- **Solution**: Parameterised the `uniform_bgl()` and
  `create_uniform_bind_group()` helpers with a `lit: bool` flag that
  conditionally adds the light binding.
- **Pattern**: When testing multiple shader variants, make bind-group
  layout creation configurable rather than one-size-fits-all.

#### OffscreenTarget for Pixel Readback

- **Challenge**: Needed to verify that the render pass actually drew
  something without visual inspection.
- **Solution**: Leveraged the existing `gup::export::png::OffscreenTarget`
  which handles texture creation with `RENDER_ATTACHMENT | COPY_SRC`,
  staging buffer allocation, row-padding alignment, and BGRA→RGBA
  conversion. This avoided re-implementing readback logic.
- **Pattern**: Reuse the export module's offscreen infrastructure for
  GPU integration tests — it's already battle-tested.

### Architectural Decisions

#### Direct Pipeline Construction vs Selection API

- **Decision**: Tests construct wgpu pipelines directly (shader modules,
  bind groups, vertex buffers, render pass) rather than going through the
  Selection/Mark rendering API.
- **Reasoning**: The goal is to test the WGSL shaders and bind-group
  compatibility at the lowest possible level. If a shader has a
  type mismatch or a bind-group layout is wrong, the test should fail
  at pipeline creation, not be masked by higher-level error handling.
- **Trade-off**: Tests are more verbose (~880 lines) but provide
  precise failure localisation.
- **Future**: Higher-level rendering tests via the Selection API could
  complement these low-level tests but belong in a separate story.

### Development Workflow Insights

- The story was straightforward — all 4 tests passed on first run after
  fixing a minor API mismatch (`panic_on_timeout()` → `unwrap()` for
  `device.poll()` return type in wgpu v26).
- The `mask all-fix` command touched many pre-existing files with
  formatting changes. These were committed separately to keep the
  integration test commit clean.
- Performance assertion (100K instances < 16ms) passed comfortably,
  confirming the 3D pipeline is well within the 60 FPS budget even
  at high instance counts.
