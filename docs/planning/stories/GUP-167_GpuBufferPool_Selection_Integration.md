# GUP-167: GpuBufferPool Integration for Selection Rendering

**Status**: 🚧 In Progress

## Story Overview

**Title**: Wire Selection instance buffers through GpuBufferPool **Epic**: Phase
1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**: 3

## Context

GUP-165 (Selection API Render Integration) created instance buffers via
`device.create_buffer_init()` directly. The GpuBufferPool from GUP-030 provides
buffer reuse and memory pressure management, but wasn't integrated because each
Selection owns its buffers exclusively and the pool's allocation/deallocation
lifecycle doesn't naturally fit RAII ownership.

In dynamic scenarios where Selections are frequently created and destroyed
(e.g., animated transitions, data streaming), pool-based allocation could reduce
GPU memory churn.

## User Story

**As a** library developer building dynamic visualisations **I want** Selection
instance buffers to be allocated from the GpuBufferPool **So that** buffer reuse
reduces GPU memory allocation overhead in high-churn scenarios

## Acceptance Criteria

- [ ] Selection's `prepare_render()` allocates instance buffers from
      `BufferPool` instead of `device.create_buffer_init()`
- [ ] Buffers are returned to the pool when the Selection drops or reallocates
- [ ] Benchmark shows reduced allocation count for create/destroy cycles
- [ ] No regression in rendering correctness (all existing GPU tests pass)

## Dependencies

- **Requires**: GUP-030 (GPU Buffer Pool Management) ✅
- **Requires**: GUP-165 (Selection API Render Integration) ✅

## Testing Strategy

- Benchmark: measure allocation count for 1000 Selection create/destroy cycles
- GPU integration tests: verify rendering still works with pooled buffers
- Memory pressure test: verify pool eviction doesn't break rendering

## Definition of Done

- [ ] All acceptance criteria met
- [ ] No performance regression in existing tests
- [ ] `mask all-fix` clean
