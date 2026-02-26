# GUP-220: Mixed Shallow+Deep Chain Attribute Deduplication

**Status**: 📋 Planned

## Story Overview

**Title**: Fix struct deduplication for mixed shallow and deep chain attribute
bindings **Epic**: Phase 1 Initiative 4 - Advanced Data Mapping **Priority**:
Low **Story Points**: 3

## Context

GUP-218 introduced name-based WGSL struct deduplication: when multiple
`attr_shader()` bindings produce structs with the same name, only the first
definition is emitted. GUP-219 added depth-based suffixes to disambiguate nested
`ChainUniforms` structs within a single deep chain. However, if two _different_
chains (one shallow, one deep) are bound to separate attributes on the same
selection, both produce an outermost `struct ChainUniforms { ... }` but with
**different field layouts**. The name-based deduplication keeps only the first
definition, causing the second binding's uniform type to reference the wrong
struct.

## User Story

**As a** library user **I want** to bind both a two-function chain and a
three-function chain to different attributes on the same selection **So that**
each chain's GPU uniform struct is correctly defined

## Acceptance Criteria

- [ ] A selection with both `attr_shader("radius", ..., scale1.compose(scale2))`
      and
      `attr_shader("fill_color", ..., scale1.compose(scale2).compose(color_map))`
      produces correct, non-conflicting WGSL struct definitions
- [ ] Name-based deduplication is replaced or augmented with content-aware
      deduplication (or `ChainUniforms` names are made globally unique)
- [ ] Existing GUP-218 deduplication of truly identical structs still works
- [ ] GPU integration test renders correctly with mixed chain bindings

## Dependencies

- **Requires**: GUP-219 (Deep Chain Binding Support) ✅
- **Requires**: GUP-218 (Duplicate Struct Definition Prevention) ✅

## Technical Tasks

1. Evaluate two approaches:
   - **Option A**: Change `ShaderUniform::wgsl_type_name()` to return `String`
     (enables each `ChainUniforms<A, B>` to have a unique name based on
     component types). Requires updating ~22 implementations.
   - **Option B**: Augment name-based deduplication with layout comparison — if
     two structs share a name but have different field definitions, assign a
     unique suffix to one. Keeps the trait signature stable.
2. Implement the chosen approach
3. Update `shader_fn_info_from()` if `uniform_type_name` derivation changes
4. Add tests for mixed shallow+deep chains on a single selection

## Testing Strategy

- Unit test: Two bindings with same-named but different-layout ChainUniforms
- GPU integration test: Render with mixed chain attribute bindings
- Regression test: Existing identical-layout deduplication still works

## Risk Assessment

- **Low risk**: This is an edge case unlikely to be hit in typical usage. Both
  approaches are well-understood; the main decision is API stability vs
  implementation simplicity.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection and chain tests still pass
- [ ] `mask all-fix` clean
