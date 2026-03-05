# GUP-315: Graph Node Label Rendering

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-20

## Context

GUP-311 renders graph nodes as coloured circles but lacks text labels. Adding
node labels would make graph visualisations far more informative, especially for
social networks, dependency graphs, and knowledge graphs. The project already
has an SDF text rendering pipeline (`TextRenderer`, `FontAtlas`) that could be
integrated.

## User Story

> "As a visualization user, I want to see labels on graph nodes so I can
> identify what each node represents."

## Acceptance Criteria

- [ ] Nodes display text labels positioned at or near the node centre
- [ ] Labels are readable at the default zoom level
- [ ] Labels scale appropriately with zoom (via viewport transform)
- [ ] Label rendering does not degrade FPS below 30 for a 200-node graph
- [ ] Labels can be toggled on/off with a keyboard shortcut

## Technical Tasks

1. Integrate `TextRenderer` into the interactive graph example
2. Create `FontAtlas` during initialisation
3. Queue one text label per node each frame before the render pass
4. Apply viewport transform to text positions
5. Add L key to toggle labels

## Dependencies

### Prerequisite Stories

- GUP-311: Interactive Force-Directed Graph Rendering ✅

## Testing Strategy

- Visual validation: labels visible and correctly positioned
- Performance: measure FPS with labels on vs off
- Toggle: verify L key hides/shows labels

## Risk Assessment

- **Medium**: SDF text rendering may have performance implications for hundreds
  of labels. May need batching or culling for off-screen nodes.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint clean
- [ ] Retrospective added
