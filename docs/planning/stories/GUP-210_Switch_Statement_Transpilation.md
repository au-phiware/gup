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

## Retrospective

**Completed**: 2025-07-24

### Key Technical Learnings

#### Match Arm Body Conversion

- **Challenge**: Rust match arm bodies are expressions, but WGSL switch case
  bodies are compound statements. Assignment expressions like `result = 10` were
  being handled through `convert_expr` which uses a placeholder BinaryOp for
  assignments, producing `result + 10;` instead of `result = 10;`.
- **Solution**: Added explicit handling for `Expr::Assign` and compound
  assignments in `convert_arm_body()`, routing them through the statement
  conversion path instead of the expression path.
- **Pattern**: When converting expression-context code to statement-context,
  always check if the expression needs statement-level handling (assignments,
  compound assignments, control flow).

#### Exhaustive Enum Matching

- **Challenge**: Adding a new variant to `WgslStatement` requires updating every
  match expression across the codebase (optimizer, validation, codegen). The
  Rust compiler correctly caught 4 non-exhaustive matches.
- **Solution**: Followed the existing pattern in each module (optimizer's
  `collect_idents_in_stmt`, `fold_stmt`, `elim_conversions_stmt`, and
  validation's `collect_idents_stmt`) to recurse into switch selectors, case
  selectors, case bodies, and default bodies.
- **Pattern**: When adding AST variants, grep for all match expressions on the
  enum to ensure exhaustive coverage. The compiler helps, but understanding the
  semantic intent of each match is important.

#### syn 2.0 PatLit API

- **Challenge**: The `PatLit` struct in syn 2.0 wraps `ExprLit` (via a type
  alias), exposing a `lit: Lit` field directly. Negative integer literals like
  `-1` in pattern context are parsed as a single `Lit::Int` token with the
  negative sign included.
- **Solution**: Used `Pat::Lit(lit) => self.convert_literal(&lit.lit)` to reuse
  the existing literal conversion, which handles i32, u32, f32, and bool.
- **Pattern**: Pattern literals and expression literals have the same `Lit` type
  in syn 2.0, so existing conversion functions can be reused.

### Architectural Decisions

#### Statement-Only Match Support

- **Decision**: Match expressions are supported only as statements (not in
  expression position like `let x = match y { ... };`).
- **Reasoning**: WGSL `switch` is a statement, not an expression. Supporting
  match-as-expression would require introducing a temporary variable and
  assigning in each case body, which adds complexity.
- **Trade-off**: Users who want match-as-expression must restructure their code
  to use match-as-statement with a mutable variable.
- **Future**: Could add match-as-expression support by generating
  `var result; switch(...) { case N: { result = ...; } }` but this adds
  significant complexity for limited benefit.

#### Or-Pattern Multi-Selector Support

- **Decision**: Support Rust or-patterns (`1 | 2 | 3`) mapping to WGSL
  multi-selector cases (`case 1, 2, 3:`).
- **Reasoning**: This is a natural 1:1 mapping between the two languages and
  avoids users having to duplicate case bodies.
- **Trade-off**: None — clean semantic mapping.
- **Future**: This enables efficient multi-value matching in shader functions.

### Development Workflow Insights

- The pre-commit hook caught a formatting issue in INDEX.md (prettier
  formatting) that would have been easy to miss. The `mask all-fix` workflow is
  essential.
- Edition 2024 string parsing is strict — a duplicated line in a string literal
  context caused cascading parse errors across the entire file that were very
  misleading (errors appeared in pre-existing, valid code far from the actual
  issue). Lesson: when seeing bizarre parse errors in existing code after an
  edit, look for syntax errors (especially unterminated strings) near the edit
  location.
- The existing control flow test infrastructure (`transpile()` and
  `transpile_with_uniforms()` helpers) made adding new match tests very
  straightforward.
