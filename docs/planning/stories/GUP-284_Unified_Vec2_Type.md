# GUP-284: Unified Vec2 Type

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-26

## Context

The codebase has multiple 2D vector representations:

- `interaction::Vec2` — simple struct, no arithmetic ops, used by
  `InteractionEvent`, `ElementHit`, `event::RawInputEvent`, etc.
- `shader_function::Vec2` — WGSL-compatible type with `bytemuck::Pod`
- Various `[f32; 2]` arrays — used in GPU buffer structs

During GUP-013 implementation, the event module needed a `ViewportTransform`
with arithmetic operations on `Vec2`, but `interaction::Vec2` doesn't implement
`Add`/`Sub`/`Mul`/`Div`. This required manual field-level arithmetic, which is
error-prone and verbose.

## User Story

> "As a library developer, I want a single, ergonomic 2D vector type used
> throughout the codebase so that I can perform coordinate transforms and
> arithmetic without manual field decomposition or type conversion."

## Acceptance Criteria

- [x] A single `Vec2` type is used across interaction, event, and public API
      surfaces
- [x] The type implements `Add`, `Sub`, `Mul`, `Div` for component-wise
      operations and scalar multiply/divide
- [x] The type implements `Debug`, `Clone`, `Copy`, `PartialEq`
- [x] Conversion `From<[f32; 2]>` and `Into<[f32; 2]>` are provided
- [x] All existing `interaction::Vec2` and `shader_function::Vec2` usages are
      migrated or aliased
- [x] No new external dependency required (implement in-crate)

## Technical Tasks

- [x] Promote `interaction::Vec2` to a top-level `src/math.rs` module (or
      similar) and implement arithmetic traits
- [x] Update `interaction.rs` to use the new type
- [x] Update `event.rs` to use the new type
- [x] Verify `shader_function::Vec2` can coexist or be replaced
- [x] Update all call sites

## Dependencies

### Prerequisite Stories

- None (standalone refactoring)

## Testing Strategy

- Unit tests for arithmetic ops, conversions, edge cases (zero, negative)
- Compilation check of all modules after migration

## Risk Assessment

- **Medium**: Wide-reaching refactor touching many files. Mitigate by using a
  type alias initially and converting incrementally.

## Definition of Done

- [x] All Acceptance Criteria met
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint clean: `mask all-fix`

## Implementation Summary

### What was implemented

A single canonical `Vec2` type in `src/math.rs` that unifies the two
previously separate definitions (`interaction::Vec2` and
`shader_function::Vec2`). The type is GPU-compatible (`#[repr(C)]`,
`bytemuck::Pod`), implements full arithmetic operators (`Add`, `Sub`, `Mul`,
`Div` for both component-wise and scalar operations), and provides conversions
to/from `[f32; 2]`.

### Key files changed

- **`src/math.rs`** — New module: canonical `Vec2` definition with 13 unit
  tests
- **`src/interaction.rs`** — Removed local `Vec2` struct, replaced with
  `pub use crate::math::Vec2`; simplified `Rect` and `GestureRecognizer` code
  using arithmetic operators
- **`src/shader_function.rs`** — Removed local `Vec2` struct, replaced with
  `pub use crate::math::Vec2`; kept `ShaderType` impl
- **`src/event.rs`** — Simplified `ViewportTransform` using arithmetic
  operators (`screen_to_world`, `world_to_screen`)
- **`src/lib.rs`** — Added `pub mod math`

### Test counts

- 13 new unit tests in `math::tests` (constructors, all arithmetic ops,
  conversions, bytemuck Pod, repr(C) layout, zero/negative edge cases)
- 2438 existing lib tests pass unchanged
- All integration tests pass
- All examples compile
