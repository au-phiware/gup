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

## Retrospective

**Completed**: 2025-02-27

### Key Technical Learnings

#### Multi-Pass via Shader Entry Points (Not Separate Shaders)

- **Challenge**: The story suggested creating separate mark types (e.g.,
  `StrokedCircle`) for multi-pass rendering. But creating a whole new `Mark`
  implementation just to demonstrate the API seemed over-engineered.
- **Solution**: Used a single WGSL shader module with multiple entry points
  (`vs_main`, `vs_shadow`, `fs_main`, `fs_fill`, `fs_outline`, `fs_shadow`).
  The `RenderPassConfig` struct's `vertex_entry_point` and
  `fragment_entry_point` fields select which entry point each pass uses. This
  avoided any new Rust types while exercising the full multi-pass API.
- **Pattern**: When demonstrating multi-pass rendering, prefer multiple entry
  points in a shared shader over creating separate mark types. This keeps the
  example focused on the multi-pass pattern rather than mark boilerplate.

#### Clip-Space Stroke Width Must Match Radius Units

- **Challenge**: Initial stroke widths (3.0–7.0) were far too large relative to
  clip-space radii (0.08–0.11). The fragment shader computes
  `stroke_thickness = stroke_width / radius`, so a ratio of 37+ covered the
  entire circle in stroke colour with no visible ring.
- **Solution**: Set `stroke_width` to ~15–25% of radius (0.015–0.025), giving a
  clear visible ring.
- **Pattern**: When working with mark shaders that use ratio-based stroke
  calculations, keep stroke and radius in the same coordinate space and ensure
  the ratio produces a visible but not dominant ring (typically 10–30%).

#### Arc::try_unwrap Pitfall with Arc::clone

- **Challenge**: The initial `ensure_renderer` method tried
  `Arc::try_unwrap(Arc::clone(ctx))` to get device access. Since `clone()`
  increments the reference count, `try_unwrap` always fails (there are at least
  two strong references).
- **Solution**: Moved renderer creation into `render_frame` where the context is
  already taken out of the `Option` and unwrapped. This is the same pattern used
  by other examples (`scatter_plot_demo`, `windowed_demo`).
- **Pattern**: Never `Arc::clone` + `Arc::try_unwrap` — the clone guarantees
  failure. Instead, restructure code so exclusive access happens naturally (e.g.,
  inside a `take()` + `try_unwrap()` sequence).

### Architectural Decisions

#### Single Example File vs Two Separate Examples

- **Decision**: Created one example (`multi_pass_mark_demo`) with both
  techniques shown side-by-side, rather than two separate examples.
- **Reasoning**: Both techniques use the same infrastructure (shaders, buffers,
  `MultiPassRenderer`) and are more compelling when compared visually in one
  window. Separate examples would duplicate ~70% of the code.
- **Trade-off**: The example is larger (~500 lines), but each section is clearly
  labelled and the overall structure is readable.
- **Future**: Additional multi-pass techniques (glow, depth peeling) can be
  added as sections in the same example.

#### Custom Shaders vs MarkInfoImpl::create_render_pipeline_for_pass

- **Decision**: Created pipelines directly in the example rather than using
  `MarkInfoImpl::create_render_pipeline_for_pass`, which only uses the mark's
  built-in shaders (not our custom multi-pass shaders).
- **Reasoning**: The multi-pass shaders have entry points (`vs_shadow`,
  `fs_fill`, etc.) that the Circle mark's shaders don't have. Using
  `create_render_pipeline_for_pass` would override the entry points but still
  use Circle's shaders, which lack the shadow vertex offset logic.
- **Trade-off**: The example creates pipelines manually, which means more
  boilerplate. But this accurately reflects how a real user would create
  multi-pass pipelines with custom effects.

### Development Workflow Insights

- The first screenshot captured was from the wrong window (a particle
  simulation) — always verify the PID matches the expected process.
- The `Arc::try_unwrap` bug was invisible at runtime (the `if let Ok` silently
  failed), so the renderer was never created and the window showed only the
  clear colour. Adding explicit error logging would have caught this sooner.
- Pre-existing flaky test `test_registry_scalability` (6ms vs 5ms target)
  intermittently fails under load — unrelated to this story.
