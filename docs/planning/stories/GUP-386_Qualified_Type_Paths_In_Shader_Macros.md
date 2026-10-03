# GUP-386: Accept Qualified Type Paths in Shader Function Macros

## Story Overview

**Initiative**: Developer Experience **Status**: 📋 Planned **Created**:
2026-10-04

## Context

GUP-377 made every proc macro emit absolute `::gup::` paths, so
`#[wgsl_function]`, `#[shader_fn]` and the derives no longer need glob imports.
While writing the external-crate test for that story, a remaining import
requirement surfaced: the macros reject qualified type paths in signatures and
struct fields.

```rust
#[wgsl_function]
fn shade(color: ::gup::shader_function::Vec3, k: f32) -> f32 { ... }
// error: Unsupported input type for WGSL function: Complex type paths not yet supported

#[derive(WgslStruct)]
#[repr(C)]
struct Material { albedo: gup::shader_function::Vec3 }
// error: Complex type paths are not supported in WgslStruct. Use simple type names.
```

Users must therefore `use gup::shader_function::{Vec2, Vec3, Vec4, ...}` by
name. The type mappers match on single-segment paths only (`Vec3`, `Mat4`, ...),
and uniform-field conversion (`Vec3` to `[f32; 3]`) does the same.

## User Story

> "As a developer, I want to write fully qualified types such as
> `gup::shader_function::Vec3` in shader function signatures and WGSL structs,
> so that I can follow my crate's import conventions without the macros
> rejecting my code."

## Acceptance Criteria

- [ ] `#[wgsl_function]` and `#[shader_fn]` accept multi-segment type paths
      whose final segment is a known WGSL-mappable type (`Vec2`, `Vec3`, `Vec4`,
      `Mat2`, `Mat3`, `Mat4`, scalars).
- [ ] `#[derive(WgslStruct)]`, `#[derive(ShaderType)]` and `#[derive(Mark)]`
      accept the same qualified paths for fields.
- [ ] Uniform-field conversion (vector types to `[f32; N]`) works for qualified
      paths.
- [ ] Qualified paths to custom structs (e.g. `my_mod::Material`) are treated as
      custom types and emit the struct's WGSL name (the final segment).
- [ ] Error messages for truly unsupported types are unchanged.

## Technical Tasks

- [ ] Add a shared helper in `gup-macros` that resolves a `syn::TypePath` to its
      final segment for type mapping (reject generics and `qself`).
- [ ] Use it in `wgsl_function.rs`, `shader_fn.rs`, `wgsl_struct.rs`,
      `mark_derive.rs` and the `ShaderType` derive in `lib.rs`.
- [ ] Extend `tests/macro_crate_path_tests.rs` to use qualified vector types
      without named imports.

## Dependencies

### Prerequisite Stories

- GUP-377: Fix `#[wgsl_function]` crate path resolution ✅

## Testing Strategy

- Unit tests in `gup-macros` for qualified-path type mapping.
- Integration tests using `::gup::shader_function::Vec3` in each macro.

## Success Metrics

- [ ] `tests/macro_crate_path_tests.rs` needs no named `Vec*` imports.

## Risk Assessment

- **Low**: Matching on the final segment could accept a user type that happens
  to be named `Vec3`. That is already true for single-segment paths, so the
  behaviour does not get worse.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Story status updated to ✅ Complete
