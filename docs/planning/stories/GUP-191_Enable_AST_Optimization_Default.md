# GUP-191: Enable AST Optimization by Default

**Story ID**: GUP-191 **Title**: Enable AST Optimization by Default **Status**:
✅ Complete **Completed**: 2025-07-18 **Priority**: Low **Effort**: 3 story
points **Created**: 2025-08-08 **Dependencies**: GUP-189, GUP-190

## Overview

Once the WGSL parser in `shader_ast` can handle the full range of generated
shader constructs — struct definitions, `var<uniform>` bindings, attribute
decorators like `@group(0) @binding(0)`, etc. — flip the
`OptimizationConfig.use_ast_analysis` default to `true` and deprecate the
string-based optimization path.

## Context

GUP-189 integrated the AST optimizer into `ComposableShaderPipeline` but kept it
opt-in (`use_ast_analysis: false` by default) because the AST parser handles
only a subset of WGSL. As the parser's coverage grows (especially via GUP-190
which adds compute shader support), this story flips the default so all users
benefit from AST-based optimization automatically.

## User Story

As a developer using the shader pipeline, I want AST-based optimization to be
the default so that I get the best optimizations without needing to opt in.

## Acceptance Criteria

- [x] `OptimizationConfig::default()` sets `use_ast_analysis` to `true`
- [x] All existing tests pass with the new default
- [x] The AST parser can handle all WGSL output from `generate_vertex_shader()`
      and `generate_fragment_shader()`
- [x] String-based optimization methods are marked `#[deprecated]`
- [x] Documentation updated to reflect the new default

## Technical Tasks

1. Extend `shader_ast::parser` to handle `@group`/`@binding` attributes,
   `var<uniform>` declarations, and struct field attributes.
2. Flip `use_ast_analysis` default to `true`.
3. Mark `remove_unused_uniforms`, `inline_small_functions_advanced`, and
   `fold_constants` as deprecated.
4. Update doc comments and examples.

## Dependencies

- GUP-189: AST Integration with ComposableShaderPipeline (provides the
  integration bridge)
- GUP-190: WGSL Compute Shader AST Support (extends parser coverage)

## Testing Strategy

- Run all existing tests with the new default.
- Verify that generated pipeline shaders round-trip through the AST parser.
- Performance benchmarks to confirm no regression.

## Success Metrics

- Zero fallbacks to string-based path during normal pipeline operation.
- All tests pass with `use_ast_analysis: true` as default.

## Risk Assessment

- **Low risk**: Fallback mechanism from GUP-189 provides safety net.

## Definition of Done

- [x] Implementation complete with tests passing
- [x] `mask all-fix` clean
- [x] All examples compile
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

- Added `return_attributes` field to `Function` AST node to preserve
  `@location(N)` on function return types during parse→generate roundtrips
- Updated the WGSL parser to store return type attributes (previously discarded)
- Updated the WGSL generator to emit return type attributes
- Flipped `OptimizationConfig::default()` to set `use_ast_analysis: true`
- Deprecated four string-based optimization methods with `#[deprecated]`:
  `remove_unused_uniforms`, `inline_small_functions_advanced`, `fold_constants`,
  `propagate_constants`
- Updated doc comments on `use_ast_analysis` to reflect the new default
- Added roundtrip tests verifying generated shaders parse through the AST

### Key Files Changed

| File                                         | Change                                               |
| -------------------------------------------- | ---------------------------------------------------- |
| `src/shader_ast/types.rs`                    | Added `return_attributes` field to `Function`        |
| `src/shader_ast/parser.rs`                   | Store return type attributes instead of discarding   |
| `src/shader_ast/generator.rs`                | Emit return type attributes in generated WGSL        |
| `src/shader_ast/optimizer.rs`                | Added `return_attributes: vec![]` to struct literals |
| `src/shader_ast/benchmarks.rs`               | Added `return_attributes: vec![]` to struct literals |
| `src/shader_ast/type_check.rs`               | Added `return_attributes: vec![]` to struct literal  |
| `src/shader_pipeline.rs`                     | Default flip, deprecations, 4 new tests              |
| `tests/shader_pipeline_performance_tests.rs` | Updated comparison test for new default              |

### Test Counts

- **4 new unit tests** in `shader_pipeline::tests` (2 roundtrip + 2 default
  verification)
- **1512 total lib tests** pass
- **All integration tests** pass
- **All examples compile**
