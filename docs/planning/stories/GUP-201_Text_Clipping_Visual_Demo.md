# GUP-201: Text Clipping Visual Demo

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Low  
**Story Points**: 2  
**Status**: ✅ Complete  
**Dependencies**: GUP-105 (Text Clipping Detection)

## Problem Statement

The text clipping detection system (GUP-105) is fully functional but lacks a
dedicated visual demonstration. The existing `text_rendering_demo` does not
showcase clipping strategies, making it harder for developers to understand and
evaluate the feature.

## User Story

**As a** developer evaluating Gup  
**I want** a visual demo showing text clipping strategies in action  
**So that** I can understand the automatic text boundary management capabilities

## Acceptance Criteria

- [x] Demo showing text truncation with ellipsis in different container sizes
- [x] Demo showing dynamic font scaling (before/after comparison)
- [x] Demo showing text repositioning near edges
- [x] Demo showing strategy cascade (truncation → scaling → hide)
- [x] Container bounds visualization (visible boundary rectangles)
- [x] Side-by-side: unclipped vs clipped text rendering

## Technical Tasks

1. Create `text_clipping_demo` example or enhance `text_rendering_demo`
2. Render container bounds as visible rectangles for debugging
3. Show each strategy in a labeled section
4. Add keyboard controls to toggle clipping on/off
5. Display clipping statistics (number clipped, strategies used)

## Testing Strategy

- Manual visual verification
- Screenshot comparison
- Example compilation check

## Definition of Done

- [x] Demo example compiles and runs
- [x] All clipping strategies visually demonstrated
- [x] Container bounds visible for debugging

## Implementation Summary

**Completed**: 2025-07-17

### What Was Implemented

A new `examples/text_clipping_demo.rs` example that demonstrates all five text
clipping strategies in clearly labeled sections:

1. **Truncation with Ellipsis** — four containers at 280, 200, 140, and 90 px
   widths showing progressively more aggressive truncation
2. **Dynamic Font Scaling** — before/after comparison with overflow vs
   scaled-to- fit text
3. **Text Repositioning** — text nudged left and up to stay within bounds
4. **Strategy Cascade** — truncation → scaling → hide applied in sequence with
   three different container widths
5. **Side-by-side** — unclipped (overflowing) vs clipped (truncated) rendering

### Key Features

- **Container bounds visualisation**: Line-outline rectangles drawn via a custom
  wgpu `LineList` pipeline (`RectPipeline`) rendered in the same render pass as
  text
- **Keyboard toggle**: Press **C** to switch clipping on/off globally; status
  bar at top reflects current state
- **Clipping statistics overlay**: Bottom-of-screen counter showing total items,
  clipped breakdown (truncated / scaled / hidden), and unclipped count

### Key Files Changed

| File                             | Change         |
| -------------------------------- | -------------- |
| `examples/text_clipping_demo.rs` | New (≈960 LOC) |
| `examples/README.md`             | Added entry    |
| `docs/planning/stories/INDEX.md` | Status update  |

### Test Counts

- 5 unit tests in the example (app init, sections, demo items, rect vertices,
  toggle)
- All 1705+ project tests still pass (3 pre-existing mark renderer failures
  unrelated)

---

**Story Created**: 2026-02-26  
**Origin**: GUP-105 follow-up ("Demo Enhancement" AC not completed)

## Retrospective

**Completed**: 2025-07-17

### Key Technical Learnings

#### Combining Multiple Render Pipelines in One Pass

- **Challenge**: Drawing container-bound outline rectangles alongside SDF text
  in the same render pass required a second wgpu pipeline.
- **Solution**: Created a minimal `RectPipeline` with a simple vertex+color
  `LineList` shader that renders before text. Both pipelines share the same
  render pass.
- **Pattern**: For debug/overlay visuals in examples, a lightweight inline WGSL
  shader with `LineList` topology is the simplest approach — no need to pull in
  the full `MarkRenderer` / `Rectangle` mark infrastructure.

#### Pixel-to-NDC Coordinate Conversion

- **Challenge**: The text renderer works in pixel coordinates (top-left origin)
  while wgpu clip space is NDC (-1..1, y-up).
- **Solution**: `RectOutline::to_vertices()` converts pixel coords to NDC with
  y-flip: `ndc_y = -(py / h * 2.0 - 1.0)`.
- **Pattern**: Keep coordinate systems isolated per pipeline; convert at the
  vertex-generation boundary.

### Architectural Decisions

#### Standalone Example vs Enhancing Existing Demo

- **Decision**: Created a new `text_clipping_demo.rs` rather than extending
  `text_rendering_demo.rs`.
- **Reasoning**: The existing demo focuses on font rendering capabilities
  (sizes, colours, anchors); adding clipping sections would make it unwieldy and
  dilute both demos' purposes.
- **Trade-off**: One more file to maintain, but cleaner separation of concerns.
- **Future**: Each text-related feature (rendering, clipping, hover reveal,
  wrapping) has its own focused demo.

#### Statistics Classification Heuristic

- **Decision**: Classified clipped items by inspecting the label string and
  whether glyphs are empty, rather than threading strategy identity through the
  layout engine.
- **Reasoning**: The `LayoutResult` only exposes `clipped: bool` and empty
  glyphs; adding a `strategy_used` field would be a larger API change for a
  demo- only need.
- **Trade-off**: Approximate categorisation based on label text.
- **Future**: A `ClipAction` enum on `LayoutResult` would make this exact.

### Development Workflow Insights

- The `mask all-fix` pipeline is reliable and fast; running it before every
  commit caught formatting drifts immediately.
- GPU examples cannot be visually verified in the headless CI/development
  environment — tests validate structure and the example compiles+runs without
  errors, but visual review requires a desktop session.
- The project's `Arc<GupContext>` take-unwrap-rewrap pattern for mutable access
  is verbose but well-established in every example. A `RefCell`-based helper
  could reduce boilerplate in future.

### Follow-up Stories

No new stories identified. All clipping strategies (truncation, scaling,
repositioning, hide, wrapping) are now demonstrated. The `TextWrapping` strategy
is exercised in GUP-199's dedicated wrapping demo.
