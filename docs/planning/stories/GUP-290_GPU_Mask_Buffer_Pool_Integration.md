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
