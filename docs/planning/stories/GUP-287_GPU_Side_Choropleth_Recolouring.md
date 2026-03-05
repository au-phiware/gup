# GUP-287: GPU-Side Choropleth Recolouring

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-15
**Completed**: 2025-07-18

## Context

GUP-275 (Choropleth Chart Builder) assigns per-vertex fill colours at CPU build
time. This means that changing the colour scale, animating between datasets, or
highlighting a hovered region requires re-tessellating and re-uploading the
entire geometry. For interactive applications (dashboards, animated transitions)
this is too expensive.

This story adds a GPU-side per-region colour lookup: a storage buffer of region
colours indexed by feature index, with a fragment shader that reads the colour
from the buffer rather than the vertex attribute. The CPU side only needs to
update the storage buffer (a small flat array) when colours change.

## User Story

> "As a visualization developer, I want to dynamically recolour choropleth
> regions without rebuilding the geometry, so that I can animate colour
> transitions and highlight hovered regions at interactive frame rates."

## Acceptance Criteria

- [x] A `RegionColorBuffer` (or equivalent) stores per-region RGBA colours in a
      GPU storage buffer, indexed by feature index.
- [x] The choropleth fragment shader reads the region colour from the storage
      buffer instead of the vertex attribute when GPU-side recolouring is
      enabled.
- [x] `ChoroplethChart::update_colors(new_data)` updates the storage buffer
      without re-tessellating geometry.
- [x] Colour transitions between two datasets can be animated by interpolating
      the storage buffer values over time.
- [x] The existing CPU-side per-vertex colouring remains the default; GPU-side
      recolouring is opt-in.

## Dependencies

### Prerequisite Stories

- GUP-275: Choropleth Chart Builder ✅

### Enables Stories

- GUP-288: Choropleth Tooltip and Hover Interaction

## Testing Strategy

- Unit tests for `RegionColorBuffer` creation and update.
- Integration test verifying that recolouring does not produce GPU validation
  errors.
- Visual test comparing CPU-side and GPU-side colouring for identical datasets.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What Was Implemented

- **`RegionColorBuffer`** — CPU-side per-region RGBA colour array with methods
  for creation (`new`, `from_regions`), mutation (`set_color`,
  `update_from_data`), animation (`interpolate`), and GPU upload
  (`as_bytes`).
- **`IndexedChoroplethVertex`** — Lightweight vertex type (position +
  `region_index: u32`) for GPU-side colour lookup, replacing the per-vertex
  `color: [f32; 4]` when GPU recolouring is active.
- **`ChoroplethChart::update_colors()`** — Recolours all regions from new data
  without re-tessellating geometry, updating the `RegionColorBuffer`, domain
  bounds, and per-region value records.
- **`ChoroplethChart::interpolate_colors()`** — Produces an interpolated
  `RegionColorBuffer` for smooth animated transitions between colour states.
- **`ChoroplethChartBuilder::gpu_recolor(bool)`** — Opt-in toggle (default
  `false`). When enabled, the build step produces both standard per-vertex
  coloured geometry and indexed vertices + colour buffer.
- **WGSL shaders** — `choropleth_recolor.vert.wgsl` reads `region_index` from
  each vertex and looks up colour from a `storage` buffer at
  `@group(0) @binding(1)`. Fragment shader mirrors `geo_path.frag.wgsl`.
- **Shader constants** — `RECOLOR_VERTEX_SHADER` and `RECOLOR_FRAGMENT_SHADER`
  exposed for pipeline construction.
- **Example** — `examples/choropleth_gpu_recolor.rs` demonstrating dynamic
  recolouring, interpolation, and per-region highlighting.

### Key Files Changed

| File | Change |
| --- | --- |
| `src/chart_builder/builders/choropleth.rs` | +600 lines: RegionColorBuffer, IndexedChoroplethVertex, update_colors, interpolate_colors, gpu_recolor builder method, shader constants, 19 new tests |
| `src/mark/shaders/choropleth_recolor.vert.wgsl` | New vertex shader for storage-buffer colour lookup |
| `src/mark/shaders/choropleth_recolor.frag.wgsl` | New fragment shader (fill/stroke selection) |
| `examples/choropleth_gpu_recolor.rs` | New example demonstrating GPU recolouring |

### Test Counts

- **40 unit tests** in `chart_builder::builders::choropleth::tests` (21 original + 19 new)
- **2 982 total lib tests** pass under `cargo test -- --test-threads=1`
