# GUP-180: FunctionChain Binding Support

**Status**: 📋 Planned

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

- [ ] `attr_shader()` works with `FunctionChain` types
- [ ] Generated WGSL includes both functions and the composed entry point
- [ ] Uniform buffer contains the serialised `ChainUniforms<A, B>` data
- [ ] GPU integration test renders correctly with a composed function binding
- [ ] Type safety: composed output type validated against attribute type

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

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
