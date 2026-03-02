# GUP-290: GPU Mask Buffer Pool Integration

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress **Created**:
2025-07-22

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

- [ ] `SelectionMaskBuffer::new` accepts an optional `&mut BufferPool` parameter
- [ ] Mask and output buffers are acquired from the pool when available
- [ ] Buffers are returned to the pool when `SelectionMaskBuffer` is dropped
- [ ] No performance regression compared to direct allocation

## Technical Tasks

- [ ] Add `BufferPool` integration to `SelectionMaskBuffer::new`
- [ ] Implement `Drop` for buffer return to pool
- [ ] Add pooled variant of `ensure_capacity`
- [ ] Benchmark pooled vs non-pooled allocation

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

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
