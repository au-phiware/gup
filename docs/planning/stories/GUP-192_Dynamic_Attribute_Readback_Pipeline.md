# GUP-192: Dynamic Attribute Readback Pipeline

**Status**: ✅ Complete (2025-07-27) **Priority**: Low **Category**: Feature
Enhancement **Estimated Effort**: 1 day **Dependencies**: GUP-186 (Dynamic
Attribute GPU Upload Pipeline)

## Overview

Add async GPU-to-CPU readback for dynamic attribute buffers managed by
`DynamicAttributeBufferManager`. Currently the pipeline only supports upload
(CPU→GPU); this story adds download (GPU→CPU) for debugging, validation, and
CPU-side post-processing of GPU-computed attribute values.

## Context

GUP-186 introduced `DynamicAttributeBufferManager` with automatic buffer
lifecycle, dirty-only uploads, and MarkRenderer integration. However, there is
no way to read attribute data back from the GPU. This is needed for:

- Debugging GPU-computed attributes (e.g., shader-driven values)
- Validating that uploads wrote correct data
- CPU-side analytics on GPU-computed results

## User Story

**As a** visualization developer **I want** to read dynamic attribute values
back from the GPU **So that** I can debug, validate, and post-process
GPU-computed attribute data

## Acceptance Criteria

- [x] Async readback of uniform buffer contents (static attributes)
- [x] Async readback of individual storage buffers (per-instance attributes)
- [x] Staging buffer management for efficient readback
- [x] Integration with existing `GpuBuffer::download()` patterns

## Technical Tasks

1. Add `download_static_values()` to `DynamicAttributeBufferManager`
2. Add `download_per_instance(name)` to `DynamicAttributeBufferManager`
3. Manage staging buffers for GPU→CPU copy operations
4. Write GPU integration tests for readback correctness

## Testing Strategy

- GPU integration tests: upload → readback → verify values match
- Roundtrip test: upload static, modify on GPU (compute shader), readback
- Performance test: readback should not block rendering pipeline

## Success Metrics

- Readback values match uploaded values exactly
- Staging buffer reuse minimizes allocation overhead

## Risk Assessment

- **Low risk**: follows existing `GpuBuffer::download()` patterns
- Async buffer mapping requires careful lifetime management

## Definition of Done

- [x] Async readback methods implemented
- [x] Staging buffer management
- [x] GPU integration tests pass
- [x] Documentation with usage examples

## Implementation Summary

### Files Changed

| File                                      | Change                                                              |
| ----------------------------------------- | ------------------------------------------------------------------- |
| `src/mark/advanced_rendering.rs`          | Added readback methods, staging buffer cache, COPY_SRC buffer flags |
| `tests/dynamic_attribute_buffer_tests.rs` | Added 9 GPU integration tests for readback                          |

### New Methods on `DynamicAttributeBufferManager`

- **`download_static_values()`** — Async method to read back all static
  attribute values from the GPU uniform buffer. Returns `Vec<[f32; 4]>` in
  alphabetical attribute name order.
- **`download_per_instance(name)`** — Async method to read back per-instance
  attribute data from a named GPU storage buffer.
- **`clear_staging_buffers()`** — Releases cached staging buffers to free GPU
  memory.

### New Internal Types

- **`StagingBuffer`** — Tracks a cached `MAP_READ | COPY_DST` buffer and its
  size for reuse across readback calls.

### Internal Helpers

- **`ensure_staging_buffer()`** — Creates or reuses a staging buffer of
  sufficient size for a given cache key.
- **`copy_and_map()`** — Static method that copies data from a source buffer to
  a staging buffer, maps it, and returns the data.

### Buffer Flag Changes

- Uniform buffers now include `COPY_SRC` usage flag (was: `UNIFORM | COPY_DST`)
- Storage buffers now include `COPY_SRC` usage flag (was: `STORAGE | COPY_DST`)

### Test Counts

- 9 new GPU integration tests in `dynamic_attribute_buffer_tests.rs` (total 27)
- All 1543 library tests continue to pass

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Borrow Checker with Cached Staging Buffers

- **Challenge**: The initial
  `readback_buffer(&mut self, ..., source: &wgpu::Buffer)` design failed because
  `source` borrows `self.uniform_buffer` or `self.storage_buffers` immutably
  while `readback_buffer` needs `&mut self` to insert into
  `self.staging_buffers`.
- **Solution**: Split into two phases: `ensure_staging_buffer(&mut self, ...)`
  handles the mutable staging buffer creation, then `copy_and_map(...)` is a
  static method that only takes `&wgpu::Buffer` references and doesn't touch
  `self`. This separates the mutable and immutable borrows into distinct scopes.
- **Pattern**: When a method needs to borrow different fields of `self` mutably
  and immutably, split the operation into a mutable setup phase (modifying
  caches/state) and a static execution phase (only reading from the prepared
  state).

#### COPY_SRC Flag Requirement

- **Challenge**: The existing buffer creation in GUP-186 used
  `UNIFORM | COPY_DST` and `STORAGE | COPY_DST` — no `COPY_SRC`. This means the
  buffers could not be used as source operands in `copy_buffer_to_buffer`, which
  is required for GPU→CPU readback via staging buffers.
- **Solution**: Added `COPY_SRC` to both uniform and storage buffer creation
  flags. This is a minor additional GPU resource cost but essential for readback
  support.
- **Pattern**: When designing GPU buffer managers, always include `COPY_SRC` if
  readback may ever be needed. The overhead is negligible compared to the cost
  of buffer recreation.

### Architectural Decisions

#### Staging Buffer Per Cache Key

- **Decision**: One staging buffer per readback target, cached by key
  (`"uniform"` for the uniform buffer, attribute name for storage buffers).
- **Reasoning**: Each buffer may have a different size, and staging buffers must
  match the data size being read. Per-key caching avoids creating a single large
  staging buffer that wastes memory for small readbacks.
- **Trade-off**: More staging buffer objects, but each is right-sized. An
  alternative would be a single shared staging buffer sized to the largest
  buffer, but that would waste memory and require tracking the maximum size.
- **Future**: If staging buffer count becomes an issue, a pool-based approach
  (like the existing `BufferPool` in `buffer.rs`) could be used.

#### Static `copy_and_map()` Method

- **Decision**: Made `copy_and_map()` a static (associated) function rather than
  a `&self` method.
- **Reasoning**: This resolves the borrow checker conflict by ensuring the
  method doesn't hold any borrows on `self`. The caller provides all required
  buffer references explicitly.
- **Trade-off**: Slightly more verbose call sites, but gains correctness and
  clarity about which borrows are in scope.

### Development Workflow Insights

- The implementation was straightforward and closely followed the existing
  `GpuBuffer::download()` pattern — the main complexity was the borrow checker
  issue with staging buffer caching, which required the two-phase approach.
- Writing tests first helped validate the API design before committing to the
  internal structure.
- The story was well-scoped and completed within the estimated 1-day effort.
