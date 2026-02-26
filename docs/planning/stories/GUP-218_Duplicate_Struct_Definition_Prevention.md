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

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### WGSL Struct Definition Format

- **Challenge**: The `uniform_struct_def` field is a single string that can
  contain multiple struct definitions concatenated together (from
  `ChainUniforms::wgsl_struct_definition()` which recursively includes nested
  component struct defs).
- **Solution**: Split the compound string on `struct` keyword boundaries,
  extract each struct name, and deduplicate using a `HashSet<String>`.
- **Pattern**: When deduplicating generated code, work at the semantic level
  (struct names) rather than raw string equality, since struct definitions from
  different sources may have different whitespace but the same name.

#### Separation of Struct Emission from Binding Declarations

- **Challenge**: The original code interleaved struct definitions with
  `@group/@binding` declarations in a single loop per binding.
- **Solution**: Separated into two passes: first collect and deduplicate all
  struct definitions, then emit all binding declarations. This is cleaner and
  ensures structs are defined before they're referenced.
- **Pattern**: When injecting generated code, separate declaration/definition
  phases from usage phases.

### Architectural Decisions

#### String-Level Deduplication vs Trait-Level

- **Decision**: Deduplicate at the string level in
  `generate_shader_bound_vertex_wgsl()` rather than modifying `ShaderUniform` to
  avoid emitting nested defs.
- **Reasoning**: Keeps the fix localised to the code generation function.
  Modifying `ChainUniforms::wgsl_struct_definition()` would be a larger change
  and could break the self-contained property that each uniform type's
  definition is independently valid.
- **Trade-off**: String parsing is slightly fragile (relies on the `struct`
  keyword prefix and `}` delimiter), but WGSL struct syntax is well-defined and
  unlikely to change.
- **Future**: If the project moves to AST-based WGSL generation (GUP-189 style),
  deduplication would happen naturally at the AST node level.

### Development Workflow Insights

- The story was straightforward — a 2-point story that delivered in a single
  focused increment. The existing test infrastructure for `ShaderFnInfo` and
  `generate_shader_bound_vertex_wgsl` made it easy to write targeted tests.
- The pre-existing `test_registry_scalability` failure in
  `mark_pipeline_performance_tests` is unrelated to this change (confirmed by
  running it on the base commit).
