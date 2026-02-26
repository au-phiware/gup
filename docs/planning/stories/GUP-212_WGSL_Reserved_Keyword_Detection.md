# GUP-212: WGSL Reserved Keyword Detection

## Story Overview

**Title**: WGSL Reserved Keyword Detection in Transpiler  
**Epic**: Phase 2 Initiative 3 - Rust-to-WGSL Transpilation Research  
**Priority**: Medium  
**Story Points**: 3  
**Status**: ✅ Complete (2025-07-26)

## Context

During GUP-062 validation, it was discovered that the `#[shader_fn]` transpiler
allows Rust parameter names that are WGSL reserved keywords (e.g., `target`,
`sample`, `texture`). This causes GPU shader compilation failures at runtime
rather than catching the error at compile time.

## User Story

**As a** developer using `#[shader_fn]`  
**I want** clear compile-time errors when I use WGSL reserved keywords as
parameter names  
**So that** I don't encounter confusing GPU compilation failures at runtime

## Acceptance Criteria

- [x] The transpiler rejects parameter names that are WGSL reserved keywords
- [x] Error messages clearly identify the reserved keyword and suggest
      alternatives
- [x] All WGSL reserved keywords from the spec are covered
- [x] Existing tests continue to pass
- [x] New tests validate the keyword detection

## Technical Tasks

1. Add a list of WGSL reserved keywords to the transpiler
2. Validate function parameter names against the keyword list during macro
   expansion
3. Generate clear `syn::Error` messages with suggestions
4. Add tests for each category of reserved keyword

## Dependencies

- GUP-061: Integration with Shader Function System (complete)

## Testing Strategy

- Unit tests for keyword detection in `gup-macros/src/shader_fn.rs`
- Integration tests verifying compile-time errors for reserved keyword usage

## Success Metrics

- 100% of WGSL reserved keywords detected at compile time
- Zero false positives on valid parameter names
- Error messages include suggested alternatives

## Risk Assessment

- **Low risk**: Well-scoped enhancement to existing code
- **No breaking changes**: Only adds new validation, doesn't change existing
  behaviour

## Definition of Done

- [x] WGSL reserved keyword list implemented
- [x] Compile-time validation added to `#[shader_fn]` macro
- [x] Clear error messages with suggestions
- [x] Tests covering all keyword categories
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

A comprehensive WGSL reserved keyword detection system that validates
identifiers at compile time in both `#[shader_fn]` and `#[wgsl_function]`
macros.

### Key Files Changed

- **`gup-macros/src/wgsl_keywords.rs`** (new) — Shared keyword module with:
  - Four keyword categories: Reserved, Contextual, Built-in Types, Future
    Reserved
  - ~160 keywords from the WGSL specification
  - `validate_param_name()` and `validate_function_name()` functions
  - Alternative name suggestions (appending `_val` suffix)
- **`gup-macros/src/shader_fn.rs`** — Integrated keyword validation for function
  names and all parameter names (input + uniform)
- **`gup-macros/src/wgsl_function.rs`** — Integrated keyword validation for
  function names and all parameter names (input + uniform)
- **`gup-macros/src/lib.rs`** — Registered the new `wgsl_keywords` module

### Test Counts

- 16 unit tests in `wgsl_keywords` module (keyword detection, classification,
  suggestions, validation)
- 6 integration tests in `shader_fn` (reserved keyword function name, input
  param, uniform param, contextual keyword, future reserved, valid names)
- 6 integration tests in `wgsl_function` (same categories through full macro
  parsing)
- All 1,379+ existing unit tests continue to pass
