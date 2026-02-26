# GUP-214: Duplicate Struct Definition Prevention

**Status**: 📋 Planned

## Story Overview

**Title**: Prevent duplicate struct definitions in generated WGSL **Epic**:
Phase 1 Initiative 4 - Advanced Data Mapping **Priority**: Low **Story Points**:
2

## Context

When multiple shader function bindings share component types (e.g., two
different `FunctionChain`s both using `LinearScaleUniforms`), the generated WGSL
from `generate_shader_bound_vertex_wgsl()` may contain duplicate struct
definitions. WGSL does not allow struct redefinition, so this would cause shader
compilation failures.

Currently, function code is deduplicated by function name (via a `HashSet`), but
struct definitions are not deduplicated. The
`ChainUniforms::wgsl_struct_definition()` method includes nested struct
definitions, which can collide when multiple chain bindings use the same
component uniform type.

## User Story

**As a** library user **I want** to bind multiple shader function chains that
share component types **So that** the generated WGSL compiles correctly without
duplicate struct definitions

## Acceptance Criteria

- [ ] `generate_shader_bound_vertex_wgsl()` deduplicates struct definitions by
      name
- [ ] Multiple chain bindings sharing a component type generate valid WGSL
- [ ] Test: two chains both using `LinearScaleUniforms` produce a single struct
      definition

## Dependencies

- **Requires**: GUP-180 (FunctionChain Binding Support) ✅

## Testing Strategy

- Unit test: two chain bindings with shared uniform types
- GPU integration test: render with multiple chain bindings

## Risk Assessment

- **Low risk**: Straightforward string deduplication. May need to parse struct
  names from the definition strings.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] Existing Selection tests still pass
- [ ] `mask all-fix` clean
