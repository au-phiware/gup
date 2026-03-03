# GUP-313: Interactive Treemap Drill-Down

## Story Overview

**Initiative**: Advanced Scale **Status**: 📋 Planned **Created**: 2025-07-18

## Context

GUP-260 provides treemap layout with `TreemapCell::node_index` mapping back to
the input hierarchy. Combined with the interaction system (GUP-012), this
enables click-to-zoom navigation where clicking a treemap cell re-roots the
layout at that node, providing hierarchical drill-down exploration.

## User Story

> "As an end user exploring a large hierarchy, I want to click on a treemap cell
> to zoom into that subtree and click a breadcrumb or back button to return to
> the parent view."

## Acceptance Criteria

- [ ] Clicking a treemap cell re-roots the layout at the clicked node's subtree.
- [ ] A breadcrumb trail shows the navigation path from the original root.
- [ ] Clicking the breadcrumb navigates back to that ancestor level.
- [ ] Smooth animated transition between zoom levels.
- [ ] Works with all four treemap algorithm variants.

## Dependencies

### Prerequisite Stories

- GUP-260: GPU Treemap Layout ✅
- GUP-012: GPU Interaction System ✅
- GUP-067: Rectangle and Line Mark Implementations ✅

## Testing Strategy

- Unit tests for subtree extraction and re-rooting logic.
- Integration test verifying click → re-layout → correct subtree displayed.

## Risk Assessment

- **Low**: Subtree extraction from a flat tree is straightforward given the
  contiguous child range representation. Animation between layouts requires
  interpolating cell positions, which the transition system (GUP-016) can
  handle.

## Definition of Done

- [ ] All Acceptance Criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] Retrospective added
