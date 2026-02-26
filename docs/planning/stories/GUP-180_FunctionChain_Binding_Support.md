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

## Retrospective

**Completed**: 2025-07-23

### Key Technical Learnings

#### Self-Contained WGSL Generation for Composed Functions

- **Challenge**: `FunctionChain::generate_wgsl()` only returned the
  `composed_chain()` wrapper function. When injected into a vertex shader via
  `attr_shader()`, the component functions (`linear_scale()`, `color_map()`)
  were undefined, causing WGSL compilation failure.
- **Solution**: Override `generate_wgsl()` to emit all component functions
  first, then the composed wrapper. Each function's WGSL is obtained by calling
  `generate_wgsl()` on the inner functions recursively.
- **Pattern**: When serialising composed structures to a flat representation
  (type-erased `ShaderFnInfo`), ensure all transitive dependencies are included.
  A composed function's WGSL must be self-contained.

#### Type-Mismatched Input/Output in Shader Function Chains

- **Challenge**: A chain like `LinearScale (f32→f32) → ColorMap (f32→vec4)`
  bound to `fill_color` (vec4 field) has input type `f32` but the instance field
  is `vec4<f32>`. The raw `f32` value cannot be stored in a `vec4` field without
  conversion, and the WGSL cannot read `instance.fill_color` (vec4) and pass it
  to a function expecting `f32`.
- **Solution**: Two-sided conversion — `widen_attr_value()` pads the raw value
  to match the field type on the CPU side (e.g., `f32` → `[f32, 0, 0, 0]`), and
  `narrow_field_expr()` generates WGSL that extracts the correct component on
  the GPU side (e.g., `instance.fill_color.x`).
- **Pattern**: When pipeline input and output types differ, handle conversions
  symmetrically: widen at the storage boundary, narrow at the consumption
  boundary.

#### `#[repr(C)]` for GPU-Facing Structs

- **Challenge**: `ChainUniforms<A, B>` lacked `#[repr(C)]`, meaning Rust could
  reorder fields. The WGSL struct definition assumed `first` then `second`
  ordering, which could disagree with the Rust layout.
- **Solution**: Added `#[repr(C)]` to guarantee field ordering matches WGSL.
- **Pattern**: All structs that are uploaded to GPU buffers via `bytemuck` must
  use `#[repr(C)]`. This is a recurring theme (GUP-013 documented similar
  alignment issues).

### Architectural Decisions

#### Type Conversion at Binding Boundaries

- **Decision**: Added `widen_attr_value()` and `narrow_field_expr()` to handle
  type mismatches between shader function input type and instance field type.
- **Reasoning**: The existing design stores raw data in the instance struct's
  existing fields. When a chain's input type is narrower than the field type, we
  need conversion on both the CPU side (widening for storage) and GPU side
  (narrowing for consumption).
- **Trade-off**: Only supports widening to wider types (f32→vec2, f32→vec4,
  vec2→vec4). Arbitrary type conversions are not supported.
- **Future**: If more complex type relationships are needed (e.g., vec3→vec4, or
  custom struct types), additional conversion patterns could be added.

#### Nested Struct Definitions in ShaderUniform

- **Decision**: `ChainUniforms::wgsl_struct_definition()` now includes nested
  uniform struct definitions, making the output self-contained.
- **Reasoning**: The `generate_shader_bound_vertex_wgsl` function emits one
  struct definition per binding. For a chain, a single call to
  `wgsl_struct_definition()` must produce all the types needed.
- **Trade-off**: If two chains share a component type (e.g., both use
  `LinearScaleUniforms`), the struct definition could be emitted twice. The WGSL
  code deduplicates function definitions but not struct definitions. This would
  need a deduplication pass if multiple chain bindings are used simultaneously.
- **Future**: A struct deduplication layer in the WGSL generation could prevent
  redefinition errors when multiple chain bindings share component types.

### Development Workflow Insights

- The story was nominally low-risk ("just needs testing") but actually had three
  distinct bugs that would have caused GPU shader compilation failures. This
  validates the story's premise that explicit testing was needed.
- The `ShaderFnInfo` type-erasure boundary is where most composition bugs
  manifest. The serialisation from generic types to flat strings/bytes loses
  structural information (nested functions, nested uniforms) that must be
  explicitly flattened.
- `narrow_field_expr()` and `widen_attr_value()` are small focused functions
  that make the type conversion logic easy to test and extend.

### Follow-up Stories

1. **GUP-218: Duplicate Struct Definition Prevention** — When multiple shader
   function bindings share component types (e.g., two different chains both
   using `LinearScaleUniforms`), the generated WGSL may contain duplicate struct
   definitions. Add a deduplication pass to `generate_shader_bound_vertex_wgsl`
   that tracks emitted struct names and skips duplicates.

2. **GUP-219: Deep Chain Binding Support** — Currently only two-level chains
   (`A.compose(B)`) are tested. Validate that deeper chains
   (`A.compose(B).compose(C)`) work correctly with `attr_shader()`, including
   nested `ChainUniforms<ChainUniforms<A, B>, C>` serialisation and WGSL
   generation.
