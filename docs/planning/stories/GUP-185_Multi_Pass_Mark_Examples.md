# GUP-185: Multi-Pass Mark Examples

**Status**: ✅ Complete (2025-02-27) **Priority**: Low **Category**: Examples /
Documentation **Estimated Effort**: 1 day **Dependencies**: GUP-069 (Advanced
Mark Rendering Features)

## Overview

Create example marks that use multi-pass rendering to validate the multi-pass
API with visual output. These examples serve as both documentation and
integration tests.

## Context

GUP-069 introduced `MultiPassConfig`, `MultiPassRenderer`, and
`MarkInfoImpl::create_render_pipeline_for_pass()` for multi-pass rendering.
While the API is fully tested, no visual examples exist yet demonstrating the
feature in action.

## User Story

**As a** visualization developer **I want** to see working examples of
multi-pass mark rendering **So that** I understand how to implement custom marks
with fill + outline, shadow effects, and similar multi-layer visuals.

## Acceptance Criteria

- [x] Stroked circle example: fill pass + outline pass
- [x] Drop-shadow example: shadow pass + main pass
- [x] Example renders correctly and demonstrates visual layering
- [x] README/doc comments explain the multi-pass pattern

## Technical Tasks

1. Create a `StrokedCircle` mark that uses two-pass rendering
2. Create example that renders stroked circles with configurable outline width
3. Create a shadow effect example using multi-pass with offset
4. Add documentation showing the multi-pass configuration pattern

## Testing Strategy

- Visual validation of rendered output
- Ensure examples compile and run without errors

## Success Metrics

- Examples render correctly with visible multi-pass effects
- Examples serve as documentation for the multi-pass API

## Risk Assessment

- **Low risk**: builds on tested infrastructure
- GPU `PolygonMode::Line` requires `NON_FILL_POLYGON_MODE` feature which may not
  be available on all GPUs — examples should handle gracefully

## Definition of Done

- [x] Examples compile and run
- [x] Visual output demonstrates multi-pass rendering
- [x] Documentation explains the pattern

## Implementation Summary

### What Was Implemented

Two multi-pass rendering techniques demonstrated in a single windowed example:

1. **Drop-shadow effect** (left half of window): A shadow pass renders circles
   offset and blurred using a custom vertex entry point (`vs_shadow`) and a
   soft-falloff fragment entry point (`fs_shadow`). A second pass renders the
   crisp main circle on top.

2. **Fill + outline effect** (right half of window): A fill pass renders solid
   circle interiors using `fs_fill`. A second pass renders only the stroke ring
   using `fs_outline`. The two passes use different fragment shader entry points
   within the same shader module.

Both techniques issue multiple draw calls within a single GPU render pass,
following the project's "single render pass per frame" convention.

### Key Files

| File                                              | Change                                      |
| ------------------------------------------------- | ------------------------------------------- |
| `examples/multi_pass_mark_demo.rs`                | **New**: Windowed example with both demos   |
| `src/mark/shaders/circle_multi_pass.vert.wgsl`    | **New**: Vertex shader with vs_main/vs_shadow |
| `src/mark/shaders/circle_multi_pass.frag.wgsl`    | **New**: Fragment shader with 4 entry points |
| `tests/advanced_mark_rendering_tests.rs`          | +3 tests for multi-pass config validation   |
| `examples/README.md`                              | Added multi_pass_mark_demo entry            |

### Test Counts

- 3 new integration tests (shadow config, fill+outline config, pipeline mismatch)
- All 23 advanced_mark_rendering_tests pass
- All examples compile (`cargo check --examples`)
