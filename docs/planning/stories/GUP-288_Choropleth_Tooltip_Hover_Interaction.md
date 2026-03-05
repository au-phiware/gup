# GUP-288: Choropleth Tooltip and Hover Interaction

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-15

## Context

GUP-275 (Choropleth Chart Builder) produces tessellated per-region geometry but
does not wire into the interaction system (GUP-012/GUP-014). Users expect to
hover over a country on a choropleth map and see a tooltip showing the region
name, data value, and rank. They also expect visual feedback: the hovered region
should highlight (e.g., brighten or outline).

This story connects the choropleth chart to the existing GPU hit-testing and
interaction infrastructure so that pointer events are mapped to region IDs and
the builder can configure tooltip content and hover styling.

## User Story

> "As a visualization developer, I want hovering over a choropleth region to
> show a tooltip with the region name and value, and to visually highlight the
> hovered region, so that users can explore the data interactively."

## Acceptance Criteria

- [x] Hovering over a choropleth region triggers a tooltip displaying the region
      name (from GeoJSON properties) and the data value.
- [x] The hovered region is visually highlighted (configurable: brighten,
      outline, or opacity change).
- [x] `.tooltip(true/false)` enables or disables the tooltip.
- [x] `.tooltip_format(closure)` allows custom tooltip content.
- [x] The interaction uses the GPU hit-testing system (GUP-012/GUP-014) to map
      pointer coordinates to region indices.

## Dependencies

### Prerequisite Stories

- GUP-275: Choropleth Chart Builder ✅
- GUP-012: GPU Interaction System ✅
- GUP-014: Interaction Performance ✅

## Testing Strategy

- Unit tests for region hit-testing (point-in-polygon for projected
  coordinates).
- Integration test verifying hover events produce correct region IDs.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

### What Was Implemented

- **`HoverHighlight` enum** — Three highlight styles: `Brighten(f32)`,
  `Dim(f32)`, and `None` for controlling visual feedback on the hovered region.
- **Builder methods** — `.tooltip(bool)`, `.tooltip_format(closure)`,
  `.highlight_style(HoverHighlight)` on `ChoroplethChartBuilder`.
- **CPU-side hit-testing** — `ChoroplethChart::region_at_point(x, y)` using
  ray-casting point-in-polygon on projected polygon exterior rings stored during
  build.
- **Tooltip content** — `ChoroplethChart::tooltip_content(region_index)` with
  default format (`"<name>: <value>"`) and custom formatter support.
- **Hover colour computation** — `ChoroplethChart::highlighted_color(region_index, is_hovered)`
  applies the configured highlight style.
- **Region polygon storage** — `region_polygons: Vec<Vec<Vec<[f32; 2]>>>`
  captured during tessellation for efficient hit-testing.
- **Crate-level export** — `HoverHighlight` added to `pub use` in `lib.rs`.

### Key Files Changed

| File | Change |
|------|--------|
| `src/chart_builder/builders/choropleth.rs` | All tooltip/hover/hit-test types, methods, and 18 new tests |
| `src/lib.rs` | Export `HoverHighlight` from crate root |

### Test Counts

- 58 choropleth module tests (40 existing + 18 new)
- 3000 total lib tests pass
