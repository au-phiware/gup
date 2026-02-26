# GUP-208: Mark Derive Macro GPU Instance Buffer Generation

**Status**: ✅ Complete (2025-07-18) **Priority**: Low **Category**: Developer
Experience **Estimated Effort**: 1 day **Dependencies**: GUP-071 (Custom Mark
Development Kit)

## Summary

Extend the `#[derive(Mark)]` macro to generate GPU-compatible instance buffer
types from field annotations (`#[mark(position)]`, `#[mark(color)]`), enabling
fully automatic storage buffer layout for custom marks.

## Background

GUP-071 delivered the `#[derive(Mark)]` macro that generates Mark trait
implementations, vertex types, and attribute type validation. However, the
current implementation does not generate GPU instance data types (the
`#[repr(C)]` structs with bytemuck derives that get uploaded to storage
buffers). Developers must still manually create these when using custom marks
with the renderer.

## User Story

As a custom mark developer, I want field annotations on my derive-based mark to
automatically generate GPU-compatible instance buffer types, so that I can
render custom marks without manually creating aligned data structures.

## Acceptance Criteria

- [x] `#[mark(position)]`, `#[mark(color)]`, `#[mark(size)]` field annotations
      generate corresponding fields in a `{Name}Instance` struct
- [x] Generated instance struct has `#[repr(C)]`, `bytemuck::Pod`,
      `bytemuck::Zeroable`
- [x] Proper WGSL alignment padding is inserted automatically (e.g., vec4 needs
      16-byte alignment)
- [x] `From<&{Name}> for {Name}Instance` conversion is generated
- [x] Generated structs pass the existing `MarkValidator` memory layout checks
- [x] Documentation updated with instance buffer generation examples

## Technical Tasks

1. Add field attribute parsing to `mark_derive.rs`
2. Generate instance struct with proper alignment
3. Generate `From` conversion implementation
4. Add tests for various field combinations
5. Update documentation

## Testing Strategy

- Compile-time tests for correct attribute parsing
- Runtime tests verifying bytemuck compatibility
- Alignment validation tests

## Risk Assessment

- **Low risk**: Builds on established patterns from existing mark types
- **Alignment complexity**: WGSL alignment rules are well-understood from
  existing implementations

## Definition of Done

- [x] All acceptance criteria met
- [x] Tests pass with `--test-threads=1`
- [x] `mask all-fix` passes
- [x] Documentation updated

## Implementation Summary

### What Was Implemented

Extended the `#[derive(Mark)]` proc macro to generate GPU-compatible
`{Name}Instance` structs from field-level annotations.

### Key Files Changed

- **`gup-macros/src/mark_derive.rs`** — Core implementation:
  - `GpuType` enum with WGSL alignment/size/conversion logic
  - `parse_field_mark_role()` for `#[mark(role)]` attribute parsing
  - `generate_instance_struct()` for padded struct + From impl generation
  - `extract_type_name()` helper for type path extraction
- **`gup-macros/src/lib.rs`** — Updated derive macro rustdoc with field
  annotation documentation
- **`examples/custom_mark_demo.rs`** — Added instance buffer generation showcase
  with Diamond and Arrow marks
- **`docs/CUSTOM_MARK_GUIDE.md`** — New "Instance Buffer Generation" section
  with WGSL alignment table and usage examples
- **`tests/mark_derive_instance_tests.rs`** — 28 comprehensive tests

### Test Count

28 new tests covering:

- Instance struct creation from field annotations (10 mark types)
- `From<&T>` and `From<T>` conversions
- WGSL alignment validation (Vec2, Vec3, Vec4 padding)
- Struct size as multiples of max alignment
- bytemuck Pod/Zeroable compatibility and array casting
- Custom role names, integer fields, triangle primitives
- MarkValidator integration (all marks pass validation)
- Debug, Clone, Copy derive verification
