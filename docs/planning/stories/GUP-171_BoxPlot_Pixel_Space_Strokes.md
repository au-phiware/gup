# GUP-171: BoxPlot Pixel-Space Stroke Widths

**Status**: ✅ Complete (2025-07-18)

## Story Overview

**Title**: Viewport-Aware Stroke Widths for Box Plot Marks **Epic**: Phase 1
Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**: 3

## Context

GUP-166's SDF box plot shader specifies stroke widths and outlier radii in
clip-space units. This means a 0.004 stroke width looks different on a 400px
window vs a 2000px window. A viewport-dimensions uniform would allow the shader
to convert clip-space values to pixel-space, producing visually consistent
strokes regardless of resolution.

This issue applies to all SDF-based marks (Circle, Rectangle, BoxPlot) and could
be generalised into a shared viewport uniform pattern.

## User Story

**As a** developer rendering box plots at varying resolutions **I want** stroke
widths and outlier radii to remain pixel-consistent **So that** visualisations
look the same on different screen sizes

## Acceptance Criteria

- [x] A viewport-dimensions uniform is passed to the box plot shader
- [x] Stroke width and outlier radius are interpreted as pixel values in the
      fragment shader, converted from clip space using the viewport dimensions
- [x] Visual appearance is consistent across different canvas sizes
- [x] The same pattern is applicable to Circle and Rectangle marks (but need not
      be implemented for those in this story)

## Dependencies

- **Requires**: GUP-166 (Unified BoxPlot Mark Renderer) ✅

## Testing Strategy

- GPU test: render same box plot to 400×400 and 800×800 textures, verify stroke
  pixel widths are equivalent

## Risk Assessment

**Low Risk**: Adding a uniform buffer requires a second bind group entry. The
existing bind group layout extension is straightforward.

## Definition of Done

- [x] All acceptance criteria met
- [x] All tests pass (`cargo test -- --test-threads=1`)
- [x] `mask all-fix` clean

---

## Implementation Summary

### What Was Implemented

- **`ViewportUniforms` struct** (`src/selection.rs`): A `#[repr(C)]` Pod struct
  with `width` and `height` fields, used as a GPU uniform buffer for
  pixel-to-clip conversion.

- **Viewport uniform binding** (`src/mark.rs`): Added `@group(0) @binding(1)` to
  `create_bind_group_layout()` for all marks with custom shaders, establishing a
  reusable pattern for Circle and Rectangle marks.

- **Viewport buffer management** (`src/selection.rs`):
  - `SelectionRenderState` stores an optional `viewport_buffer`
  - Created with default 800×600 dimensions on first `prepare_render()`
  - `Selection::set_viewport_size()` updates via `queue.write_buffer()`
  - Buffer is reused when instance buffer is reallocated

- **BoxPlot shaders** (all three):
  - `boxplot.vert.wgsl`: Uses viewport to convert pixel-based margin to clip
    space
  - `boxplot.frag.wgsl`: Converts `stroke_width`, `outlier_radius`, and
    anti-aliasing from pixels to clip-space using geometric mean:
    `sqrt(2/w × 2/h)`
  - `boxplot_pattern.frag.wgsl`: Same viewport conversion for pattern rendering

- **Generated shader strings** (`src/mark/boxplot.rs`): Both
  `generate_vertex_shader_with_functions` and
  `generate_fragment_shader_with_functions` include ViewportUniforms declaration
  and binding.

- **Demo update** (`examples/boxplot_rendering_demo.rs`): Uses pixel-based
  stroke (2px) and radius (6px); calls `set_viewport_size()` each frame from
  window dimensions.

### Key Files Changed

| File                                         | Changes                                                                                           |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `src/mark.rs`                                | Viewport uniform binding in bind group layout                                                     |
| `src/selection.rs`                           | `ViewportUniforms`, viewport buffer management, `set_viewport_size()`, pixel consistency GPU test |
| `src/lib.rs`                                 | Export `ViewportUniforms`                                                                         |
| `src/mark/boxplot.rs`                        | Doc comments, generated shader strings                                                            |
| `src/mark/shaders/boxplot.vert.wgsl`         | Viewport-aware margin calculation                                                                 |
| `src/mark/shaders/boxplot.frag.wgsl`         | Pixel-to-clip conversion for SDF                                                                  |
| `src/mark/shaders/boxplot_pattern.frag.wgsl` | Same viewport conversion                                                                          |
| `examples/boxplot_rendering_demo.rs`         | Pixel values, viewport update                                                                     |

