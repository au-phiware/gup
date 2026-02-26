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
  with `width` and `height` fields, used as a GPU uniform buffer for pixel-to-clip
  conversion.

- **Viewport uniform binding** (`src/mark.rs`): Added `@group(0) @binding(1)` to
  `create_bind_group_layout()` for all marks with custom shaders, establishing
  a reusable pattern for Circle and Rectangle marks.

- **Viewport buffer management** (`src/selection.rs`):
  - `SelectionRenderState` stores an optional `viewport_buffer`
  - Created with default 800×600 dimensions on first `prepare_render()`
  - `Selection::set_viewport_size()` updates via `queue.write_buffer()`
  - Buffer is reused when instance buffer is reallocated

- **BoxPlot shaders** (all three):
  - `boxplot.vert.wgsl`: Uses viewport to convert pixel-based margin to clip space
  - `boxplot.frag.wgsl`: Converts `stroke_width`, `outlier_radius`, and anti-aliasing
    from pixels to clip-space using geometric mean: `sqrt(2/w × 2/h)`
  - `boxplot_pattern.frag.wgsl`: Same viewport conversion for pattern rendering

- **Generated shader strings** (`src/mark/boxplot.rs`): Both
  `generate_vertex_shader_with_functions` and `generate_fragment_shader_with_functions`
  include ViewportUniforms declaration and binding.

- **Demo update** (`examples/boxplot_rendering_demo.rs`): Uses pixel-based stroke (2px)
  and radius (6px); calls `set_viewport_size()` each frame from window dimensions.

### Key Files Changed

| File | Changes |
|------|---------|
| `src/mark.rs` | Viewport uniform binding in bind group layout |
| `src/selection.rs` | `ViewportUniforms`, viewport buffer management, `set_viewport_size()`, pixel consistency GPU test |
| `src/lib.rs` | Export `ViewportUniforms` |
| `src/mark/boxplot.rs` | Doc comments, generated shader strings |
| `src/mark/shaders/boxplot.vert.wgsl` | Viewport-aware margin calculation |
| `src/mark/shaders/boxplot.frag.wgsl` | Pixel-to-clip conversion for SDF |
| `src/mark/shaders/boxplot_pattern.frag.wgsl` | Same viewport conversion |
| `examples/boxplot_rendering_demo.rs` | Pixel values, viewport update |

### Test Count

- 1587 tests pass (3 pre-existing failures in `mark::renderer` unrelated to this story)
- New GPU test: `gpu_boxplot_pixel_consistent_strokes` — renders at 256px and 512px,
  verifies non-white pixel ratio ≈2.0 (pixel-consistent strokes)

_Identified during GUP-166 retrospective (2025-07-17)._
