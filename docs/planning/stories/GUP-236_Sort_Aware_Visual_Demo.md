# GUP-236: Sort-Aware Visual Demo

**Story ID**: GUP-236 **Title**: Sort-Aware Visual Demo **Status**: ✅ Complete
**Priority**: Low **Effort**: S **Created**: 2025-07-20 **Completed**:
2025-07-21 **Dependencies**: GUP-184 (GPU Radix Sort for Z-Order)

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

- [x] Example renders two views: sorted and unsorted transparent marks
- [x] Overlapping transparent circles show visible rendering artifacts without
      sort
- [x] Sorted view shows correct back-to-front compositing
- [x] Example includes toggle or side-by-side comparison

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

- [x] Example compiles and runs
- [x] Visual difference is clearly visible
- [x] Added to examples README

## Implementation Summary

### What Was Implemented

A side-by-side visual demo (`examples/z_sort_demo.rs`) showing the effect of
Z-order sorting on transparent overlapping circle marks:

- **Left cluster**: 6 overlapping semi-transparent circles rendered in
  front-to-back order (incorrect for alpha blending — farther circles overdraw
  nearer ones, producing washed-out indistinct layering).
- **Right cluster**: Same 6 circles rendered in back-to-front order (correct
  compositing — nearer circles properly appear on top of farther ones with clear
  color distinction).

### Key Files Changed

| File                      | Change                                      |
| ------------------------- | ------------------------------------------- |
| `examples/z_sort_demo.rs` | New example — 380 lines                     |
| `Cargo.toml`              | Registered `z_sort_demo` as `[[example]]`   |
| `examples/README.md`      | Added `z_sort_demo` to Technical Deep Dives |

### Architecture

- Uses the same circle shaders (`circle.vert.wgsl`, `circle.frag.wgsl`) and
  `CircleInstance` storage buffer layout as the core mark renderer.
- Two separate instance storage buffers with different orderings of the same
  circle data, rendered in a single render pass with alpha blending.
- Follows the `ApplicationHandler` + `GupContext` window pattern established by
  `multi_pass_mark_demo.rs`.
