# GUP-135: Fix Example Compilation Errors

**Status**: 📋 Planned  
**Priority**: High  
**Story Points**: 3  
**Created**: 2025-01-10 (from GUP-032 retrospective)

## Story Overview

**Title**: Update Examples to Current ShaderFunction API  
**Epic**: Technical Debt / Maintenance  

## Context

Multiple examples have outdated ShaderFunction implementations that fail to compile:

```
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

- [ ] Update all examples to use current ComposableShaderFunction API
- [ ] Remove deprecated `apply()` methods
- [ ] Implement required trait methods: `Uniforms`, `wgsl_function()`, etc.
- [ ] Ensure all examples compile with `cargo check --examples`

### AC2: Verify Example Functionality

- [ ] Run each example and verify visual output
- [ ] Ensure no runtime errors
- [ ] Update example documentation if APIs changed

### AC3: Prevent Future Breakage

- [ ] Add CI check for example compilation
- [ ] Document example maintenance in CLAUDE.md
- [ ] Consider example test suite

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

---

_Created from GUP-032 retrospective - identified during testing._
