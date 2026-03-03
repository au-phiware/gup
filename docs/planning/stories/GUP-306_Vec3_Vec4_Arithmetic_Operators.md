# GUP-306: Vec3/Vec4 Arithmetic Operators

## Story Overview

**Initiative**: Core GPU Primitives **Status**: 📋 Planned **Created**:
2025-07-27

## Context

GUP-284 unified `Vec2` into `src/math.rs` with full arithmetic operators (`Add`,
`Sub`, `Mul`, `Div` for both component-wise and scalar operations) and array
conversions (`From<[f32; 2]>`, `Into<[f32; 2]>`). However, `Vec3` and `Vec4`
remain in `shader_function.rs` with only constructors (`new`, `zero`, `one`) and
`ShaderType` impls — no arithmetic operators and no array conversions. This
inconsistency will surprise developers who expect uniform ergonomics across all
vector types.

## User Story

> "As a library developer, I want `Vec3` and `Vec4` to support the same
> arithmetic operators and conversions as `Vec2` so that I can perform
> calculations on 3D/4D vectors without manual field decomposition."

## Acceptance Criteria

- [ ] `Vec3` implements `Add`, `Sub`, `Mul`, `Div` for component-wise operations
      and scalar multiply/divide
- [ ] `Vec4` implements `Add`, `Sub`, `Mul`, `Div` for component-wise operations
      and scalar multiply/divide
- [ ] `Vec3` provides `From<[f32; 3]>` and `Into<[f32; 3]>` conversions
- [ ] `Vec4` provides `From<[f32; 4]>` and `Into<[f32; 4]>` conversions
- [ ] Both types are optionally migrated into `src/math.rs` alongside `Vec2`
- [ ] Unit tests cover arithmetic, conversions, and edge cases for both types
- [ ] `Vec3` arithmetic correctly handles the `_padding` field (preserves zero
      padding)

## Technical Tasks

- [ ] Add arithmetic trait impls to `Vec3` (handling `_padding` field)
- [ ] Add arithmetic trait impls to `Vec4`
- [ ] Add `From`/`Into` array conversions for `Vec3` and `Vec4`
- [ ] Optionally move `Vec3`/`Vec4` definitions to `src/math.rs` and re-export
      from `shader_function`
- [ ] Write unit tests for all new trait implementations
- [ ] Simplify any existing manual field arithmetic on Vec3/Vec4

## Dependencies

### Prerequisite Stories

- GUP-284 ✅ — Established the pattern in `math.rs` for Vec2

## Testing Strategy

- Unit tests for all arithmetic ops on Vec3 and Vec4
- Conversion roundtrip tests
- Edge case tests (zero, negative, large values)
- Verify `_padding` field in Vec3 is always zero after operations
- Full test suite regression check

## Risk Assessment

- **Low**: Straightforward trait implementations following the Vec2 pattern. The
  `Vec3` `_padding` field needs care but is well understood.

## Definition of Done

- [ ] All Acceptance Criteria met
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint clean: `mask all-fix`
