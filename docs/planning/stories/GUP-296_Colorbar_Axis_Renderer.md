# GUP-296: Colorbar Axis Renderer

## Story Overview

**Initiative**: Chart Builders  
**Status**: ✅ Complete  
**Created**: 2026-03-03  
**Completed**: 2025-07-18

## Context

GUP-248 (Heatmap Chart Builder) introduced the `.colorbar(true/false)` toggle
and stores the `ColorScale` configuration, but the actual visual rendering of
the colorbar — a thin gradient-filled rectangle with tick marks and numeric
labels drawn adjacent to the plot area — was not implemented. This story adds
that renderer as a reusable component that any chart with a colour-scale
dimension can use.

The colorbar should integrate with the existing axis system (GUP-093) for tick
generation and label formatting, and use the `ColorScale` GPU shader function
(GUP-255) for the gradient fill so that the legend exactly matches the cell
colours.

## User Story

> "As a data analyst viewing a heatmap, I want a colour legend next to the plot
> so I can read off the numeric value corresponding to each colour without
> guessing."

## Acceptance Criteria

- [x] A `ColorbarRenderer` struct renders a thin vertical or horizontal gradient
      strip adjacent to the plot area.
- [x] The gradient is filled using the same `ColorScale` as the chart cells,
      ensuring visual consistency.
- [x] Tick marks and numeric labels are placed along the colorbar using the
      `TickGenerator` from GUP-093.
- [x] The colorbar inherits the fill domain from the heatmap (or from
      `.fill_domain()` overrides).
- [x] Rendering is suppressed when `.colorbar(false)` is set on the builder.
- [x] The colorbar is composable: it can be added to any `ComposedChart` that
      has a `ColorScale`.

## Technical Tasks

- [x] Create `src/chart_builder/colorbar.rs` with `ColorbarRenderer`.
- [x] Implement GPU-instanced gradient strip rendering using Rectangle marks.
- [x] Integrate with `TickGenerator` for tick marks and labels.
- [x] Wire into `ComposedChart` rendering pipeline when `config.color_scale` and
      `show_colorbar` are both set.
- [x] Add unit tests and visual validation.

## Dependencies

### Prerequisite Stories

- GUP-248: Heatmap Chart Builder ✅ — provides the `show_colorbar` flag and
  `fill_domain` configuration.
- GUP-093: Scale-Axis Integration System ✅ — provides tick generation and label
  formatting.
- GUP-255: ColorScale GPU Shader Function ✅ — provides the gradient shader.

## Testing Strategy

- Unit tests for tick placement and domain inheritance.
- Visual validation with the heatmap example.

## Risk Assessment

- **Low**: The colorbar is a self-contained renderer with well-defined inputs.

## Definition of Done

- [x] All acceptance criteria met
- [x] Tests pass
- [x] Visual validation with heatmap example
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

A reusable `ColorbarRenderer` component that generates a gradient-filled strip
with tick marks and numeric labels, integrated into the `ComposedChart`
rendering pipeline.

### Key Files Changed

| File | Change |
| --- | --- |
| `src/chart_builder/colorbar.rs` | **New** — `ColorbarRenderer`, `ColorbarConfig`, `ColorbarGeometry`, `ColorbarOrientation` |
| `src/chart_builder.rs` | Added `show_colorbar` to `ChartConfig`, `GradientStripPipeline` (TriangleList), colorbar integration in `ComposedChart` (`prepare_draw_commands`, `prepare_tick_pipeline`, `queue_chart_text`, `draw_colorbar_gradient`) |
| `src/chart_builder/builders/heatmap/mod.rs` | Propagate `show_colorbar` from `HeatmapBuilder` to `ChartConfig` during `build_with_data` |
| `examples/heatmap_chart.rs` | Added `.colorbar(true)` and colorbar status output |

### Architecture

- `ColorbarRenderer` is a standalone component that takes a `ColorScale` and
  produces `ColorbarGeometry` (gradient triangles, tick instances, outline
  lines, labels).
- CPU-side gradient sampling via linear interpolation over `ColorGradientStorage`
  stops, matching the GPU shader's behaviour.
- `GradientStripPipeline` uses TriangleList topology with the same `basic.wgsl`
  shader as axis lines, but draws filled quads instead of line segments.
- Colorbar outline and ticks are merged into existing axis-line and tick buffers
  for efficient single-pass rendering.
- Colorbar labels are queued via the standard `TextRenderer` path in
  `queue_chart_text()`.

### Test Counts

- 11 unit tests in `colorbar::tests` (colour sampling, geometry generation,
  configuration, multiple palettes)
- 10 integration tests in `tests_colorbar` (ComposedChart integration, domain
  inheritance, composability, suppression)
- 2 heatmap propagation tests (show_colorbar true/false through build pipeline)
- **Total: 23 new tests, all passing**
