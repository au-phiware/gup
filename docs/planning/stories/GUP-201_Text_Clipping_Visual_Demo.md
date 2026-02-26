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
