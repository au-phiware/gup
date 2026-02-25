# GUP-061: Integration with Existing Shader Function System

**Status**: ✅ Complete (2025-07-27)

## Story Overview

**Title**: Bridge Rust-to-WGSL Transpiler with Existing ShaderFunction
Infrastructure **Epic**: Phase 2 - Rust-to-WGSL Transpilation System
**Priority**: High **Story Points**: 8

## Context

The Rust-to-WGSL transpilation system (GUP-054 through GUP-060) builds a new
path for writing shader functions in Rust syntax. However, the rest of the
codebase — mark rendering, shader pipelines, the `#[wgsl_function]` macro — all
rely on the existing `ShaderFunction` trait and `ShaderPipeline` infrastructure.

This story bridges the two: transpiled Rust functions must be usable anywhere a
`ShaderFunction` is accepted, with no breaking changes to existing code that
already uses string-embedded WGSL.

## User Story

**As a** library developer **I want** to write a shader function in Rust syntax
and use it alongside existing `#[wgsl_function]` functions without changing any
calling code **So that** I can migrate incrementally at my own pace

## Acceptance Criteria

### AC1: Transpiled Functions Implement ShaderFunction

- [x] A function defined with the new Rust-syntax transpiler implements
      `ShaderFunction` (same trait as `#[wgsl_function]`)
- [x] `function_name()`, `wgsl_function()`, and `create_uniforms()` all work
      correctly for transpiled functions
- [x] Transpiled functions can be passed to `ShaderPipeline::add_function()`

### AC2: Mixed Pipeline Support

- [x] A `ShaderPipeline` can hold both string-based and transpiled functions in
      the same pipeline
- [x] The generated WGSL from a mixed pipeline compiles successfully on the GPU
- [x] No performance regression vs. all-string pipelines

### AC3: Backward Compatibility

- [x] All existing `#[wgsl_function]` usages compile without modification
- [x] All existing tests pass without changes
- [x] The transpiler is an opt-in addition, not a forced migration

### AC4: Migration Guide

- [x] Document the equivalence between `#[wgsl_function]` and new Rust-syntax
      approach
- [x] Provide before/after examples for common function patterns
- [x] Note any features supported by one approach but not the other

## Technical Requirements

- Builds on GUP-057 (expression transpilation) and GUP-058 (control flow)
- Must not require changes to `ShaderPipeline`, `MarkRenderer`, or `Selection`
- Use a wrapper type or blanket impl to satisfy `ShaderFunction` for transpiled
  results, not a new trait
- Alignment with existing WGSL type system (GUP-056)

## Dependencies

- **Requires**: GUP-056 (Type System Mapping) ✅
- **Requires**: GUP-057 (Expression Transpilation) ✅
- **Requires**: GUP-058 (Control Flow Handling) ✅
- **Requires**: GUP-005 (Shader Function Trait) ✅
- **Requires**: GUP-051 (WGSL Code Generation Templates) ✅
- **Blocks**: GUP-062 (Community Validation) — needs working end-to-end path

## Testing Strategy

- Unit test: transpile a simple function and verify `wgsl_function()` output
  matches expected WGSL
- Integration test: build a `ShaderPipeline` mixing one transpiled and one
  `#[wgsl_function]` function; validate GPU compilation succeeds
- Regression: all existing shader function tests still pass

## Risk Assessment

**Medium Risk**: The `ShaderFunction` trait may need minor adjustments to
accommodate transpiled metadata (e.g., uniform type information). This must be
done without breaking existing macro-generated impls.

**Mitigation**: Implement as a newtype wrapper `TranspiledFn<T>` that implements
`ShaderFunction` by delegating to the transpilation output. If the trait needs
extension, add optional methods with default implementations.

## Definition of Done

- [x] AC1–AC4 acceptance criteria checked off
- [x] `mask test` passes (all existing tests green)
- [x] `mask all-fix` clean
- [x] Retrospective written

## Implementation Summary

### What Was Implemented

A `#[shader_fn]` proc macro attribute that transpiles Rust function bodies to
WGSL using the existing transpilation pipeline (`RustToWgsl` + `WgslCodeGen`)
and generates the same output as `#[wgsl_function]`: a configuration struct, a
GPU uniform struct, and a `ComposableShaderFunction` trait implementation.

### Key Files Changed

