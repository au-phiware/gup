# GUP-236: Sort-Aware Visual Demo

**Story ID**: GUP-236 **Title**: Sort-Aware Visual Demo **Status**: 🚧 In
Progress **Priority**: Low **Effort**: — **Created**: 2025-07-20
**Dependencies**: GUP-184 (GPU Radix Sort for Z-Order)

## Overview

Create an example application demonstrating transparent overlapping marks
rendered with and without Z-order sorting, showing the visual difference.

## Context

GUP-184 implements GPU radix sort for Z-order instance sorting but has no visual
demonstration. A side-by-side example would help developers understand when
sorting is needed and verify correctness visually.

## User Story

As a developer evaluating Gup's rendering capabilities, I want to see a visual
demo of Z-order sorting so I can understand its effect on transparent mark
rendering.

## Acceptance Criteria

- [ ] Example renders two views: sorted and unsorted transparent marks
- [ ] Overlapping transparent circles show visible rendering artifacts without
      sort
- [ ] Sorted view shows correct back-to-front compositing
- [ ] Example includes toggle or side-by-side comparison

## Technical Tasks

1. Create `examples/z_sort_demo.rs` with overlapping transparent circles at
   varying Z-depths
2. Render both sorted and unsorted output for visual comparison
3. Add to examples README

## Dependencies

- GUP-184: GPU Radix Sort for Z-Order

## Testing Strategy

- Visual inspection of rendered output
- Screenshot comparison

## Success Metrics

- Clear visual difference between sorted and unsorted rendering
- Example compiles and runs without errors

## Risk Assessment

- **Risk**: Transparency rendering may need alpha blending configuration
  - **Mitigation**: Use standard alpha blending pipeline

## Definition of Done

- [ ] Example compiles and runs
- [ ] Visual difference is clearly visible
- [ ] Added to examples README
