# GUP-166: Unified BoxPlot Mark Renderer

**Status**: 📋 Planned

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

- [ ] `BoxPlotMark` implements the `Mark` trait (GUP-009)
- [ ] Shader renders box (IQR rect), median line, whiskers, and outlier circles
      in one pipeline (or minimal coordinated draw calls)
- [ ] `BoxPlotAttributes` drives all visual properties (colours, stroke width,
      outlier radius) without additional per-call configuration

### AC2: Selection API Integration

- [ ] `Selection::bind(BoxPlotMark)` drives rendering via GUP-165 infrastructure
- [ ] Multiple box plots per selection (one per data group) rendered in a single
      `Selection::render()` call
- [ ] Vertical and horizontal orientations supported

### AC3: Updated boxplot_rendering_demo.rs

- [ ] Demo replaced from manual primitive decomposition to `BoxPlotMark`-based
      rendering using the Selection API
- [ ] Four distributions render correctly and visibly
- [ ] Demo compiles cleanly and produces no GPU validation errors

### AC4: Performance Baseline

- [ ] 100 box plots render at ≥60 FPS on the development GPU
- [ ] Benchmark result documented in the retrospective

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

- **Requires**: GUP-149 (Box Plot Statistical Foundation) 🚧
- **Requires**: GUP-165 (Selection API Render Integration) 📋
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

- [ ] AC1–AC4 acceptance criteria checked off
- [ ] GUP-149 `boxplot_rendering_demo.rs` replaced and GUP-149 closed ✅
- [ ] All tests pass (`mask test`)
- [ ] No new Clippy warnings (`mask all-fix` clean)
- [ ] Retrospective written with follow-up stories identified

---

_Identified during GUP-149 retrospective (2025-01-11). Created 2026-02-24._
