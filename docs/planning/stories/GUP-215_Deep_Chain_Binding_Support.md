# GUP-215: Deep Chain Binding Support

**Status**: 📋 Planned

## Story Overview

**Title**: Validate deep function chain bindings with attr_shader() **Epic**:
Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**:
3

## Context

GUP-180 validated two-level function chains (`A.compose(B)`) with
`attr_shader()`. Deeper chains (`A.compose(B).compose(C)`) produce nested
`ChainUniforms<ChainUniforms<A, B>, C>` types. The recursive
`wgsl_struct_definition()` and `generate_wgsl()` methods should handle this, but
the nested `ChainUniforms` struct name is always `"ChainUniforms"`, which would
cause name collisions when the inner and outer chain both generate a struct with
the same name.

## User Story

**As a** library user **I want** to compose three or more shader functions into
a deep chain and bind it to an attribute **So that** I can build multi-stage GPU
transformation pipelines

## Acceptance Criteria

- [ ] Three-function chain (`A.compose(B).compose(C)`) works with
      `attr_shader()`
- [ ] Nested `ChainUniforms` struct names are unique or properly handled
- [ ] WGSL generation includes all component functions at every level
- [ ] GPU integration test renders correctly with a deep chain
- [ ] `ChainUniforms<ChainUniforms<A, B>, C>` serialises correctly via bytemuck

## Dependencies

- **Requires**: GUP-180 (FunctionChain Binding Support) ✅
- **Recommended**: GUP-214 (Duplicate Struct Definition Prevention)

## Testing Strategy

- Unit test: `ShaderFnInfo` for a 3-level chain
- GPU integration test: render with a 3-level chain binding
- Alignment test: verify `ChainUniforms` nesting preserves correct byte layout

## Risk Assessment

- **Medium risk**: The `ChainUniforms` WGSL type name is always
  `"ChainUniforms"`. Nested chains would generate two structs with the same name
  but different layouts, causing WGSL compilation failure. May need unique type
  names per nesting level.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
