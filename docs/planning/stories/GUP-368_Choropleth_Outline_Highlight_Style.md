# GUP-368: Choropleth Outline Highlight Style

## Story Overview

**Initiative**: Chart Builders **Status**: 💡 New **Created**: 2025-07-22

## Context

GUP-288 added hover highlighting for choropleth regions with `Brighten` and
`Dim` styles. Users also expect an **outline** style that draws a thicker border
around the hovered region — a common pattern in geographic visualisations.
Implementing outline highlighting requires generating additional stroke geometry
at render time or modifying the existing stroke colour/width for the hovered
region.

## User Story

> "As a visualization developer, I want to highlight the hovered choropleth
> region with a visible outline, so that users can clearly see which region
> they are interacting with without altering the fill colour."

## Acceptance Criteria

- [ ] `HoverHighlight::Outline { color, width }` variant added.
- [ ] When a region is hovered, the outline style draws a thicker border around
      it using the configured colour and width.
- [ ] Outline style works with both CPU-tessellated and GPU-recoloured charts.
- [ ] Existing `Brighten` and `Dim` styles are unaffected.

## Technical Tasks

1. Add `Outline { color: [f32; 4], width: f32 }` variant to `HoverHighlight`.
2. Implement stroke generation or modification for the hovered region at render
   time.
3. Add tests for outline highlight.

## Dependencies

### Prerequisite Stories

- GUP-288: Choropleth Tooltip and Hover Interaction ✅

## Testing Strategy

- Unit tests for outline colour and width configuration.
- Visual test verifying outline renders correctly.

## Risk Assessment

- **Medium**: Outline rendering requires either per-region stroke width control
  or additional geometry generation, which may interact with the GPU render
  pipeline (GUP-366).

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
