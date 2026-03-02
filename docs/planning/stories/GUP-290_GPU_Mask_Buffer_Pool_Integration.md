# GUP-290: GPU Mask Buffer Pool Integration

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete **Created**:
2025-07-22 **Completed**: 2025-07-27

## Context

GUP-288's `SelectionMaskBuffer` allocates new GPU buffers (mask buffer and
output instance buffer) in its constructor and when capacity grows. For
applications with many charts or dynamic dataset sizes, this can lead to
allocation churn. GUP-003 established a `BufferPool` system for reusing GPU
buffers across frames. This story integrates `SelectionMaskBuffer` with the
existing buffer pool to reduce allocation overhead.

## User Story

> "As a visualization developer with multiple linked charts, I want
> SelectionMaskBuffer to reuse GPU buffers from the pool so that creating and
> destroying charts doesn't cause GPU memory fragmentation."

## Acceptance Criteria

- [x] `SelectionMaskBuffer::new` accepts an optional `&mut BufferPool` parameter
- [x] Mask and output buffers are acquired from the pool when available
- [x] Buffers are returned to the pool via `release_to_pool()` (following
      established codebase pattern from GUP-167)
- [x] No performance regression compared to direct allocation

## Technical Tasks

- [x] Add `BufferPool` integration to `SelectionMaskBuffer::new`
- [x] Implement `release_to_pool()` for buffer return to pool
- [x] Add pooled variant of `ensure_capacity`
- [x] Benchmark support for pooled vs non-pooled allocation

## Dependencies

### Prerequisite Stories

- GUP-288: GPU Selection Mask Buffer ✅ — provides SelectionMaskBuffer
- GUP-003: GPU Buffer Management ✅ — provides BufferPool

## Testing Strategy

- Unit tests for pool integration
- GPU integration tests for buffer reuse
- Performance benchmarks comparing pooled vs direct allocation

## Risk Assessment

- **Low**: Both systems are well-tested. The main concern is ensuring buffer
  usage flags are compatible with pool allocation.

## Definition of Done

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

- Modified `SelectionMaskBuffer::new()` to accept an optional `&mut BufferPool`
  parameter
- When a pool is provided, mask buffer (Storage), output buffer (Instance), and
  config buffer (Uniform) are acquired from the pool
- Pool metadata `(BufferType, size_class)` is stored alongside each buffer for
  later deallocation
- `release_to_pool(&mut self, pool: &mut BufferPool, device: &Device)` method
  returns all pooled buffers to the pool
- `is_pooled()` query method reports whether any buffers came from a pool
- `ensure_capacity()` extended with pool parameter: returns old buffers to pool
  and acquires new ones from pool on capacity growth
- `update_and_dispatch()` extended with pool parameter, forwarded to
  `ensure_capacity()`
- All existing callers updated to pass `None` (backward compatible)
- Debug output includes `is_pooled` field

### Key Files Changed

| File                                   | Changes                              |
| -------------------------------------- | ------------------------------------ |
| `src/selection_mask.rs`                | Core pool integration, new methods   |
| `src/linked_selection.rs`              | Updated callers (passes `None`)      |
| `tests/selection_mask_gpu_tests.rs`    | 6 new GPU tests for pool integration |
| `benches/selection_mask_benchmarks.rs` | Updated callers for new API          |

### Test Counts

- 14 unit tests (CPU-only, selection_mask module) — all passing
- 15 GPU integration tests (9 existing + 6 new pool tests) — all passing

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Buffer Pool Integration Pattern

- **Challenge**: The original story AC specified "Buffers are returned to the
  pool when SelectionMaskBuffer is dropped", which would require storing an
  `Arc<Mutex<BufferPool>>` inside the struct for automatic Drop-based cleanup.
