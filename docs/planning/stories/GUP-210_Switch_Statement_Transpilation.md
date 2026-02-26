# GUP-210: Switch Statement Transpilation

## Story Overview

**Title**: Switch Statement Transpilation from Rust Match to WGSL  
**Epic**: Phase 2 Initiative 4 - Rust-to-WGSL Transpilation  
**Priority**: Low  
**Story Points**: 5  
**Status**: ✅ Complete (2025-07-24)

## Context

GUP-058 implemented control flow transpilation but explicitly excluded match
expression conversion, producing an error instead. WGSL supports `switch`
statements for integer matching, and simple Rust match expressions on integers
could be automatically converted.

## User Story

**As a** shader function developer  
**I want** to use simple Rust match expressions in shader functions  
**So that** I can write readable branching logic on integer values without
manually converting to if-else chains

## Problem Statement

Rust match expressions on integers map naturally to WGSL switch statements, but
complex patterns (guards, destructuring, references) have no WGSL equivalent. We
need to support the common case while producing clear errors for unsupported
patterns.

## Acceptance Criteria

- [x] Convert `match x { 0 => ..., 1 => ..., _ => ... }` to WGSL switch
- [x] Support integer literal patterns and default/wildcard arms
- [x] Error with clear message on unsupported patterns (guards, ranges, etc.)
- [x] Add `Switch` variant to `WgslStatement` AST
- [x] Test suite covering supported and unsupported match patterns

## Dependencies

- GUP-058: Control Flow Handling (provides the converter architecture)

## Testing Strategy

- Unit tests for integer match → switch conversion
- Error tests for unsupported pattern types
- Pipeline tests for end-to-end transpilation

## Definition of Done

- [x] Integer match expressions transpile to WGSL switch statements
- [x] Clear error messages for unsupported patterns
- [x] Test coverage for all supported cases

## Implementation Summary

### What was implemented

- **AST**: Added `Switch { selector, cases, default_body }` variant to
  `WgslStatement` enum and `SwitchCase` struct with `selectors` and `body`
  fields.
- **Conversion**: Added `convert_match_statement()` method that converts Rust
  `match` expressions on integers to WGSL `switch` statements. Supports integer
  literal patterns (`i32`, `u32`), wildcard (`_`) for default cases, and
  or-patterns (`1 | 2 | 3`) for multi-selector cases.
- **Arm body handling**: Added `convert_arm_body()` that properly handles block
  bodies, return statements, assignments, and compound assignments in match
  arms.
- **Error reporting**: Added `convert_case_pattern()` with clear error messages
  for unsupported patterns (guards, ranges, variable bindings, destructuring).
- **Code generation**: Added WGSL `switch` statement output with proper
  indentation and WGSL syntax (`case N: { ... }`, `default: { ... }`).
- **Optimizer/validation**: Extended `collect_idents_in_stmt`, `fold_stmt`, and
  `elim_conversions_stmt` to handle the new `Switch` variant.

### Key files changed

- `gup-macros/src/transpile/ast.rs` — `Switch` variant and `SwitchCase` struct
- `gup-macros/src/transpile/convert.rs` — Match conversion logic (3 new methods)
- `gup-macros/src/transpile/codegen.rs` — WGSL switch code generation
- `gup-macros/src/transpile/optimizer.rs` — Switch handling in 3 optimizer
  passes
- `gup-macros/src/transpile/validation.rs` — Switch handling in ident collection
- `gup-macros/src/transpile/control_flow_tests.rs` — 16 new tests

### Test counts

- 16 new tests added (15 in control_flow_tests, 1 in codegen tests)
- Total gup-macros tests: 487 (all passing)
- Full project tests: 1549+ (all passing except 1 pre-existing flaky perf test)
