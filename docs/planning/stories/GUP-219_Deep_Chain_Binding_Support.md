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

## Retrospective

**Completed**: 2025-07-11

### Key Technical Learnings

#### Whole-Word Identifier Renaming in WGSL

- **Challenge**: Inner chain identifiers (`ChainUniforms`, `composed_chain`)
  needed to be renamed to depth-suffixed variants (e.g. `ChainUniforms_1`,
  `composed_chain_1`), but a naive string replace of `ChainUniforms` would also
  match `ChainUniforms_1`, breaking deeper nesting.
- **Solution**: Implemented `replace_wgsl_identifier()` that checks identifier
  boundaries — the character before and after the match must not be alphanumeric
  or underscore. This ensures `ChainUniforms` is renamed but `ChainUniforms_1`
  is left untouched.
- **Pattern**: When doing identifier renaming in generated code, always check
  word boundaries. The `is_ident_char()` helper (ASCII alphanumeric or `_`)
  provides a reliable boundary check for WGSL/GLSL identifiers.

#### Duplicate Function Definitions in Composed WGSL

- **Challenge**: When both components of a chain share the same underlying
  function type (e.g. two `LinearScale` instances), `generate_wgsl()`
  concatenated both components' WGSL code, producing duplicate
  `fn linear_scale(...)` definitions. The WGSL compiler rejected the duplicate.
- **Solution**: Added `deduplicate_wgsl_functions()` as a post-processing step
  on the final generated WGSL. It parses function boundaries (by matching
  braces) and keeps only the first occurrence of each function name.
- **Pattern**: When generating code by concatenation of sub-components, always
  consider deduplication. Components may share dependencies that produce
  identical code fragments.

#### Static Return Types vs Dynamic Names

- **Challenge**: `ShaderUniform::wgsl_type_name()` returns `&'static str`,
  preventing dynamic name generation for nested chains. Changing the return type
  to `String` would touch 22+ implementations.
- **Solution**: Kept `wgsl_type_name()` returning `"ChainUniforms"` (static) but
  performed renaming in the `wgsl_struct_definition()` and `generate_wgsl()`
  methods, which already return `String`. The outermost chain always uses the
  plain `ChainUniforms` name (matching `wgsl_type_name()`), while inner chains
  get suffixed. This is consistent because `ShaderFnInfo` uses
  `wgsl_type_name()` for the uniform binding declaration, which always refers to
  the outermost struct.
- **Pattern**: When a trait method has a restrictive return type, work within
  that constraint by handling name resolution in the methods that already return
  dynamic types.

### Architectural Decisions

#### Depth-Based Suffix Naming

- **Decision**: Use `chain_depth()` (computed recursively as `max(A, B) + 1`) as
  the suffix for inner chain identifiers
- **Reasoning**: Depth-based suffixes are deterministic, unique for each nesting
  level, and human-readable in generated WGSL. They naturally compose: a depth-3
  chain produces `ChainUniforms_1`, `ChainUniforms_2`, and `ChainUniforms`.
- **Trade-off**: If a selection has both a shallow chain and a deep chain bound
  to different attributes, the name-based deduplication from GUP-218 could
  incorrectly conflate two `ChainUniforms` structs with different layouts. This
  edge case is documented but unlikely in practice.
- **Future**: If mixed shallow+deep chain attributes become a real use case,
  `wgsl_type_name()` should be changed to return `String` for content-aware
  naming.

#### Function Deduplication in generate_wgsl()

- **Decision**: Apply deduplication at the `FunctionChain::generate_wgsl()`
  level rather than in `generate_shader_bound_vertex_wgsl()`
- **Reasoning**: The vertex shader injection layer already deduplicates by
  `function_name` across bindings, but doesn't inspect the `wgsl_code` string.
  Deduplicating at the generation level ensures each chain produces clean,
  non-redundant WGSL regardless of how it's consumed.
- **Trade-off**: Slight overhead from parsing function boundaries in every
  `generate_wgsl()` call, but this is negligible since WGSL generation is not a
  hot path.
- **Future**: This same deduplication pattern should likely be applied to
  `ParallelComposition` if it ever supports nested compositions.

### Development Workflow Insights

- The implementation was straightforward once the naming strategy was clear. The
  main complexity was reasoning about the static vs dynamic type name constraint
  and ensuring consistency between struct definitions and function signatures.
- Writing the `replace_wgsl_identifier` helper first and testing it in isolation
  simplified the rest of the implementation.
- The GPU test caught a real bug (duplicate function definitions) that the unit
  tests missed because they don't validate WGSL syntax. GPU integration tests
  remain essential for shader code generation features.
- The `chain_depth()` trait method addition was minimal and non-breaking thanks
  to the default value of 0.

### Follow-up Stories

1. **GUP-220: Mixed Shallow+Deep Chain Attribute Deduplication** — When a
   selection binds both a shallow chain (e.g. `LinearScale.compose(ColorMap)`)
   and a deep chain (e.g. `LinearScale.compose(LinearScale).compose(ColorMap)`)
   to different attributes, both produce an outermost `struct ChainUniforms` but
   with different layouts. The GUP-218 name-based deduplication would keep only
   the first, causing an incorrect struct definition for the second binding. Fix
   by either switching to content-aware deduplication or changing
   `wgsl_type_name()` to return `String`.