| File                                | Change                                         |
| ----------------------------------- | ---------------------------------------------- |
| `gup-macros/src/shader_fn.rs`       | New — `#[shader_fn]` proc macro implementation |
| `gup-macros/src/lib.rs`             | Added `shader_fn` proc macro entry point       |
| `src/lib.rs`                        | Updated macro documentation comments           |
| `tests/shader_fn_integration.rs`    | New — 21 integration tests (AC1–AC3)           |
| `docs/SHADER_FN_MIGRATION_GUIDE.md` | New — migration guide (AC4)                    |

### Test Counts

- 7 unit tests in `gup-macros/src/shader_fn.rs`
- 21 integration tests in `tests/shader_fn_integration.rs`
  - 14 trait implementation tests (AC1)
  - 5 GPU compilation validation tests
  - 2 pipeline integration tests (AC2)
- All 1379+ existing tests continue to pass (AC3)

---

_Identified during Phase 2 Rust-to-WGSL Transpilation initiative planning._
_Created 2026-02-25._

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Proc Macro Code Generation Reuse

- **Challenge**: The story suggested a "wrapper type or blanket impl" approach,
  but the transpiler lives in the proc macro crate — making runtime wrappers
  awkward because the transpilation happens at compile time.
- **Solution**: Created a `#[shader_fn]` proc macro that reuses the same code
  generation path as `#[wgsl_function]` (`WgslFunctionInfo::to_tokens()`). The
  transpiler replaces only the WGSL body generation step; all struct generation,
  uniform handling, and trait impl generation are shared.
- **Pattern**: When bridging compile-time systems, prefer generating the same
  output type rather than wrapping different outputs. This gives zero-cost
  interoperability.

#### `crate::` Resolution in Integration Tests

- **Challenge**: Macro-generated code references `crate::shader_function::...`.
  In integration tests (`tests/` directory), `crate` refers to the test binary,
  not `gup`. Tests fail with "unresolved import" unless the module is brought
  into scope.
- **Solution**: Tests must include `use gup::shader_function::{self, ...};` so
  that `crate::shader_function` resolves via the glob import at crate root
  level. This matches the pattern used by existing `#[wgsl_function]` tests.
- **Pattern**: Always check macro expansion context. `crate::` in generated code
  works within the defining crate but requires explicit module imports in
  external crate contexts.

### Architectural Decisions

#### Single Macro, Shared Code Generation

- **Decision**: Implemented `#[shader_fn]` as a thin wrapper around existing
  `WgslFunctionInfo::to_tokens()`, differing only in how the WGSL body is
  produced (transpiler vs manual string translation).
- **Reasoning**: Ensures both approaches generate identical output structures.
  No changes needed to `ShaderPipeline`, `MarkRenderer`, or `Selection`.
- **Trade-off**: Tightly couples `shader_fn.rs` to `wgsl_function.rs` internals
  (uses `WgslFunctionInfo` and `UniformParam` directly). Changes to the code
  generation in `wgsl_function.rs` will affect `#[shader_fn]` output.
- **Future**: If the two macros diverge significantly, consider extracting the
  shared code generation into a separate module.

#### No Changes to Core Traits

- **Decision**: `ComposableShaderFunction` trait and `ShaderPipeline` remained
  completely unchanged.
- **Reasoning**: The transpiler produces the same output (WGSL string + struct
  metadata) as the manual approach. No trait extensions were needed.
- **Trade-off**: None — this was the ideal outcome.
- **Future**: Validates that the trait design is sufficiently abstract to
  accommodate new code generation backends.

### Development Workflow Insights

- The story was significantly simpler than estimated (8 story points). The
  existing transpiler infrastructure was well-designed and the code generation
  path in `wgsl_function.rs` was cleanly separable from the WGSL body
  generation. Total implementation was ~280 lines of new Rust + ~240 lines of
  tests + ~305 lines of documentation.
- Running `mask all-fix` consistently caught formatting issues before they could
  become problems.
- GPU validation tests (compiling transpiled WGSL on the actual GPU) provided
  high confidence that the transpiler output is correct, beyond just string
  matching.

### Follow-up Stories

No new follow-up stories were identified. The existing GUP-062 (Community
Validation and Proof-of-Concept) is now unblocked and should demonstrate the
end-to-end path from Rust function to GPU execution.
