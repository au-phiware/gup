# GUP-179: Shader Function Uniform Live Update

**Status**: ✅ Complete

## Story Overview

**Title**: Live update of shader function parameters without data re-upload
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Medium
**Story Points**: 5

## Context

GUP-177 implemented GPU-side shader function attribute bindings where raw data
values are uploaded to the GPU and shader functions transform them in the vertex
shader. Currently, changing shader function parameters (e.g., adjusting a
`LinearScale` domain) requires calling `prepare_render_bound()` again, which
re-runs all CPU extractors and re-uploads instance data.

The key performance advantage of GPU shader function bindings is that re-mapping
— changing how data maps to visual properties — should only require updating the
shader function's uniform buffer, not re-uploading all instance data. This story
adds that capability.

## User Story

**As a** library user exploring large datasets interactively **I want** to
change shader function parameters (e.g., scale domain/range) without
re-uploading instance data **So that** re-mapping is near-instantaneous for
100K+ point datasets

## Acceptance Criteria

- [x] New method `update_shader_uniforms(name, new_shader_fn)` on Selection
- [x] Method only re-uploads the uniform buffer for the named attribute
- [x] Instance data is NOT re-uploaded (only uniform buffer writes)
- [x] Performance: uniform update completes in <1ms for any dataset size
- [x] Error handling: returns error if attribute name not found or not GPU-bound

## Dependencies

- **Requires**: GUP-177 (GPU Shader Function Attribute Binding) ✅

## Testing Strategy

- Unit test: verify only uniform buffer is updated (no instance re-upload)
- GPU integration test: update shader params and render in same frame
- Performance test: compare uniform update time vs full prepare_render time

## Risk Assessment

- **Low risk**: The uniform buffers are already stored in SelectionRenderState;
  this is primarily a convenience API over `queue.write_buffer()`.

## Definition of Done

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean
- [x] Performance comparison included

## Implementation Summary

### What Was Implemented

A new public method `update_shader_uniforms<S>()` on `Selection<T, M>` that
enables near-instantaneous re-mapping of GPU shader function parameters without
re-uploading instance data. The method:

1. Finds the shader binding by attribute name
2. Validates the new shader function's output type matches the original binding
3. Writes updated uniform bytes to the GPU buffer via `queue.write_buffer()`
4. Updates the stored `ShaderFnInfo` for consistency with future
   `prepare_render_bound()` calls

### Key Files Changed

- `src/selection.rs` — Added `update_shader_uniforms()` method and 7 tests

### Test Coverage

7 new tests (49 total selection tests):

| Test                                              | Type       | Purpose                               |
| ------------------------------------------------- | ---------- | ------------------------------------- |
| `update_shader_uniforms_unit_not_found`           | Unit       | Verify binding storage and lookup     |
| `gpu_update_shader_uniforms_basic`                | GPU        | End-to-end update + render validation |
| `gpu_update_shader_uniforms_not_found_error`      | GPU        | Error for unknown attribute name      |
| `gpu_update_shader_uniforms_not_gpu_bound_error`  | GPU        | Error for CPU-bound attribute         |
| `gpu_update_shader_uniforms_before_prepare_error` | GPU        | Error when render state uninitialised |
| `gpu_update_shader_uniforms_type_mismatch_error`  | GPU        | Error for mismatched output types     |
| `gpu_update_shader_uniforms_performance`          | GPU + Perf | <1ms for 100K points vs full prepare  |
