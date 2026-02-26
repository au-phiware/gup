# GUP-205: SDF Text Rendering Performance Tuning

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs **Theme**: Text Rendering
**Priority**: Low **Story Points**: 3 **Status**: 🚧 In Progress

## Overview

Tune the SDF (Signed Distance Field) shader parameters for optimal
quality/performance balance at different text sizes used by axis labels. The
existing TextRenderer has excellent batching (single draw call) but the SDF
rendering parameters haven't been profiled against the range of font sizes
typically used for axis tick labels, axis titles, and chart annotations.

## Context

GUP-094 identified that while the TextRenderer batching is efficient, the SDF
shader parameters (smoothing threshold, edge softness, sub-pixel sampling)
haven't been tuned for the specific text sizes used by the axis system. Small
labels may appear blurry or aliased without proper SDF parameter adjustment.

## User Story

> "As a chart developer, I want axis labels to render crisply at all sizes so
> that my visualizations look professional regardless of chart dimensions."

## Acceptance Criteria

- [ ] SDF parameters profiled at 8px, 12px, 16px, 24px, and 32px text sizes
- [ ] Optimal smoothing values documented for each size range
- [ ] Auto-tuning logic selects appropriate SDF params based on text size
- [ ] No visual regression for existing text rendering
- [ ] Performance impact of tuning measured (should be negligible)

## Dependencies

- **GUP-099**: GPU Text Rendering Pipeline ✅
- **GUP-108**: Correct SDF Font Atlas Generation ✅
- **GUP-094**: Axis Performance Optimization ✅

## Testing Strategy

- Visual comparison: text at various sizes with tuned vs default parameters
- Performance benchmark: measure rendering time with auto-tuning enabled
- Regression: existing text tests continue to pass

## Definition of Done

- [ ] SDF parameters tuned and documented
- [ ] Auto-tuning logic implemented
- [ ] Visual quality maintained or improved
- [ ] Performance unchanged or improved
