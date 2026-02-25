# GUP-189: AST Integration with ComposableShaderPipeline

**Story ID**: GUP-189 **Title**: AST Integration with ComposableShaderPipeline
**Status**: 📋 Planned **Priority**: Medium **Effort**: 5 story points
**Created**: 2025-08-07 **Dependencies**: GUP-073 (Advanced Shader Composition)

## Overview

Wire the AST-based optimizer from `shader_ast` into the existing
`ComposableShaderPipeline.optimize_shader()` method so that current pipeline
users get AST-based dead code elimination, constant folding, and function
inlining transparently, without changing their API usage.

## Context

GUP-073 introduced a full AST-based WGSL system (`shader_ast` module) but it
lives alongside the existing string-based `ComposableShaderPipeline`. The two
systems are not yet integrated — the old pipeline still uses string-based
optimizations (regex-like replacements). This story bridges the gap.

## User Story

As a developer using the shader pipeline, I want the existing optimization
methods to use AST-based passes automatically, so that I get better
optimizations without changing my code.

## Acceptance Criteria

- [ ] `ComposableShaderPipeline::optimize_shader()` delegates to AST-based
      optimizer when `OptimizationConfig.use_ast_analysis` is true
- [ ] Backward-compatible: existing tests still pass
- [ ] AST parsing errors fall back to string-based optimization gracefully
- [ ] Generated WGSL from the pipeline is at least as small as before
- [ ] Performance: no regression in pipeline compilation benchmarks

## Technical Tasks

1. Add AST parsing step inside `optimize_shader()`.
2. Implement fallback logic if AST parsing fails.
3. Update `OptimizationConfig` to expose AST control.
4. Add integration tests comparing old and new optimizer output.

## Dependencies

- GUP-073: Advanced Shader Composition (provides `shader_ast` module)

## Testing Strategy

- Unit tests for the integration path.
- Regression tests comparing output before/after.
- Performance benchmarks to detect regressions.

## Success Metrics

- All existing `shader_pipeline` tests continue to pass.
- AST optimizer produces smaller or equal WGSL output.
- No measurable performance regression.

## Risk Assessment

- **Low risk**: The AST system is new and additive. String-based fallback
  ensures no breakage.

## Definition of Done

- [ ] Implementation complete with tests passing
- [ ] `mask all-fix` clean
- [ ] All examples compile
- [ ] Documentation updated
