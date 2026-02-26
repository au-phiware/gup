# GUP-219: Deep Chain Binding Support

**Status**: ✅ Complete (2025-07-11)

## Story Overview

**Title**: Validate deep function chain bindings with attr_shader() **Epic**:
Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**:
3

## Implementation Summary

### What Was Implemented

1. **`chain_depth()` method on `ShaderUniform` trait** — returns nesting depth
   for chain uniform types (default 0, increments for each `ChainUniforms`
   nesting level)
2. **`replace_wgsl_identifier()` helper** — whole-word replacement in WGSL code,
   respecting identifier boundaries (won't rename `ChainUniforms_1` when
   targeting `ChainUniforms`)
3. **`deduplicate_wgsl_functions()` helper** — removes duplicate function
   definitions from generated WGSL (handles cases like two `LinearScale`
   instances in a chain producing duplicate `fn linear_scale(...)` blocks)
4. **Updated `ChainUniforms::wgsl_struct_definition()`** — inner chain structs
   are renamed with depth suffix (e.g. `ChainUniforms_1`) to avoid WGSL name
   collisions
5. **Updated `FunctionChain::generate_wgsl()`** — inner chain function names are
   similarly renamed (e.g. `composed_chain_1`), with function deduplication
   applied

### Key Files Changed

- `src/shader_function.rs` — core trait extension, helpers, ChainUniforms and
  FunctionChain updates
- `src/selection.rs` — deep chain test suite

### Test Counts

- 7 new unit tests in `shader_function::tests` (struct names, depth values, WGSL
  generation, bytemuck layout, create_uniforms, replace_wgsl_identifier)
- 4 new tests in `selection::tests` (ShaderFnInfo metadata, renamed inner
  structs, WGSL injection, GPU integration render)
- All 1574 existing tests continue to pass

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

- [x] Three-function chain (`A.compose(B).compose(C)`) works with
      `attr_shader()`
- [x] Nested `ChainUniforms` struct names are unique or properly handled
- [x] WGSL generation includes all component functions at every level
- [x] GPU integration test renders correctly with a deep chain
- [x] `ChainUniforms<ChainUniforms<A, B>, C>` serialises correctly via bytemuck

## Dependencies

- **Requires**: GUP-180 (FunctionChain Binding Support) ✅
- **Recommended**: GUP-218 (Duplicate Struct Definition Prevention)

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

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean
