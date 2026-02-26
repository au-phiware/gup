# GUP-221: Pool-Aware set_data for Selection

**Status**: ✅ Complete (2026-02-27)

## Story Overview

**Title**: Pool-Aware set_data for Selection **Epic**: Phase 1 Initiative 4 -
Advanced Data Mapping **Priority**: Low **Story Points**: 2

## Context

GUP-167 integrated `BufferPool` into `Selection::prepare_render()`, but
`Selection::set_data()` destroys the render state (setting it to `None`) without
returning pool-allocated instance buffers. This means buffers are silently
dropped instead of being recycled, defeating the pool's purpose when `set_data`
is used in high-churn workflows.

Additionally, the shader-function binding path (`prepare_render_shader_bound`)
was not integrated with the pool in GUP-167.

## User Story

**As a** library developer using pool-allocated Selection buffers **I want**
`set_data()` to return the instance buffer to the pool **So that** pool
recycling works correctly regardless of how the Selection's data is updated

## Acceptance Criteria

- [x] `set_data()` accepts an optional `&mut BufferPool` parameter (or the
      Selection stores a weak pool reference)
- [x] When a pool-allocated instance buffer exists, it is returned to the pool
      before clearing the render state
- [x] The shader-function binding path supports pool allocation
- [x] No regression in existing tests

## Technical Tasks

1. Decide on API approach: per-call pool parameter on `set_data`, stored pool
   reference, or a combined `set_data_with_pool` method
2. Implement pool return in `set_data` or its replacement
3. Wire `BufferPool` through `prepare_render_shader_bound` and
   `SelectionRenderState::new_with_shader_fns`
4. Update tests and callers

## Dependencies

- **Requires**: GUP-167 (GpuBufferPool Selection Integration) ✅

## Testing Strategy

- Unit test: verify `set_data` returns buffer to pool
- GPU test: cycle `set_data` + `prepare_render` with pool, verify pool stats
- Verify shader-function path works with pooled buffers

## Risk Assessment

- **API ergonomics**: Adding a pool parameter to `set_data` is awkward. May need
  a stored reference approach instead.
- **Complexity**: Low — straightforward extension of GUP-167 patterns.

## Definition of Done

- [x] All acceptance criteria met
- [x] No performance regression
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

1. **`Selection::set_data_with_pool`** — New method that returns the
   pool-allocated instance buffer to the `BufferPool` before clearing the render
   state. The existing `set_data` method remains unchanged for backward
   compatibility.

2. **Pool support in shader-function binding path** — Wired `BufferPool` through
   `prepare_render_shader_bound`, `create_shader_bound_buffers_and_bind_group`,
   and `SelectionRenderState::new_with_shader_fns`. These methods now allocate
   instance buffers from the pool when provided, and return old buffers on
   reallocation.

### Key Files Changed

| File             | Change                                                    |
| ---------------- | --------------------------------------------------------- |
| `src/selection.rs` | `set_data_with_pool` method + shader-bound pool wiring + 3 GPU tests |

### Test Count

- **3 new GPU tests** in selection.rs
- **All 1600 existing tests pass** (3 pre-existing failures in
  `mark::renderer::tests` unrelated to this change)
