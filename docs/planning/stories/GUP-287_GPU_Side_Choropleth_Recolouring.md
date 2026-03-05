# GUP-287: GPU-Side Choropleth Recolouring

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**: 2025-07-15

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

- [ ] A `RegionColorBuffer` (or equivalent) stores per-region RGBA colours in a
      GPU storage buffer, indexed by feature index.
- [ ] The choropleth fragment shader reads the region colour from the storage
      buffer instead of the vertex attribute when GPU-side recolouring is
      enabled.
- [ ] `ChoroplethChart::update_colors(new_data)` updates the storage buffer
      without re-tessellating geometry.
- [ ] Colour transitions between two datasets can be animated by interpolating
      the storage buffer values over time.
- [ ] The existing CPU-side per-vertex colouring remains the default; GPU-side
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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
