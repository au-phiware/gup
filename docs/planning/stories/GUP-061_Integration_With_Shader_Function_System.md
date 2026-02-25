# GUP-061: Integration with Existing Shader Function System

**Status**: 🚧 In Progress

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

- [ ] A function defined with the new Rust-syntax transpiler implements
      `ShaderFunction` (same trait as `#[wgsl_function]`)
- [ ] `function_name()`, `wgsl_function()`, and `create_uniforms()` all work
      correctly for transpiled functions
- [ ] Transpiled functions can be passed to `ShaderPipeline::add_function()`

### AC2: Mixed Pipeline Support

- [ ] A `ShaderPipeline` can hold both string-based and transpiled functions in
      the same pipeline
- [ ] The generated WGSL from a mixed pipeline compiles successfully on the GPU
- [ ] No performance regression vs. all-string pipelines

### AC3: Backward Compatibility

- [ ] All existing `#[wgsl_function]` usages compile without modification
- [ ] All existing tests pass without changes
- [ ] The transpiler is an opt-in addition, not a forced migration

### AC4: Migration Guide

- [ ] Document the equivalence between `#[wgsl_function]` and new Rust-syntax
      approach
- [ ] Provide before/after examples for common function patterns
- [ ] Note any features supported by one approach but not the other

## Technical Requirements

- Builds on GUP-057 (expression transpilation) and GUP-058 (control flow)
- Must not require changes to `ShaderPipeline`, `MarkRenderer`, or `Selection`
- Use a wrapper type or blanket impl to satisfy `ShaderFunction` for transpiled
  results, not a new trait
- Alignment with existing WGSL type system (GUP-056)

## Dependencies

- **Requires**: GUP-056 (Type System Mapping) 📋
- **Requires**: GUP-057 (Expression Transpilation) 📋
- **Requires**: GUP-058 (Control Flow Handling) 📋
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

- [ ] AC1–AC4 acceptance criteria checked off
- [ ] `mask test` passes (all existing tests green)
- [ ] `mask all-fix` clean
- [ ] Retrospective written

---

_Identified during Phase 2 Rust-to-WGSL Transpilation initiative planning._
_Created 2026-02-25._
