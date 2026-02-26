# GUP-188: Automatic Draw Call Metrics in MarkRenderer

**Status**: ✅ Complete (2025-07-25) **Priority**: Low **Category**: Performance
/ Developer Experience **Estimated Effort**: 0.5 days **Dependencies**: GUP-070
(Mark Performance Optimization)

## Overview

Add a `render_marks_tracked(&mut self, ...)` variant to `MarkRenderer` that
automatically increments `MarkPerformanceMetrics` counters (draw calls,
instances rendered, pipeline switches) without requiring callers to manually
accumulate statistics.

## Context

GUP-070 added `MarkPerformanceMetrics` and `metrics_mut()` to `MarkRenderer`,
but the existing `render_marks()` method takes `&self` and cannot update the
metrics struct. Users who want per-frame metrics must manually call
`metrics_mut().draw_calls += 1` after each render, which is error-prone.

## User Story

**As a** visualization developer using the mark renderer **I want** draw call
and instance metrics to be tracked automatically **So that** I can monitor
rendering performance without manual bookkeeping

## Acceptance Criteria

- [x] `render_marks_tracked()` takes `&mut self` and updates metrics
- [x] Draw call count, instance count, and pipeline switch count are tracked
- [x] Existing `render_marks(&self)` API is unchanged for backward compatibility
- [x] Performance overhead of tracking is <1% (simple counter increments)

## Technical Tasks

1. Add `render_marks_tracked<M: Mark>(&mut self, ...)` method
2. Track `draw_calls += 1` and `total_instances += instance_count`
3. Add corresponding `render_marks_with_patterns_tracked()` variant
4. Add unit tests verifying counter accuracy

## Testing Strategy

- Unit test: render N batches, verify `metrics.draw_calls == N`
- Unit test: render M instances, verify `metrics.total_instances == M`
- Benchmark: verify <1% overhead vs non-tracked path

## Risk Assessment

- **Low risk**: Additive API change with no breaking changes

## Definition of Done

- [x] `render_marks_tracked()` implemented and tested
- [x] Documentation updated with metrics usage examples
- [x] Benchmark confirms negligible overhead
- [x] All existing tests pass

## Implementation Summary

### What Was Implemented

- **5 tracked render methods** in `MarkRenderer`:
  - `render_marks_tracked()` — base tracked render
  - `render_marks_with_patterns_tracked()` — with accessibility patterns
  - `render_marks_multi_pass_tracked()` — multi-pass (also tracks pipeline
    switches)
  - `render_marks_with_state_tracked()` — with viewport/scissor state isolation
  - `render_marks_with_dynamic_attrs_tracked()` — with dynamic attribute buffers
- **`BufferType::Index`** variant added to fix a pre-existing bug where the
  MarkRenderer's index buffer was created with `Storage` usage flags, which
  lacked the `INDEX` usage flag required by `set_index_buffer()`
- **7 new tests** verifying metrics tracking, reset, accumulation across frames,
  and backward compatibility of the non-tracked API

### Key Files Changed

| File                                | Change                                         |
| ----------------------------------- | ---------------------------------------------- |
| `src/mark/renderer.rs`              | Added 5 tracked methods + 7 tests              |
| `src/buffer.rs`                     | Added `BufferType::Index` variant              |
| `docs/mark-system/api-reference.md` | Documented tracked methods and metrics API     |
| `docs/mark-system/performance.md`   | Updated profiling section with tracked example |

### Test Results

- 12 renderer tests pass (7 new + 5 existing)
- 2,100+ total tests pass across all crates, 0 failures

## Retrospective

**Completed**: 2025-07-25

### Key Technical Learnings

#### Index Buffer Usage Flags

- **Challenge**: The existing `MarkRenderer::new()` created its index buffer
  with `BufferType::Storage`, which gives `STORAGE | COPY_DST | COPY_SRC` usage
  flags. When `render_marks()` called `set_index_buffer()`, wgpu raised a
  validation error because the `INDEX` usage flag was missing. This bug was
  latent because no existing tests called `render_marks()` with a full GPU
  pipeline.
- **Solution**: Added a new `BufferType::Index` variant with
  `INDEX | COPY_DST | COPY_SRC` flags and updated `MarkRenderer` to use it.
- **Pattern**: When adding render test infrastructure, always test the full
  pipeline path (set_pipeline → set_bind_group → set_vertex_buffer →
  set_index_buffer → draw). Unit tests that only exercise buffer uploads can
  miss usage-flag mismatches.

#### wgpu v26 RenderPassColorAttachment

- **Challenge**: `wgpu::RenderPassColorAttachment` in v26 requires a
  `depth_slice: Option<u32>` field. Tests adapted from older examples or
  AI-generated code often omit this field.
- **Solution**: Created a `begin_test_render_pass()` helper in the test module
  that includes all required fields, avoiding duplication.
- **Pattern**: Centralise render pass creation in test helpers to insulate tests
  from wgpu API churn.

### Architectural Decisions

#### Tracked Methods Delegate to Non-Tracked

- **Decision**: Each `_tracked` method calls the corresponding non-tracked
  method, then increments counters. Counters are only updated on success (after
  the `?` propagation).
- **Reasoning**: Avoids duplicating the rendering logic. If the render fails,
  metrics are not inflated with phantom draw calls.
- **Trade-off**: One extra function call overhead per draw (trivial vs render
  cost).
- **Future**: If profiling shows the delegation overhead matters (unlikely), the
  bodies could be inlined with a macro.

#### Five Tracked Variants Instead of Two

- **Decision**: Added tracked variants for all five render methods (basic,
  patterns, multi-pass, state, dynamic attrs), not just the two mentioned in the
  story.
- **Reasoning**: Consistency — every rendering path should have a tracked
  equivalent. Leaving some out would force users to mix tracked and manual
  metrics.
- **Trade-off**: Slightly more code surface, but each method is trivial
  (delegate + counter increment).

### Development Workflow Insights

- The story was straightforward — 0.5-day estimate was accurate.
- Discovering the `BufferType::Index` bug was the most valuable side-effect.
  Without a full GPU rendering test, this bug would have persisted until someone
  tried to use `MarkRenderer` with indexed marks in production.
- The `create_circle_render_context` test helper pattern (setting up pipeline,
  bind group, and render target) is reusable for future MarkRenderer tests.
