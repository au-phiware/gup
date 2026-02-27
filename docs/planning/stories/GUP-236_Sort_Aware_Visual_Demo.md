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

## Retrospective

**Completed**: 2025-07-21

### Key Technical Learnings

#### Alpha Blending and Draw Order

- **Challenge**: Demonstrating the visual impact of instance draw order on
  transparent overlapping marks requires careful data setup — the artifacts must
  be obviously wrong, not subtly off.
- **Solution**: Used 6 circles at varying conceptual Z-depths with alpha values
  between 0.55 and 0.80. Front-to-back ordering makes the large background
  circle dominate (drawn last, overdrawing everything), while back-to-front
  ordering produces correct layering.
- **Pattern**: For transparency demos, use 5–7 overlapping elements with alpha
  0.55–0.80 and highly distinct hue per depth layer. This makes ordering
  artifacts immediately obvious.

#### Index Format Must Match Data Type

- **Challenge**: `Circle::generate_indices()` returns `Vec<u32>` but initial
  code used `wgpu::IndexFormat::Uint16`, causing misinterpreted index data.
- **Solution**: Changed to `wgpu::IndexFormat::Uint32` to match the actual data
  type.
- **Pattern**: Always verify index buffer format matches the type returned by
  `Mark::generate_indices()`. Consider adding a `Mark::index_format()` method to
  make this explicit.

### Architectural Decisions

#### CPU-Side Sorting vs GPU Radix Sort for Demo

- **Decision**: Used CPU-side sorting (two pre-sorted `Vec<CircleInstance>`) to
  demonstrate the visual effect rather than invoking the GPU compute pipeline.
- **Reasoning**: The demo's goal is showing _why_ sorting matters, not how the
  GPU sort works. Two static instance buffers with different orderings are
  simpler and more didactic than wiring up `PooledComputeInstanceFilter`.
- **Trade-off**: Doesn't exercise the actual GPU radix sort pipeline. A more
  advanced demo could use `dispatch_sorted()` with live animation.
- **Future**: Could extend with a toggle that dynamically re-orders instances
  via the GPU compute pipeline.

#### Side-by-Side vs Toggle

- **Decision**: Implemented side-by-side comparison (two clusters rendered
  simultaneously) rather than a full-screen toggle.
- **Reasoning**: Side-by-side allows immediate visual comparison without needing
  to remember what the previous view looked like.
- **Trade-off**: Each cluster is smaller (half the window width) — a toggle
  would allow larger circles.
- **Future**: Could add a Space-key toggle for full-screen view of either mode.

### Development Workflow Insights

- The `multi_pass_mark_demo.rs` example was an excellent template — its pattern
  of `GupContext::headless()` → `add_surface()` → `begin_frame_for_surface()`
  with `Arc::try_unwrap` is the established windowed example pattern.
- The circle vertex/fragment shaders work out-of-the-box with storage buffer
  instancing and alpha blending — no custom shaders were needed.
- `mask all-fix` catches markdown formatting issues from the pre-commit hook,
  which requires running `prettier --write` on story files before committing.
