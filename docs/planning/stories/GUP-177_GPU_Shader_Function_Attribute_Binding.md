# GUP-177: GPU-Side Shader Function Attribute Binding

**Status**: ✅ Complete (2025-07-18)

## Story Overview

**Title**: Extend attr() to accept ComposableShaderFunction types for GPU-side
attribute transformations **Epic**: Phase 1 Initiative 4 - Advanced Data Mapping
**Priority**: Medium **Story Points**: 8

## Context

GUP-168 implemented CPU-side attribute binding where closures extract values
from data items on the CPU and upload them to the GPU as instance data. The
original vision included binding GPU shader functions directly:

```rust
selection
    .attr("position", linear_scale)   // GPU shader function
    .attr("color", color_map)         // GPU shader function
    .prepare_render(&device, &queue)?;
```

This would run attribute transformations on the GPU, enabling:

- Better performance for large datasets (data stays on GPU)
- Integration with the ComposableShaderFunction composition system
- Dynamic re-mapping without re-uploading instance data

## User Story

**As a** library user with large datasets **I want** to bind shader functions
directly to mark attributes **So that** attribute transformations run on the GPU
instead of the CPU

## Acceptance Criteria

- [x] `attr()` accepts `ComposableShaderFunction` types in addition to closures
- [x] Shader function bindings generate WGSL code for attribute transformation
- [x] Data is uploaded in raw form; transformation happens in the vertex shader
- [x] Mixed CPU closure + GPU shader function bindings work together
- [x] Performance improvement demonstrated for 100K+ point datasets
- [x] Type safety: shader function output types must match mark attribute types

## Dependencies

- **Requires**: GUP-168 (Selection Attribute Binding Pipeline) ✅
- **Requires**: GUP-005 (Shader Function System) ✅

## Testing Strategy

- Unit tests for shader function binding storage
- GPU integration test: render with shader function attribute bindings
- Performance benchmark: CPU closure vs GPU shader function for 100K+ points
- Type safety tests for mismatched shader function output types

## Definition of Done

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean
- [x] Performance benchmark included

## Implementation Summary

### What Was Implemented

1. **`ShaderFnInfo` struct** — Type-erased metadata for GPU shader functions
   (WGSL code, uniform bytes, function name, input/output types).

2. **`ShaderAttributeBinding<T>` struct** — Pairs a CPU raw-value extractor with
   shader function metadata for a single attribute binding.

3. **`Selection::attr_shader()` method** — New fluent API method that accepts a
   `ComposableShaderFunction` alongside a raw-value extractor closure, enabling
   GPU-side attribute transformations.

4. **`generate_shader_bound_vertex_wgsl()` function** — WGSL code generation
   that injects uniform struct definitions, binding declarations, shader
   function code, and transformation statements into the mark's existing vertex
   shader.

5. **`SelectionRenderState::new_with_shader_fns()`** — Creates a custom GPU
   pipeline with a modified vertex shader and expanded bind group layout
   (storage buffer + uniform buffers).

6. **Type safety validation** — `prepare_render_shader_bound()` validates that
   each shader function's output WGSL type matches the mark attribute's expected
   type before creating GPU resources.

### Key Files Changed

- `src/selection.rs` — All implementation (types, methods, WGSL generation,
  tests)

### Test Summary

- **57 selection tests** pass (40 pre-existing + 17 new)
- **New unit tests**: shader binding storage, metadata capture, WGSL generation,
  mixed bindings
- **New GPU tests**: rendering with mixed bindings, shader-only bindings, type
  mismatch rejection, 100K point performance benchmark
- **Performance**: GPU path ~21% faster for 100K point prepare_render (66ms vs
  84ms); re-mapping is near-instant (uniform update only)
