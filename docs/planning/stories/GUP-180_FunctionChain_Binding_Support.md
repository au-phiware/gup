# GUP-180: FunctionChain Binding Support

**Status**: ✅ Complete (2025-07-23)

## Story Overview

**Title**: Validate and fix composed shader function (FunctionChain) bindings
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story
Points**: 3

## Context

GUP-177's `attr_shader()` accepts any `ComposableShaderFunction`. The
`FunctionChain` type composes two shader functions (e.g.,
`linear_scale.compose(color_map)` produces `f32 → Vec4`). While the current
implementation uses `generate_wgsl()` which should handle chains, this has not
been explicitly tested. The `ChainUniforms<A, B>` struct layout and its WGSL
generation may need fixes to work correctly with the `ShaderFnInfo`
serialisation approach.

## User Story

**As a** library user **I want** to bind composed shader function chains to mark
attributes **So that** I can build complex GPU-side transformation pipelines
(e.g., scale → color mapping in a single binding)

## Acceptance Criteria

- [x] `attr_shader()` works with `FunctionChain` types
- [x] Generated WGSL includes both functions and the composed entry point
- [x] Uniform buffer contains the serialised `ChainUniforms<A, B>` data
- [x] GPU integration test renders correctly with a composed function binding
- [x] Type safety: composed output type validated against attribute type

## Dependencies

- **Requires**: GUP-177 (GPU Shader Function Attribute Binding) ✅

## Testing Strategy

- GPU integration test: `linear_scale.compose(color_map)` bound to "fill_color"
- Unit test: verify ShaderFnInfo captures composed function metadata correctly
- Type safety test: composed chain with wrong output type is rejected

## Risk Assessment

- **Medium risk**: `ChainUniforms<A, B>` may not serialise correctly with
  `bytemuck` if the two uniform types have different alignment requirements. The
  WGSL struct definition generation for composed uniforms may need fixes.

## Definition of Done

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

Three bugs in the FunctionChain binding path were discovered and fixed, plus
type-conversion support was added for chains where the input type differs from
the output type (e.g., `f32 → vec4<f32>`):

1. **`#[repr(C)]` on `ChainUniforms`** — Added `#[repr(C)]` to ensure
   deterministic field ordering for GPU memory layout compatibility with WGSL
   struct definitions.

2. **Self-contained WGSL generation for `FunctionChain::generate_wgsl()`** —
   Previously only emitted the `composed_chain()` wrapper function. Now includes
   both component functions' WGSL code so the generated shader is self-contained
   (e.g., `linear_scale()`, `color_map()`, and `composed_chain()` are all
   present).

3. **Nested struct definitions in `ChainUniforms::wgsl_struct_definition()`** —
   Previously only emitted the `ChainUniforms` struct referencing type names
   like `LinearScaleUniforms`. Now includes the full struct definitions for both
   nested types so the WGSL compiles without external dependencies.

4. **Type widening/narrowing for mismatched input/output types** — Added
   `widen_attr_value()` to store narrow raw data (e.g., `f32`) in wider instance
   fields (e.g., `vec4<f32>`) and `narrow_field_expr()` to generate WGSL that
   extracts the correct component (e.g., `instance.fill_color.x` instead of
   `instance.fill_color` when the chain expects `f32`).

### Key Files Changed

- `src/shader_function.rs` — `ChainUniforms` repr, `wgsl_struct_definition()`,
  `FunctionChain::generate_wgsl()`
- `src/selection.rs` — `widen_attr_value()`, `narrow_field_expr()`,
  `generate_shader_bound_vertex_wgsl()`, `prepare_render_shader_bound()`, 6 new
  tests

### Test Summary

- **1555 lib tests** pass (all existing + 6 new)
- **New unit tests**: metadata capture, nested struct defs, WGSL injection,
  chain binding storage
- **New GPU tests**: full pipeline render with
  `linear_scale.compose(color_map)`, type safety rejection for mismatched chain
  output
