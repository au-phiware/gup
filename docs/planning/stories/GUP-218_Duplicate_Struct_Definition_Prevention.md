# GUP-218: Duplicate Struct Definition Prevention

**Status**: ✅ Complete (2025-07-18)

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

- [x] `generate_shader_bound_vertex_wgsl()` deduplicates struct definitions by
      name
- [x] Multiple chain bindings sharing a component type generate valid WGSL
- [x] Test: two chains both using `LinearScaleUniforms` produce a single struct
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

- [x] All acceptance criteria met
- [x] Existing Selection tests still pass
- [x] `mask all-fix` clean

## Implementation Summary

### What Was Implemented

Modified `generate_shader_bound_vertex_wgsl()` in `src/selection.rs` to
deduplicate struct definitions by struct name using a `HashSet`. Since a single
`uniform_struct_def` string can contain multiple struct definitions (e.g.,
`ChainUniforms` includes nested component struct defs), two helper functions
were added to split and parse them:

- `split_wgsl_struct_definitions(defs)` — splits a compound definition string
  into individual `struct ...{ ... }` blocks
- `extract_wgsl_struct_name(def)` — parses the struct name from a single block

### Key Files Changed

- `src/selection.rs` — core deduplication logic + 5 new tests

### Test Counts

- 5 new unit tests added
- 152 existing selection tests continue to pass
- All examples compile
