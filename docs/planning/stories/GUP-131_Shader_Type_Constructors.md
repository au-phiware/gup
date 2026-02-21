# GUP-131: Add Constructor Methods to Shader Types

**Status**: 🚧 In Progress  
**Priority**: Low  
**Story Points**: 1  
**Created**: 2025-01-10 (from GUP-032 retrospective)

## Story Overview

**Title**: Add Constructor Methods to Shader Types  
**Epic**: Phase 1 Initiative 2 - Unified Shader Function System

## Context

During GUP-032 implementation, discovered that shader types like Vec2, Vec3,
Vec4, Mat2, Mat3, Mat4 lack standard `new()` constructors. This forces awkward
struct literal syntax:

```rust
// Current (awkward)
let v = Vec2 { x: 1.0, y: 2.0 };

// Desired (standard)
let v = Vec2::new(1.0, 2.0);
```

## User Story

**As a** visualization developer  
**I want** shader types to have standard constructor methods  
**So that** I can use familiar Rust patterns and reduce code verbosity

## Acceptance Criteria

### AC1: Vec2/Vec3/Vec4 Constructors

- [ ] Add `Vec2::new(x: f32, y: f32) -> Self`
- [ ] Add `Vec3::new(x: f32, y: f32, z: f32) -> Self`
- [ ] Add `Vec4::new(x: f32, y: f32, z: f32, w: f32) -> Self`
- [ ] Add `::zero()` and `::one()` convenience constructors
- [ ] Tests for all constructors

### AC2: Matrix Constructors

- [ ] Add `Mat2::new(...)` constructor
- [ ] Add `Mat3::new(...)` constructor
- [ ] Add `Mat4::new(...)` constructor
- [ ] Add `::identity()` constructor for matrices
- [ ] Tests for matrix constructors

### AC3: Documentation

- [ ] Update examples to use new constructors
- [ ] Document constructor patterns in shader_function module
- [ ] Add rustdoc examples for each constructor

## Technical Tasks

1. Add constructor methods to each shader type in `src/shader_function.rs`
2. Ensure constructors are const where possible
3. Update existing code to use constructors (breaking change)
4. Update all examples and tests
5. Run full test suite to catch any missed usages

## Dependencies

- None - purely additive API enhancement

## Success Metrics

- [ ] All shader types have standard constructors
- [ ] No more struct literal syntax needed in examples
- [ ] Zero performance impact (inline constructors)

## Definition of Done

- [ ] All constructors implemented and tested
- [ ] Examples updated to use new API
- [ ] Documentation updated
- [ ] All tests passing
- [ ] Code review completed

---

_Created from GUP-032 retrospective - identified during Path mark
implementation._
