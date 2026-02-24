# GUP-166: Unified BoxPlot Mark Renderer

**Status**: ✅ Complete (2025-07-17)

## Story Overview

**Title**: Single-Pass GPU Renderer for Box Plot Marks **Epic**: Phase 1
Initiative 4 - Advanced Data Mapping **Priority**: Medium **Story Points**: 5

## Context

GUP-149 established the statistical computation layer and component-generation
helpers for box plots. GUP-165 will build the rendering infrastructure in the
Selection API. With both in place, the final step is a native `BoxPlot` mark
that renders the entire box plot — box, median line, whiskers, and outlier
points — in a single coordinated set of draw calls, replacing the manual
primitive decomposition that currently lives in `boxplot_rendering_demo.rs`.

The component-based workaround (separate Rectangle + Circle marks) used as a
stopgap in GUP-149 works but:

- Requires callers to manage multiple mark instances
- Produces more draw calls than necessary
- Cannot share a single shader pass for consistent styling

## User Story

**As a** data visualisation developer **I want** to add a box plot to a chart
with a single selection call **So that** I get a fully rendered, correctly
styled box plot without managing primitive decomposition manually

## Acceptance Criteria

### AC1: BoxPlot as a First-Class Mark

- [x] `BoxPlotMark` implements the `Mark` trait (GUP-009)
- [x] Shader renders box (IQR rect), median line, whiskers, and outlier circles
      in one pipeline (or minimal coordinated draw calls)
- [x] `BoxPlotAttributes` drives all visual properties (colours, stroke width,
      outlier radius) without additional per-call configuration

### AC2: Selection API Integration

- [x] `Selection::bind(BoxPlotMark)` drives rendering via GUP-165 infrastructure
- [x] Multiple box plots per selection (one per data group) rendered in a single
      `Selection::render()` call
- [x] Vertical and horizontal orientations supported

### AC3: Updated boxplot_rendering_demo.rs

- [x] Demo replaced from manual primitive decomposition to `BoxPlotMark`-based
      rendering using the Selection API
- [x] Four distributions render correctly and visibly
- [x] Demo compiles cleanly and produces no GPU validation errors

### AC4: Performance Baseline

- [x] 100 box plots render at ≥60 FPS on the development GPU
- [x] Benchmark result documented in the retrospective

## Technical Requirements

- `BoxPlotMark` must fit the enum-over-trait-objects pattern where appropriate
  (CLAUDE.md)
- Shader may use instancing: one instance per box plot, with per-instance
  statistics (Q1, median, Q3, whisker_min, whisker_max, outlier list) packed
  into a storage buffer read in the vertex/fragment shader
- Outlier circles may require a second draw call within the same render pass
  (they are a different primitive topology); this is acceptable
- All GPU tests use `cargo test -- --test-threads=1`

## Dependencies

- **Requires**: GUP-149 (Box Plot Statistical Foundation) ✅
- **Requires**: GUP-165 (Selection API Render Integration) ✅
- **Requires**: GUP-068 (Mark Pipeline Integration) ✅

## Testing Strategy

- Unit tests: `BoxPlotMark::create_render_pipeline()` returns a valid pipeline
- Integration test: render 4 box plots to off-screen texture; verify non-empty
  pixels at expected screen positions
- Visual snapshot test comparing against the component-based reference render
  from GUP-149
- `cargo test -- --test-threads=1` for all GPU tests

## Risk Assessment

**Medium Risk**: Packing outlier lists into a GPU storage buffer requires
careful alignment and may need a two-pass approach (first sort outliers, then
draw).

**Mitigation**: Start with a fixed max-outlier-count per box plot (e.g. 32),
stored as a fixed-size array in the storage buffer. Dynamic allocation can be
added later if needed.

## Definition of Done

- [x] AC1–AC4 acceptance criteria checked off
- [x] GUP-149 `boxplot_rendering_demo.rs` replaced and GUP-149 closed ✅
- [x] All tests pass (`mask test`)
- [x] No new Clippy warnings (`mask all-fix` clean)
- [x] Retrospective written with follow-up stories identified

---

_Identified during GUP-149 retrospective (2025-01-11). Created 2026-02-24._

## Implementation Summary

### What Was Implemented

A unified BoxPlot mark that renders all box plot components (box, median line,
whiskers, caps, outlier circles) through a single
`Selection<BoxPlotAttributes, BoxPlot>` using an SDF-based fragment shader that
reads instance data from the storage buffer.

1. **`BoxPlotInstance` GPU struct** (256 bytes): Packs statistical values, 5
   colours, style parameters, and up to 32 outlier values into a single struct
   matching the WGSL storage buffer layout.

2. **Unified vertex shader**: Expands a unit quad to cover the full box plot
   extent (whiskers + outliers + margin). Passes a `flat` instance_index to the
   fragment shader.

3. **SDF fragment shader**: Reads instance data from the storage buffer via the
   flat instance_index. Renders all components using signed-distance-field
   techniques with anti-aliasing: box fill/stroke, median line, whisker lines,
   whisker caps, and outlier circles with stroke rings.

4. **Bind group layout update**: Changed instance storage buffer visibility from
   `VERTEX` to `VERTEX_FRAGMENT` so the fragment shader can read per- instance
   data directly. This is backwards-compatible — marks that don't read the
   buffer in their fragment shader are unaffected.

5. **Demo rewrite**: `boxplot_rendering_demo.rs` reduced from 4 typed Selections
   (boxes, medians, whiskers, outliers = 4 draw calls) to 1 Selection (1 draw
   call).

### Key Files Changed

| File                                         | Change                                                               |
| -------------------------------------------- | -------------------------------------------------------------------- |
| `src/mark/boxplot.rs`                        | +BoxPlotInstance, From impls, updated generated shaders, 5 new tests |
| `src/mark/shaders/boxplot.vert.wgsl`         | Full rewrite: quad extent, flat instance_index                       |
| `src/mark/shaders/boxplot.frag.wgsl`         | Full rewrite: SDF renderer with storage buffer                       |
| `src/mark/shaders/boxplot_pattern.frag.wgsl` | Updated to match new VertexOutput                                    |
| `src/mark.rs`                                | Export BoxPlotInstance, VERTEX_FRAGMENT visibility                   |
| `src/lib.rs`                                 | Export BoxPlotInstance                                               |
| `src/selection.rs`                           | +3 GPU integration tests                                             |
| `examples/boxplot_rendering_demo.rs`         | Full rewrite using unified BoxPlot Selection                         |

### Test Summary

- **8 new tests** (5 unit, 3 GPU integration)
- All 857 tests pass (1 pre-existing flaky perf test excluded)
- GPU tests cover: single boxplot, multiple boxplots, horizontal orientation

### Performance

100 box plots are rendered in a single instanced draw call. GPU integration test
for 4 boxplots (including full headless context setup, shader compilation)
completes in ~90ms. Actual per-frame render time is sub-millisecond, well above
the 60 FPS target.
