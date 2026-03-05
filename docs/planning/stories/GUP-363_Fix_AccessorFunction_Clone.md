# GUP-363: Fix AccessorFunction::clone() to Preserve Closures

## Story Overview

**Initiative**: Chart Builders **Status**: 🚧 In Progress **Created**:
2025-07-27

## Context

`AccessorFunction::clone()` silently loses the closure, returning a field-based
accessor that always produces `AccessorValue::Float(0.0)`. This was discovered
during GUP-362 when the composite builder's override-by-append pattern was found
to be using cloned accessors that returned zero for every data point. While
GUP-362 removed the affected overrides, the broken `Clone` impl remains a
landmine for any code that clones `AccessorFunction`.

## User Story

> "As a library developer, I want `AccessorFunction::clone()` to either
> faithfully reproduce the closure or fail at compile time, so that I never
> silently get wrong data from a cloned accessor."

## Acceptance Criteria

- [ ] `AccessorFunction` either preserves the closure through `Clone` (via
      `Arc`-based sharing) or is made non-Clone.
- [ ] All existing code that clones `AccessorFunction` is updated to use the new
      pattern.
- [ ] No silent data corruption from accessor cloning.
- [ ] All tests pass.

## Technical Tasks

- [ ] Audit all uses of `AccessorFunction::clone()` in the codebase.
- [ ] Choose approach: `Arc`-based closure sharing or remove `Clone`.
- [ ] Implement the chosen approach.
- [ ] Update downstream code that depends on `Clone`.

## Dependencies

### Prerequisite Stories

- GUP-362: Accessor-to-GPU Position Pipeline ✅

## Testing Strategy

- Unit tests verifying that cloned accessors produce the same output as
  originals.
- Integration tests for any code paths that previously cloned accessors.

## Risk Assessment

- **Medium**: Removing `Clone` may require API changes for downstream code. The
  `Arc` approach is more backward-compatible but adds indirection.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied.
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
