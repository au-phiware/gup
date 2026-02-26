# GUP-212: WGSL Reserved Keyword Detection

## Story Overview

**Title**: WGSL Reserved Keyword Detection in Transpiler  
**Epic**: Phase 2 Initiative 3 - Rust-to-WGSL Transpilation Research  
**Priority**: Medium  
**Story Points**: 3  
**Status**: 🚧 In Progress

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

- [ ] The transpiler rejects parameter names that are WGSL reserved keywords
- [ ] Error messages clearly identify the reserved keyword and suggest
      alternatives
- [ ] All WGSL reserved keywords from the spec are covered
- [ ] Existing tests continue to pass
- [ ] New tests validate the keyword detection

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

- [ ] WGSL reserved keyword list implemented
- [ ] Compile-time validation added to `#[shader_fn]` macro
- [ ] Clear error messages with suggestions
- [ ] Tests covering all keyword categories
- [ ] Documentation updated
