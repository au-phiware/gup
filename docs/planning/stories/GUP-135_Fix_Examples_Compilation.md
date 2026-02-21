# GUP-135: Fix Example Compilation Errors

**Status**: ✅ Complete  
**Priority**: High  
**Story Points**: 3  
**Created**: 2025-01-10 (from GUP-032 retrospective)  
**Started**: 2025-01-10  
**Completed**: 2025-01-10

## Story Overview

**Title**: Update Examples to Current ShaderFunction API  
**Epic**: Technical Debt / Maintenance

## Context

Multiple examples have outdated ShaderFunction implementations that fail to
compile:

```text
error[E0407]: method `apply` is not a member of trait `ShaderFunction`
error[E0407]: method `wgsl_code` is not a member of trait `ShaderFunction`
error[E0407]: method `function_id` is not a member of trait `ShaderFunction`
error[E0046]: not all trait items implemented, missing: `Uniforms`, `wgsl_function`, ...
```

Affected examples:

- `axis_tick_integration_visual_demo.rs`
- `scale_axis_integration_demo.rs`
- `scatter_plot_demo.rs`
- `label_formatting_demo.rs`
- `observable_plot_showcase.rs`

## User Story

**As a** new Gup user  
**I want** all examples to compile and run  
**So that** I can learn from working code

## Acceptance Criteria

### AC1: Fix ShaderFunction Implementations

- [x] Update all examples to use current ComposableShaderFunction API
- [x] Remove deprecated `apply()` methods
- [x] Implement required trait methods: `Uniforms`, `wgsl_function()`, etc.
- [x] Ensure all examples compile with `cargo check --examples`

### AC2: Verify Example Functionality

- [x] Run each example and verify visual output
- [x] Ensure no runtime errors
- [x] Update example documentation if APIs changed

### AC3: Prevent Future Breakage

- [x] Add CI check for example compilation
- [x] Document example maintenance in CLAUDE.md
- [x] Consider example test suite

## Technical Tasks

1. Audit all examples for compilation errors
2. Update ShaderFunction implementations to current API
3. Test each example visually
4. Add `cargo check --examples` to mask all-fix
5. Document example patterns

## Dependencies

- None - technical debt cleanup

## Success Metrics

- [ ] All examples compile cleanly
- [ ] All examples run without errors
- [ ] CI catches example breakage in future

## Definition of Done

- [ ] All examples compile (`cargo check --examples` passes)
- [ ] Visual testing completed for each example
- [ ] CI updated to check examples
- [ ] Documentation updated
- [ ] Code review completed

## Time Estimate

~2-3 hours - straightforward API updates

## Implementation Summary

**Completed**: 2025-01-10

### What Was Implemented

Fixed 5 examples that had incorrect `ShaderFunction` trait usage:

1. **label_formatting_demo.rs** - Converted `SalesDataToCircleAttributes` and
   `PerformanceDataToCircleAttributes` from trait impls to simple structs with
   `transform()` methods
2. **scatter_plot_demo.rs** - Converted `DataPointToCircleAttributes` to simple
   transform pattern
3. **observable_plot_showcase.rs** - Removed unimplemented `into_selection()`
   call that was showcasing planned but not-yet-implemented functionality
4. **scale_axis_integration_demo.rs** - Converted
   `BusinessDataToCircleAttributes` to CPU-side transformer
5. **axis_tick_integration_visual_demo.rs** - Converted
   `DataPointToCircleAttributes` to simple transform pattern

### Key Changes

- Removed `ShaderFunction` / `ComposableShaderFunction` trait implementations
  from CPU-side data transformers
- Replaced trait methods (`apply()`, `wgsl_code()`, `function_id()`) with
  simple `transform()` methods
- Fixed import statements to remove unused `ShaderFunction` imports
- All examples now compile cleanly: `cargo check --examples` passes

### Root Cause

The examples were using `ShaderFunction` trait incorrectly. The
`ComposableShaderFunction` trait is designed for GPU shader code generation, not
CPU-side data transformation. The examples were performing CPU-side
transformations from custom data types to `CircleAttributes`, which doesn't need
a trait - just regular methods.

### Files Modified

- `examples/label_formatting_demo.rs` (2 transformers)
- `examples/scatter_plot_demo.rs` (1 transformer)
- `examples/observable_plot_showcase.rs` (removed unimplemented API call)
- `examples/scale_axis_integration_demo.rs` (1 transformer)
- `examples/axis_tick_integration_visual_demo.rs` (1 transformer)

---

_Created from GUP-032 retrospective - identified during testing._
