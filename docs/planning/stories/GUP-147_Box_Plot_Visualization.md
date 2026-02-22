# GUP-147: Box Plot Visualization

**Status**: 💡 New

## Story Overview

**Title**: Interactive Box Plot Visualization with GPU Statistics  
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping  
**Priority**: Medium  
**Story Points**: 5

## Context

GUP-139 provides the statistical foundation (min, max, quartiles) needed for box
plots. Box plots are essential statistical visualizations showing distribution
summary through five-number summary (min, Q1, median, Q3, max) plus outliers.

## User Story

**As a** data visualization developer  
**I want** to create interactive box plots using GPU statistics  
**So that** I can efficiently visualize data distributions and identify outliers

## Acceptance Criteria

### AC1: Box Plot Mark Type

- [ ] New `BoxPlot` mark type
- [ ] Render box (Q1-Q3 interquartile range)
- [ ] Median line
- [ ] Whiskers (min-max or 1.5×IQR)
- [ ] Outlier points beyond whiskers

### AC2: Statistical Integration

- [ ] Use `Percentile` for quartile calculation
- [ ] Use `MinMax` for range
- [ ] Efficient GPU-based outlier detection
- [ ] Support grouped box plots (by category)

### AC3: Interaction and Styling

- [ ] Hover to show exact values
- [ ] Configurable colors and styles
- [ ] Horizontal and vertical orientations
- [ ] Notched box plots (confidence interval)

## Technical Requirements

- Integrate with `Percentile` from GUP-139
- Use GPU compute for outlier identification
- Render using existing mark system
- Support Observable Plot-style API

## Dependencies

- **Requires**: GUP-139 (Statistical Shader Functions) - ✅ Complete
- **May require**: GUP-010 (Basic Mark Implementations) patterns
- **Enables**: Statistical distribution visualization

## Testing Strategy

- Test with known distributions (verify quartiles)
- Visual regression tests for rendering
- Test grouped box plots (multiple categories)
- Performance test with 100+ box plots
- Interaction testing (hover, selection)

## Success Metrics

- Correct quartile and outlier calculation
- Render 100 box plots at 60 FPS
- Observable Plot-style ergonomic API
- Interactive hover shows all statistics

## Risk Assessment

**Low Risk**: Building on proven statistical and rendering foundations.

**Mitigation**: Reuse patterns from existing mark types and statistical
functions.

## Definition of Done

- [ ] BoxPlot mark type implemented
- [ ] Integration with GUP-139 statistics
- [ ] Observable Plot-style builder API
- [ ] Interactive hover and styling
- [ ] Tests verify statistical correctness
- [ ] Visual regression tests
- [ ] Example demonstrating usage
- [ ] Documentation with statistical explanations
- [ ] All tests pass

---

_Identified during GUP-139 implementation as statistical visualization use
case._
