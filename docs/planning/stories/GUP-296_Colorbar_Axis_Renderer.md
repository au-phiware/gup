# GUP-296: Colorbar Axis Renderer

## Story Overview

**Initiative**: Chart Builders  
**Status**: 🚧 In Progress  
**Created**: 2026-03-03

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

- [ ] A `ColorbarRenderer` struct renders a thin vertical or horizontal gradient
      strip adjacent to the plot area.
- [ ] The gradient is filled using the same `ColorScale` as the chart cells,
      ensuring visual consistency.
- [ ] Tick marks and numeric labels are placed along the colorbar using the
      `TickGenerator` from GUP-093.
- [ ] The colorbar inherits the fill domain from the heatmap (or from
      `.fill_domain()` overrides).
- [ ] Rendering is suppressed when `.colorbar(false)` is set on the builder.
- [ ] The colorbar is composable: it can be added to any `ComposedChart` that
      has a `ColorScale`.

## Technical Tasks

- [ ] Create `src/chart_builder/colorbar.rs` with `ColorbarRenderer`.
- [ ] Implement GPU-instanced gradient strip rendering using Rectangle marks.
- [ ] Integrate with `TickGenerator` for tick marks and labels.
- [ ] Wire into `ComposedChart` rendering pipeline when `config.color_scale` and
      `show_colorbar` are both set.
- [ ] Add unit tests and visual validation.

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

- [ ] All acceptance criteria met
- [ ] Tests pass
- [ ] Visual validation with heatmap example
- [ ] Documentation updated
