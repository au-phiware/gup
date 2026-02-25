# GUP-186: Dynamic Attribute GPU Upload Pipeline

**Status**: ✅ Complete (2025-07-23) **Priority**: Medium **Category**: Feature
Enhancement **Estimated Effort**: 2 days **Dependencies**: GUP-069 (Advanced
Mark Rendering Features)

## Overview

Build the complete GPU upload pipeline for `DynamicAttributeMap`, including
automatic buffer management, dirty-only uploads, and integration with the
rendering loop. GUP-069 provides the data structures but stops at the
`collect_static_values()` level.

## Context

GUP-069 introduced `DynamicAttributeMap` with dirty tracking, generation
counters, and `DynamicAttributeValue` variants (Static, PerInstance,
ShaderDriven). The current implementation collects values on the CPU side but
does not manage GPU buffer allocation, dirty-only partial uploads, or automatic
integration with the `MarkRenderer` rendering loop.

## User Story

**As a** visualization developer **I want** dynamic attributes to automatically
upload to the GPU when changed **So that** I can update mark properties at
runtime without manual buffer management

## Acceptance Criteria

- [x] Automatic GPU buffer creation when attributes are first set
- [x] Dirty-only upload: only changed attributes are re-uploaded to GPU
- [x] Per-instance data uploaded to storage buffers
- [x] Static data uploaded to uniform buffers
- [x] Integration with `MarkRenderer` so dynamic attributes are bound during
      rendering
- [x] Performance: attribute updates + GPU upload < 1ms for typical cases

## Technical Tasks

1. Create `DynamicAttributeBufferManager` that allocates and manages GPU buffers
2. Implement dirty-only upload logic using
   `DynamicAttributeMap::dirty_attributes()`
3. Integrate with `MarkRenderer::render_marks()` to bind dynamic attribute
   buffers
4. Add performance benchmarks for attribute update + upload cycle

## Testing Strategy

- GPU integration tests for buffer allocation and upload
- Performance tests validating <1ms update cycle
- Integration test with `MarkRenderer` end-to-end

## Success Metrics

- Dirty-only uploads reduce GPU bandwidth by >50% vs full re-upload
- Attribute update + upload cycle < 1ms for 100 attributes
- Zero regression in existing mark rendering performance

## Risk Assessment

- **Medium risk**: buffer management requires careful alignment and sizing
- Must handle buffer resizing when per-instance data grows

## Definition of Done

- [x] Automatic buffer management for dynamic attributes
- [x] Dirty-only upload implemented and tested
- [x] Integration with MarkRenderer rendering loop
- [x] Performance benchmarks pass
- [x] All existing tests continue to pass

## Implementation Summary

### Files Changed

| File                                      | Change                                                                                        |
| ----------------------------------------- | --------------------------------------------------------------------------------------------- |
| `src/mark/advanced_rendering.rs`          | Added `DynamicAttributeBufferManager`, `UploadStats`, helper methods on `DynamicAttributeMap` |
| `src/mark/renderer.rs`                    | Added `render_marks_with_dynamic_attrs()` method                                              |
| `src/mark.rs`                             | Updated exports to include new types                                                          |
| `src/lib.rs`                              | Updated exports to include new types                                                          |
| `src/prelude.rs`                          | Updated exports to include new types                                                          |
| `tests/dynamic_attribute_buffer_tests.rs` | **New**: 18 GPU integration tests                                                             |

### New Types Introduced

- **`DynamicAttributeBufferManager`**: Manages GPU buffer lifecycle for dynamic
  attributes. Allocates uniform buffers for static attributes and storage
  buffers for per-instance data. Supports dirty-only uploads, automatic buffer
  resizing, and bind group creation.
- **`UploadStats`**: Statistics tracking for upload operations including full
  uploads, partial uploads, storage uploads, buffer resizes, and bandwidth
  savings.
- **`ManagedBuffer`** (internal): Tracks individual GPU buffer state (capacity,
  length).

### New Methods on `DynamicAttributeMap`

- `collect_dirty_static_values()` — returns only changed static values with
  their sorted indices
- `collect_per_instance_data(name)` — returns per-instance values for a named
  attribute
- `per_instance_attribute_names()` — sorted list of per-instance attribute names
- `dirty_per_instance_attributes()` — sorted list of dirty per-instance names
- `mappings()` — accessor for the raw mappings HashMap

### New Method on `MarkRenderer`

- `render_marks_with_dynamic_attrs()` — renders marks with both primary and
  dynamic attribute bind groups

### Test Counts

- 15 new unit tests in `advanced_rendering.rs` (total 52)
- 18 new GPU integration tests in `dynamic_attribute_buffer_tests.rs`
- 33 new tests total
- All 1197 existing library tests continue to pass (1 pre-existing flaky test:
  GUP-187)
