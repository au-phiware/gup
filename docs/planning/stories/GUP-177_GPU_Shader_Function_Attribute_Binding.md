# GUP-177: GPU-Side Shader Function Attribute Binding

**Status**: 🚧 In Progress

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

- [ ] `attr()` accepts `ComposableShaderFunction` types in addition to closures
- [ ] Shader function bindings generate WGSL code for attribute transformation
- [ ] Data is uploaded in raw form; transformation happens in the vertex shader
- [ ] Mixed CPU closure + GPU shader function bindings work together
- [ ] Performance improvement demonstrated for 100K+ point datasets
- [ ] Type safety: shader function output types must match mark attribute types

## Dependencies

- **Requires**: GUP-168 (Selection Attribute Binding Pipeline) ✅
- **Requires**: GUP-005 (Shader Function System) ✅

## Testing Strategy

- Unit tests for shader function binding storage
- GPU integration test: render with shader function attribute bindings
- Performance benchmark: CPU closure vs GPU shader function for 100K+ points
- Type safety tests for mismatched shader function output types

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
- [ ] Performance benchmark included
