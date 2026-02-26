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

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### WGSL Alignment in #[repr(C)] Structs

- **Challenge**: Rust `[f32; N]` arrays always have 4-byte alignment regardless
  of N, but WGSL requires `vec2<f32>` at 8-byte boundaries and
  `vec3<f32>`/`vec4<f32>` at 16-byte boundaries. Simply using `[f32; 4]` in a
  `#[repr(C)]` struct does NOT guarantee 16-byte alignment.
- **Solution**: Insert explicit padding fields (`_padN: f32` or
  `_padN: [f32; K]`) before each data field to push it to the correct WGSL
  offset. Since all supported types have at least 4-byte alignment, padding is
  always expressible as `f32` multiples.
- **Pattern**: Track `current_offset` and `max_alignment` during code
  generation; compute padding as `(alignment - offset % alignment) % alignment`;
  add struct tail padding to make size a multiple of max alignment.

#### Proc Macro Field Attribute Parsing

- **Challenge**: The `#[mark(...)]` attribute serves dual purposes — container-
  level (`#[mark(primitive = "quad")]`) and field-level (`#[mark(position)]`).
  Need to distinguish path-only identifiers from key-value pairs in
  `parse_nested_meta`.
- **Solution**: Use `meta.input.peek(syn::Token![=])` to detect key-value pairs
  before consuming. For key-value pairs, call `meta.value()` to consume the
  `= value` portion. For simple identifiers, read `meta.path.get_ident()`.
- **Pattern**: Container and field attributes use the same `mark` namespace but
  are parsed from different AST nodes (`input.attrs` vs `field.attrs`), so there
  is no actual conflict.

#### Backward Compatibility via Conditional Generation

- **Challenge**: Adding instance struct generation must not break existing
  derive-based marks that have no field annotations.
- **Solution**: Only generate the `{Name}Instance` struct when at least one
  field has a `#[mark(...)]` annotation. Empty annotation set → empty token
  stream → no change to generated output.
- **Pattern**: Conditional code generation based on annotation presence
  preserves backward compatibility without feature flags.

### Architectural Decisions

#### Enum-Based GPU Type Representation

- **Decision**: Used a `GpuType` enum instead of storing `TokenStream` values in
  data structures.
- **Reasoning**: Follows the project convention of preferring enums over trait
  objects. Enum variants are `Copy`, enabling simple iteration without lifetime
  issues. Each variant encapsulates alignment, size, instance type, and
  conversion logic.
- **Trade-off**: Limited to a fixed set of supported types (f32, i32, u32, Vec2,
  Vec3, Vec4). Mat types are excluded from instance generation.
- **Future**: Can extend the enum if matrix types are needed in instance
  buffers; the pattern is straightforward to grow.

#### Annotated Fields Only (Not All Fields)

- **Decision**: Only fields with explicit `#[mark(role)]` annotations are
  included in the instance struct, rather than including all fields
  automatically.
- **Reasoning**: Gives developers explicit control over what data goes to the
  GPU. Matches the principle of least surprise — the annotation is the opt-in.
- **Trade-off**: Slightly more verbose for marks where all fields should be
  GPU-uploaded.
- **Future**: Could add a container-level `#[mark(instance_all)]` attribute to
  opt all fields in without individual annotations.

### Development Workflow Insights

- The implementation was cleanly isolated to the proc macro crate, with no
  changes needed to the core `gup` library code. This validates the macro-based
  approach to extensibility.
- Vec3 has an internal `_padding` field in gup's `shader_function` module, which
  caught a test issue early — need to construct Vec3 with `_padding: 0.0`.
- The `mask all-fix` quality gate caught no issues on any commit, confirming the
  code was clean throughout.
- The pre-existing flaky test `test_registry_scalability` appeared once during
  full test runs — this is a known intermittent GPU resource contention issue,
  not related to this story's changes.
