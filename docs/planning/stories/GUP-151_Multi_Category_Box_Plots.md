# GUP-151: Multi-Category Box Plots

**Status**: 📋 Planned

## Story Overview

**Title**: Grouped Box Plots for Category Comparison  
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping  
**Priority**: Low  
**Story Points**: 3

## Context

Box plots are often used to compare distributions across multiple categories
(e.g., sales by region, test scores by grade level). Supporting grouped box
plots requires handling multi-category data and automatic positioning.

## User Story

**As a** data analyst  
**I want** to display multiple box plots side-by-side grouped by category  
**So that** I can compare distributions across different groups

## Acceptance Criteria

### AC1: Category Grouping

- [ ] Support categorical data grouping
- [ ] Automatic positioning of box plots within groups
- [ ] Configurable spacing between categories
- [ ] Support for nested grouping (categories within categories)

### AC2: Visual Differentiation

- [ ] Color-coding by category
- [ ] Category labels on axis
- [ ] Optional legend for categories
- [ ] Consistent visual hierarchy

### AC3: Data Handling

- [ ] Efficient computation for multiple groups
- [ ] Handle varying sample sizes per category
- [ ] Support for category ordering (alphabetical, by value, custom)

## Technical Requirements

- Extend BoxPlotAttributes to support categorical metadata
- Implement automatic layout algorithm for grouped plots
- Integrate with axis system for category labels
- Support Observable Plot's `fx` and `fy` faceting patterns

## Dependencies

- **Requires**: GUP-147 (Box Plot Visualization) - ✅ Complete
- **Requires**: GUP-149 (Box Plot GPU Rendering) - 📋 Planned

## Testing Strategy

- Test with datasets of varying category counts (2-20 categories)
- Test with unequal sample sizes
- Visual regression tests for layout
- Test category ordering options

## Success Metrics

- Clean visual separation of categories
- Automatic layout works for 2-20 categories
- Performance: 100 box plots (10 categories × 10 groups) at 60 FPS
- Category labels render correctly

## Risk Assessment

**Low Risk**: Building on proven box plot foundation.

**Mitigation**: Start with simple side-by-side layout, iterate to complex.

## Definition of Done

- [ ] Grouped box plots implemented
- [ ] Category labeling integrated
- [ ] Tests cover multiple grouping scenarios
- [ ] Example demonstrating category comparison
- [ ] All tests pass

---

_Identified during GUP-147 implementation._
