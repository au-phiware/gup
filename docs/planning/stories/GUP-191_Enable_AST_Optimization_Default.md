# GUP-191: Enable AST Optimization by Default

**Story ID**: GUP-191 **Title**: Enable AST Optimization by Default **Status**:
🚧 In Progress **Priority**: Low **Effort**: 3 story points **Created**: 2025-08-08
**Dependencies**: GUP-189, GUP-190

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

- [ ] `OptimizationConfig::default()` sets `use_ast_analysis` to `true`
- [ ] All existing tests pass with the new default
- [ ] The AST parser can handle all WGSL output from `generate_vertex_shader()`
      and `generate_fragment_shader()`
- [ ] String-based optimization methods are marked `#[deprecated]`
- [ ] Documentation updated to reflect the new default

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

- [ ] Implementation complete with tests passing
- [ ] `mask all-fix` clean
- [ ] All examples compile
- [ ] Documentation updated
