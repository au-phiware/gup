# GUP-377: Fix `#[wgsl_function]` Proc Macro `crate::` Path Resolution

## Story Overview

**Initiative**: Developer Experience **Status**: 📋 Planned **Created**:
2025-07-26

## Context

The `#[wgsl_function]` procedural macro in `gup-macros` generates code that
references `crate::shader_function::ComposableShaderFunction`,
`crate::shader_function::ShaderUniform`, and similar paths using the `crate::`
prefix. This works when the macro is invoked from within the `gup` crate itself,
but fails in external crates, doctests, and any context where `crate` does not
refer to `gup`.

This was discovered during GUP-351 (Tutorial Snippet Compilation Tests) where
Tutorial 3's Full Example could not be tested as a doctest. The workaround is to
use `use gup::*;` which brings the `shader_function` module into scope so that
`crate::shader_function::*` resolves via the glob import. However, this
workaround is fragile and non-obvious.

## User Story

> "As a developer using Gup in my own crate, I want `#[wgsl_function]` to work
> without requiring `use gup::*;` so that I can define custom shader functions
> with standard import patterns."

## Acceptance Criteria

- [ ] `#[wgsl_function]` generates code using `::gup::` (or a configurable crate
      path) instead of `crate::` for all trait implementations and type
      references.
- [ ] A `crate` attribute is supported:
      `#[wgsl_function(crate = "my_gup_reexport")]` for crates that re-export
      `gup` under a different name.
- [ ] Existing usage within the `gup` crate continues to work (auto-detect
      `crate` vs `::gup::` based on whether the macro is invoked from within
      `gup`).
- [ ] Tutorial 3's Full Example can be tested as a doctest (not just an
      integration test).
- [ ] The `#[derive(Mark)]` macro is similarly audited and fixed if needed.

## Technical Tasks

- [ ] Audit all generated code paths in `gup-macros/src/wgsl_function.rs` for
      `crate::` references.
- [ ] Replace `crate::` with a configurable path, defaulting to `::gup::`.
- [ ] Add `crate` attribute parsing to the proc macro.
- [ ] Add auto-detection logic: if `CARGO_CRATE_NAME == "gup"`, use `crate::`;
      otherwise use `::gup::`.
- [ ] Update Tutorial 3 Full Example to use `rust,no_run` instead of
      `rust,ignore`.
- [ ] Update `tests/tutorial_snippet_tests.rs` if the integration test is no
      longer needed.

## Dependencies

### Prerequisite Stories

- GUP-351: Tutorial Snippet Compilation Tests ✅ — Identified the issue.

## Testing Strategy

- Verify `#[wgsl_function]` works in doctests without `use gup::*;`.
- Verify it works in external crate contexts.
- Verify existing tests in `tests/wgsl_function_macro_integration.rs` still
  pass.

## Success Metrics

- [ ] Tutorial 3 Full Example compiles as a doctest.
- [ ] `#[wgsl_function]` works with `use gup::prelude::*;` (not just
      `use gup::*;`).

## Risk Assessment

- **Medium**: Changes to proc macro code generation affect all downstream users.
  Careful testing of both internal and external usage is required.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Story status updated to ✅ Complete
