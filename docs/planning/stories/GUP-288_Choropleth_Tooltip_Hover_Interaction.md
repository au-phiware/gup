# GUP-288: Choropleth Tooltip and Hover Interaction

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2025-07-15

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

- [ ] Hovering over a choropleth region triggers a tooltip displaying the region
      name (from GeoJSON properties) and the data value.
- [ ] The hovered region is visually highlighted (configurable: brighten,
      outline, or opacity change).
- [ ] `.tooltip(true/false)` enables or disables the tooltip.
- [ ] `.tooltip_format(closure)` allows custom tooltip content.
- [ ] The interaction uses the GPU hit-testing system (GUP-012/GUP-014) to map
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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