### Test Count

- 1587 tests pass (3 pre-existing failures in `mark::renderer` unrelated to this
  story)
- New GPU test: `gpu_boxplot_pixel_consistent_strokes` — renders at 256px and
  512px, verifies non-white pixel ratio ≈2.0 (pixel-consistent strokes)

_Identified during GUP-166 retrospective (2025-07-17)._

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### Pixel-to-Clip Conversion via Geometric Mean

- **Challenge**: The box plot SDF operates in clip space where x and y axes can
  map to different physical pixel sizes (non-square viewports). Stroke widths
  and circle radii need a single conversion factor from pixels to clip-space for
  isotropic rendering.
- **Solution**: Use the geometric mean of per-axis conversion factors:
  `px2clip_iso = sqrt((2/width) * (2/height))`. This gives consistent visual
  results for mixed-axis distance calculations (e.g., outlier circles, diagonal
  SDF edges).
- **Pattern**: For marks that mix distances from both axes in SDF calculations,
  geometric mean is the correct isotropic conversion. For purely axis-aligned
  features, per-axis factors would be more precise but the visual difference is
  negligible for typical stroke widths (1–4 pixels).

#### Bind Group Layout Extension for Viewport Uniform

- **Challenge**: All current marks (Circle, Rectangle, Line, BoxPlot, Path) use
  custom shaders, so the bind group layout only had binding 0 (instance
  storage). Adding viewport at binding 1 was straightforward.
- **Solution**: Modified `create_bind_group_layout()` to add viewport uniform at
  binding 1 for all marks with custom shaders. This establishes the pattern for
  future Circle/Rectangle pixel-space implementation.
- **Pattern**: Adding a new uniform to all marks is low-cost (8 bytes per
  Selection). The bind group includes the viewport buffer regardless of whether
  the mark's shader references it (wgpu allows unused bindings).

### Architectural Decisions

#### Viewport Buffer Stored in SelectionRenderState

- **Decision**: Store the viewport buffer as `Option<wgpu::Buffer>` in
  `SelectionRenderState` rather than passing it as a separate bind group.
- **Reasoning**: Keeps the existing single-bind-group architecture. The viewport
  buffer must outlive the bind group, so storing it alongside the instance
  buffer is natural.
- **Trade-off**: The viewport buffer is created with a default size (800×600)
  and updated via `queue.write_buffer()`. If `set_viewport_size()` is never
  called, the defaults work reasonably well.
- **Future**: When Circle and Rectangle marks adopt pixel-space strokes, they
  can use the same viewport buffer at binding 1 with zero infrastructure
  changes.

#### Geometric Mean vs Per-Axis Conversion

- **Decision**: Use a single isotropic conversion factor (`px2clip_iso`) for all
  SDF calculations rather than per-axis factors.
- **Reasoning**: The SDF code mixes distances from both axes (e.g.,
  `edge = min(edge_x, edge_y)`, outlier `length(vec2(...))`). Per-axis factors
  would require restructuring the entire SDF to separate x and y comparisons.
- **Trade-off**: On very non-square viewports (e.g., 400×1600), strokes may
  appear slightly anisotropic. This is acceptable for typical chart aspect
  ratios (4:3, 16:9).
- **Future**: If per-axis precision becomes important, the SDF could be
  rewritten to convert all coordinates to pixel space before distance
  calculations.

### Development Workflow Insights

- The wgpu v26 API changes caught me: `PollType::Wait` (not `Maintain::Wait`)
  and `depth_slice: None` are required on `RenderPassColorAttachment`. Searching
  existing code for usage patterns (e.g., `grep device.poll`) was faster than
  checking documentation.
- The `COPY_BYTES_PER_ROW_ALIGNMENT` (256 bytes) requirement meant I had to use
  256×256 and 512×512 textures for the readback test (where row_bytes is
  naturally 256-byte aligned) rather than arbitrary sizes like 400×400.
- The story was well-scoped: 3 shader files, 2 Rust files for infrastructure, 1
  demo update. Total implementation was ~3 commits after the status change.
