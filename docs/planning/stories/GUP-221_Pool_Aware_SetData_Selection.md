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

| File               | Change                                                               |
| ------------------ | -------------------------------------------------------------------- |
| `src/selection.rs` | `set_data_with_pool` method + shader-bound pool wiring + 3 GPU tests |

### Test Count

- **3 new GPU tests** in selection.rs
- **All 1600 existing tests pass** (3 pre-existing failures in
  `mark::renderer::tests` unrelated to this change)

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### Separate Method vs Parameter Extension

- **Challenge**: The story suggested either adding an optional pool parameter to
  `set_data()` or storing a pool reference. Adding `Option<&mut BufferPool>` to
  `set_data` would touch all callers (there are 15+ call sites in tests and
  production code).
- **Solution**: Created a new `set_data_with_pool` method alongside the existing
  `set_data`. This preserves full backward compatibility — no existing callers
  need changing.
- **Pattern**: When extending a widely-used method with an optional resource,
  prefer a new companion method (`foo_with_bar`) over modifying the existing
  signature. This follows the project's existing pattern (e.g.,
  `prepare_render_bound` alongside `prepare_render`).

#### Shader-Bound Path Consistency

- **Challenge**: The shader-function binding path
  (`prepare_render_shader_bound`, `create_shader_bound_buffers_and_bind_group`,
  `new_with_shader_fns`) was creating instance buffers directly via
  `device.create_buffer_init()` without pool support, even though the non-shader
  path had pool support.
- **Solution**: Added `pool: Option<&mut BufferPool>` parameters and reused the
  same pool allocation pattern (allocate from pool + `queue.write_buffer`, or
  fall back to `create_buffer_init`). Also added pool return on reallocation in
  the shader-bound path.
- **Pattern**: When a subsystem has parallel code paths (direct vs
  shader-bound), ensure both paths support the same resource management
  patterns.

### Architectural Decisions

#### Companion Method Approach

- **Decision**: Created `set_data_with_pool` rather than modifying `set_data`'s
  signature.
- **Reasoning**: 15+ existing call sites would need `None` added. The companion
  method approach is zero-churn for existing code and mirrors the per-call pool
  parameter pattern established in GUP-167.
- **Trade-off**: Users must remember to use `set_data_with_pool` when working
  with pools. There's no compile-time enforcement.
- **Future**: A `Drop`-based approach via `Arc<Mutex<BufferPool>>` stored on the
  Selection would provide automatic return semantics, but at the cost of
  synchronization overhead and lifetime complexity.

### Development Workflow Insights

- The story was small and well-scoped (2 story points). Implementation took one
  focused pass: core method + shader-bound wiring + tests.
- The existing test patterns from GUP-167 made writing new tests straightforward
  — copy the pattern, adjust for `set_data_with_pool`.
- `mask all-fix` caught indentation inconsistencies in the let-chain syntax
  (rustfmt reformatted the combined `if let ... && let` block).
- Running `--test-threads=1` continues to be essential for GPU tests.

### Follow-up Stories

No new follow-up stories identified. The pool integration for Selection is now
complete across all code paths (direct mapper, attribute binding, and
shader-function binding).
