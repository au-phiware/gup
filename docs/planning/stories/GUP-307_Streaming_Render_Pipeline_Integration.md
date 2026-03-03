# GUP-307: Streaming Render Pipeline Integration

## Story Overview

**Initiative**: Debug & Development Tools **Status**: 💡 New **Created**:
2025-07-19

## Context

GUP-244 delivered the `DataStream<T>` builder API, `Selection::stream()`
integration, and the observable subscriber pattern. However, the current
integration stores the DataStream in the Selection via type-erased storage and
requires callers to manually manage flush timing relative to render passes. The
Selection's `prepare_render()` does not yet detect an active stream or
automatically use the DataStream's active GPU buffer.

This story closes that gap by making `prepare_render()` stream-aware: when a
DataStream is attached, the Selection should automatically flush pending changes
and use the stream's active buffer for instanced rendering, eliminating manual
flush management from the render loop.

## User Story

> "As a visualization developer, I want the Selection's render pipeline to
> automatically detect and use an attached DataStream's GPU buffer, so that I
> don't need to manually coordinate flush timing between the stream and the
> render pass."

## Acceptance Criteria

- [ ] When a `DataStream` is attached via `Selection::stream()`, calling
      `prepare_render()` or `prepare_render_bound()` automatically flushes the
      stream's dirty regions to the GPU before uploading instances.
- [ ] The render pipeline uses the DataStream's active buffer directly for
      instance data when a stream is attached.
- [ ] The Selection correctly handles the transition between static data mode and
      streaming mode without GPU validation errors.
- [ ] An integration test demonstrates a full render loop: push → auto-flush →
      render, with GPU readback validating the rendered data.

## Dependencies

### Prerequisite Stories

- GUP-244: Streaming Data Builder API ✅ — provides `DataStream<T>`,
  `Selection::stream()`, and the stream accessor methods.

## Testing Strategy

- Integration test with GPU readback verifying data correctness after
  stream-aware prepare_render.
- Test transition from static to streaming and back.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
