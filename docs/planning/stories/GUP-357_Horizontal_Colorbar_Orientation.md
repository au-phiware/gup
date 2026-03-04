# GUP-357: Horizontal Colorbar Orientation

## Story Overview

**Initiative**: Chart Builders  
**Status**: 📋 Planned  
**Created**: 2025-07-18

## Context

GUP-296 (Colorbar Axis Renderer) implemented vertical colorbar rendering with
the `ColorbarOrientation::Horizontal` variant defined in the config enum but not
yet implemented in the geometry generation path. Some chart layouts (e.g. wide
heatmaps, charts with limited right margin) benefit from a horizontal colorbar
placed below the plot area.

## User Story

> "As a chart author, I want to place the colour legend horizontally below the
> chart when the layout is wide and the right margin is limited."

## Acceptance Criteria

- [ ] `ColorbarRenderer::generate_geometry` supports
      `ColorbarOrientation::Horizontal`, placing the strip below the chart
      area.
- [ ] Tick marks and labels are placed along the bottom of the horizontal strip
      using `AxisPosition::Bottom`.
- [ ] The heatmap builder (or any chart builder) can switch orientation via
      a `.colorbar_orientation(Horizontal)` method.
- [ ] Unit tests validate horizontal geometry dimensions and label positions.

## Technical Tasks

- [ ] Extend `ColorbarRenderer::generate_gradient_strip` and
      `generate_outline` to handle horizontal layout.
- [ ] Use `AxisPosition::Bottom` for tick and label generation in horizontal
      mode.
- [ ] Add builder method to `HeatmapBuilder` and/or `ColorbarConfig`.
- [ ] Add unit tests for horizontal geometry.

## Dependencies

### Prerequisite Stories

- GUP-296: Colorbar Axis Renderer ✅ — provides the vertical implementation and
  `ColorbarOrientation` enum.

## Testing Strategy

- Unit tests for horizontal gradient strip vertex positions.
- Visual validation with a wide heatmap example.

## Risk Assessment

- **Low**: The vertical implementation provides a clear template.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Tests pass
- [ ] Visual validation with an example
