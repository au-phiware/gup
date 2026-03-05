# GUP-306: Vec3/Vec4 Arithmetic Operators

## Story Overview

**Initiative**: Core GPU Primitives **Status**: ✅ Complete **Created**:
2025-07-27 **Completed**: 2025-07-27

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

- [x] `Vec3` implements `Add`, `Sub`, `Mul`, `Div` for component-wise operations
      and scalar multiply/divide
- [x] `Vec4` implements `Add`, `Sub`, `Mul`, `Div` for component-wise operations
      and scalar multiply/divide
- [x] `Vec3` provides `From<[f32; 3]>` and `Into<[f32; 3]>` conversions
- [x] `Vec4` provides `From<[f32; 4]>` and `Into<[f32; 4]>` conversions
- [x] Both types are optionally migrated into `src/math.rs` alongside `Vec2`
- [x] Unit tests cover arithmetic, conversions, and edge cases for both types
- [x] `Vec3` arithmetic correctly handles the `_padding` field (preserves zero
      padding)

## Technical Tasks

- [x] Add arithmetic trait impls to `Vec3` (handling `_padding` field)
- [x] Add arithmetic trait impls to `Vec4`
- [x] Add `From`/`Into` array conversions for `Vec3` and `Vec4`
- [x] Optionally move `Vec3`/`Vec4` definitions to `src/math.rs` and re-export
      from `shader_function`
- [x] Write unit tests for all new trait implementations
- [x] Simplify any existing manual field arithmetic on Vec3/Vec4

## Implementation Summary

### What was implemented

- **Vec3**: `Add`, `Sub`, `Mul`, `Div` (component-wise), `Mul<f32>`,
  `Mul<Vec3> for f32`, `Div<f32>` (scalar), `From<[f32; 3]>`,
  `From<Vec3> for [f32; 3]>` — all operations preserve `_padding = 0.0`
- **Vec4**: `Add`, `Sub`, `Mul`, `Div` (component-wise), `Mul<f32>`,
  `Mul<Vec4> for f32`, `Div<f32>` (scalar), `From<[f32; 4]>`,
  `From<Vec4> for [f32; 4]>`

### Key files changed

- `src/shader_function/core.rs` — Added arithmetic operator impls and array
  conversions for Vec3 and Vec4
- `src/shader_function/mod.rs` — Added 28 unit tests covering arithmetic,
  conversions, edge cases, padding preservation, bytemuck roundtrips, and memory
  layout validation

### Design decisions

- **Kept Vec3/Vec4 in `shader_function/core.rs`** rather than moving to
  `src/math.rs` — the move was explicitly optional and would have created
  complexity with `ShaderType` impls (which depend on traits defined in the
  shader_function module). All types remain re-exported through
  `shader_function::*` and `prelude::*`.
- **No manual arithmetic to simplify** — a codebase audit found no existing
  manual Vec3/Vec4 field arithmetic patterns.

### Test counts

- 28 new tests (15 Vec3, 13 Vec4)
- Full test suite passes (2846+ tests)

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

- [x] All Acceptance Criteria met
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint clean: `mask all-fix`

## Retrospective

**Completed**: 2025-07-27

### Key Technical Learnings

#### Vec3 Padding Field Discipline

- **Challenge**: Vec3 has a `_padding` field for GPU 16-byte alignment that must
  always remain zero after operations, but Rust's type system doesn't enforce
  this automatically.
- **Solution**: Every arithmetic operator impl and the `From<[f32; 3]>`
  conversion explicitly sets `_padding: 0.0` in the result. A dedicated test
  (`vec3_padding_preserved`) deliberately constructs a Vec3 with non-zero
  padding and verifies all operations zero it out.
- **Pattern**: When GPU types have padding fields, treat every constructor path
  (including operator results) as a place that must zero the padding. A
  property-based test with intentionally corrupt padding catches regressions.

### Architectural Decisions

#### Keep Vec3/Vec4 in shader_function/core.rs

- **Decision**: Did not move Vec3/Vec4 to `src/math.rs` alongside Vec2.
- **Reasoning**: Vec3/Vec4 have `ShaderType` impls that depend on the
  `ShaderType` trait defined in the shader_function module. Moving the struct
  definitions would require either moving the trait (large churn) or splitting
  impls across modules (orphan rule complications). The story explicitly made
  this optional.
- **Trade-off**: Vec2 lives in `math.rs` while Vec3/Vec4 live in
  `shader_function/core.rs`, creating a slight inconsistency in module
  organisation. However, all three types are uniformly accessible through
  `shader_function::*` and `prelude::*`.
- **Future**: If a future story consolidates all math types, Vec3/Vec4 could be
  moved to `math.rs` with the `ShaderType` impls kept in shader_function via
  `impl ShaderType for crate::math::Vec3`.

### Development Workflow Insights

- The story was straightforward — following the existing Vec2 pattern made
  implementation mechanical and confident.
- ZFS snapshot accumulation on /home caused disk space exhaustion during builds.
  Using `CARGO_TARGET_DIR=/tmp/gup-target` as a workaround kept the workflow
  moving. This is an environment issue, not a code issue.
- The `mask all-fix` pre-commit hook includes markdown linting that flags
  pre-existing issues in other story files; these are unrelated to the current
  changes.
