# GUP-284: Unified Vec2 Type

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 🚧 In Progress **Created**:
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

- [ ] A single `Vec2` type is used across interaction, event, and public API
      surfaces
- [ ] The type implements `Add`, `Sub`, `Mul`, `Div` for component-wise
      operations and scalar multiply/divide
- [ ] The type implements `Debug`, `Clone`, `Copy`, `PartialEq`
- [ ] Conversion `From<[f32; 2]>` and `Into<[f32; 2]>` are provided
- [ ] All existing `interaction::Vec2` and `shader_function::Vec2` usages are
      migrated or aliased
- [ ] No new external dependency required (implement in-crate)

## Technical Tasks

- [ ] Promote `interaction::Vec2` to a top-level `src/math.rs` module (or
      similar) and implement arithmetic traits
- [ ] Update `interaction.rs` to use the new type
- [ ] Update `event.rs` to use the new type
- [ ] Verify `shader_function::Vec2` can coexist or be replaced
- [ ] Update all call sites

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

- [ ] All Acceptance Criteria met
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
