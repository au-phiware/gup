# GUP-190: WGSL Compute Shader AST Support

**Story ID**: GUP-190 **Title**: WGSL Compute Shader AST Support **Status**: ✅
Complete **Priority**: Low **Effort**: 5 story points **Created**: 2025-08-07
**Completed**: 2025-08-08 **Dependencies**: GUP-073 (Advanced Shader
Composition)

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

- [x] Parser handles `@workgroup_size(x, y, z)` attributes
- [x] Parser handles `var<storage, read_write>` declarations
- [x] Parser handles compute builtins (`global_invocation_id`,
      `local_invocation_id`, etc.)
- [x] Existing compute shader WGSL files in `src/shaders/` can be parsed
- [x] Round-trip tests for compute shader WGSL
- [x] Dead code elimination works on compute entry points

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

- [x] Implementation complete with tests passing
- [x] `mask all-fix` clean
- [x] All examples compile
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

Extended the `shader_ast` module (parser, types, generator, optimizer) to fully
support WGSL compute shader constructs. This enables parsing, type checking,
code generation, and dead code elimination for all 9 existing compute shaders.

### Key Changes

**Type System (`types.rs`)**:

- `AccessMode` enum (Read, ReadWrite) for storage buffer access qualifiers
- `AddressSpace::Storage(AccessMode)` to carry access mode information
- `AddressSpace::Function` for pointer address spaces
- `Attribute::WorkgroupSize(u32, Option<u32>, Option<u32>)` for 1/2/3D workgroup
  sizes
- `Parameter.attributes` field to preserve parameter decorators
- `WgslType::Atomic(Box<WgslType>)` for atomic types
- `WgslType::Pointer(AddressSpace, Box<WgslType>)` for pointer types
- `UnaryOp::AddressOf` and `UnaryOp::Deref` for `&` and `*` operators
- `BinaryOp::{BitwiseAnd, BitwiseOr, BitwiseXor, ShiftLeft, ShiftRight}`
- `Statement::{CompoundAssign, Loop, Break, Continue, Switch}` variants
- `SwitchCase` struct for switch statement cases
- `GlobalConst` struct and `WgslModule.constants` for `const` declarations

**Parser (`parser.rs`)**:

- Hex literal lexing (0x00FF00FFu)
- Scientific notation in floats (1e38, 3.4e+38)
- New tokens: Ampersand, Pipe, Caret, ShiftLeft, ShiftRight, PlusEqual
- New keywords: loop, break, continue, switch, case, default, const
- `@workgroup_size(x[, y[, z]])` attribute parsing
- `var<storage, read_write>` with access mode parsing
- Parameter attributes preserved (not discarded)
- `atomic<T>` and `ptr<space, T>` type parsing
- `expect_greater()` handling `>>` token splitting for nested generics
- Switch/case/default statement parsing
- Loop/break/continue statement parsing
- `const` declaration parsing (top-level and in-function)
- `&expr` (address-of) and `*expr` (dereference) unary operators
- `+=` compound assignment
- Bitwise operator precedence chain (`|`, `^`, `&`, `<<`, `>>`)
- For-loop update expression can be an assignment
- `else if` chain support

**Generator (`generator.rs`)**:

- All new statement types (switch, loop, break, continue, compound assign)
- Constant declarations
- Parameter attributes
- New unary operators (address-of, deref)

**Optimizer (`optimizer.rs`)**:

- All traversal functions handle new statement/expression types
- DCE correctly traverses switch cases, loops, compound assignments
- Type collector handles Atomic and Pointer types

### Files Changed

- `src/shader_ast/types.rs` — Core AST type definitions
- `src/shader_ast/parser.rs` — WGSL parser and lexer
- `src/shader_ast/generator.rs` — WGSL code generator
- `src/shader_ast/optimizer.rs` — Dead code elimination, constant folding,
  inlining
- `src/shader_ast/type_check.rs` — Parameter type updates
- `src/shader_ast/benchmarks.rs` — Parameter type updates

### Test Counts

- 89 shader_ast tests total (31 new)
- 9 compute shader file parsing tests (all pass)
- 4 round-trip tests for compute constructs
- 3 DCE tests with compute entry points
- 15 unit tests for individual compute features