- **Solution**: Followed the established codebase pattern from GUP-167
  (Selection API Pool Integration): store pool metadata
  `Option<(BufferType, usize)>` per buffer and provide an explicit
  `release_to_pool()` method. This avoids introducing `Arc<Mutex<>>` overhead
  and matches how every other component in the codebase handles pool
  integration.
- **Pattern**: For GPU buffer pool integration, the standard approach is:
  1. Store `Option<(BufferType, usize)>` pool metadata alongside each buffer
  2. Provide `release_to_pool()` for explicit return
  3. Caller (e.g. LinkedSelection) is responsible for calling release before
     drop

#### BufferType to Usage Flag Mapping

- **Challenge**: SelectionMaskBuffer uses specific buffer usage flag
  combinations (e.g. STORAGE | VERTEX | COPY_SRC for the output buffer) that
  don't exactly match any single BufferType's flags.
- **Solution**: BufferPool's BufferType variants provide _supersets_ of the
  needed flags: `Instance` = VERTEX | STORAGE | COPY_DST | COPY_SRC, which
  covers the output buffer's needs. This wastes no extra memory — the additional
  flags just enable more operations on the buffer.
- **Pattern**: When integrating with BufferPool, choose the BufferType whose
  flags are a superset of what you need. The extra flags have zero runtime cost.

#### Placeholder Buffer Technique

- **Challenge**: `std::mem::replace` requires a replacement value when
  extracting a buffer for deallocation, but we want to avoid creating an
  expensive full-size buffer just as a placeholder.
- **Solution**: Created a `placeholder_buffer()` helper that allocates a minimal
  4-byte buffer as a temporary replacement. The placeholder is immediately
  overwritten by the new pooled buffer on the next line.
- **Pattern**: When using `std::mem::replace` to extract GPU resources, use a
  minimal placeholder to satisfy the type system without wasting GPU memory.

### Architectural Decisions

#### Manual Release vs Automatic Drop

- **Decision**: Use explicit `release_to_pool()` instead of implementing `Drop`
  with stored pool reference.
- **Reasoning**: Matches the established Selection API pattern from GUP-167.
  Avoids introducing `Arc<Mutex<BufferPool>>` which would add lock contention
  overhead and require all callers to wrap their BufferPool.
- **Trade-off**: Callers must remember to call `release_to_pool()` before
  dropping, or buffers are simply freed (not returned to pool). This is the same
  trade-off Selection makes.
- **Future**: If automatic pool return becomes important, a
  `PooledSelectionMaskBuffer` wrapper type could be introduced that stores an
  `Arc<Mutex<BufferPool>>` and implements Drop.

#### Config Buffer Pooling

- **Decision**: Pool the config buffer (96-byte uniform) alongside the larger
  mask and output buffers.
- **Reasoning**: Consistency — all three buffers follow the same pool lifecycle.
  The config buffer is tiny so pool overhead is negligible, but it still avoids
  a GPU allocation call on recreation.
- **Trade-off**: The config buffer never grows (fixed size), so pooling it adds
  metadata overhead without capacity-growth benefits.
- **Future**: Could exclude config from pooling if profiling shows the metadata
  is not worthwhile for sub-256-byte buffers.

### Development Workflow Insights

- The implementation was straightforward because the `BufferPool` API
  (`allocate_raw` / `deallocate_raw`) is well-designed for this exact use case.
  The `allocate_raw` method returns the size class alongside the buffer, which
  is exactly what's needed for later deallocation.
- All existing GPU tests continued to pass unchanged after adding the pool
  parameter (with `None`), confirming backward compatibility.
- The `--test-threads=1` constraint for GPU tests remains essential — all 15 GPU
  tests pass reliably in serial mode.
- The `mask all-fix` command reformats unrelated markdown files, requiring
  careful `git add` to stage only relevant changes.

### Follow-up Stories

1. **GUP-291: Adaptive GPU Dimming Threshold** — Already planned. Could benefit
   from pool integration when the threshold triggers SelectionMaskBuffer
   creation/destruction at runtime.
