# GUP-190: WGSL Compute Shader AST Support

**Story ID**: GUP-190 **Title**: WGSL Compute Shader AST Support **Status**: 📋
Planned **Priority**: Low **Effort**: 5 story points **Created**: 2025-08-07
**Dependencies**: GUP-073 (Advanced Shader Composition)

## Overview

Extend the `shader_ast` parser and AST types to handle WGSL compute shader
constructs including workgroup attributes, storage buffer declarations,
compute-specific builtins (`@workgroup_size`, `global_invocation_id`), and
read-write storage access qualifiers.

## Context

GUP-073 built the AST system targeting vertex/fragment shader composition. The
project also uses compute shaders extensively (hit testing, instance filtering,
tessellation, statistics, etc.) but these are currently hand-written WGSL. Being
able to parse and optimise compute shaders through the same AST system would
enable compute shader composition and optimization.

## User Story

As a developer composing compute shaders, I want the AST system to understand
compute-specific WGSL constructs, so that I can use type checking and
optimization on compute shader pipelines.

## Acceptance Criteria

- [ ] Parser handles `@workgroup_size(x, y, z)` attributes
- [ ] Parser handles `var<storage, read_write>` declarations
- [ ] Parser handles compute builtins (`global_invocation_id`,
      `local_invocation_id`, etc.)
- [ ] Existing compute shader WGSL files in `src/shaders/` can be parsed
- [ ] Round-trip tests for compute shader WGSL
- [ ] Dead code elimination works on compute entry points

## Technical Tasks

1. Extend `AddressSpace` to support `storage` with access mode
   (read/read_write).
2. Add `workgroup_size` attribute parsing.
3. Parse compute-specific builtins.
4. Test against existing compute shaders in `src/shaders/`.
5. Update optimizer entry-point detection for `@compute` functions.

## Dependencies

- GUP-073: Advanced Shader Composition (provides `shader_ast` module)

## Testing Strategy

- Parse each existing compute shader in `src/shaders/`.
- Round-trip tests for compute shader constructs.
- Optimization tests with compute entry points.

## Success Metrics

- All existing `.wgsl` files in `src/shaders/` can be parsed.
- No regression in existing vertex/fragment shader tests.

## Risk Assessment

- **Medium risk**: Compute shaders may use WGSL features not yet in the parser
  (atomics, shared memory). Incremental approach mitigates this.

## Definition of Done

- [ ] Implementation complete with tests passing
- [ ] `mask all-fix` clean
- [ ] All examples compile
- [ ] Documentation updated
