# GUP-208: Mark Derive Macro GPU Instance Buffer Generation

**Status**: 🚧 In Progress **Priority**: Low **Category**: Developer Experience
**Estimated Effort**: 1 day **Dependencies**: GUP-071 (Custom Mark Development
Kit)

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

- [ ] `#[mark(position)]`, `#[mark(color)]`, `#[mark(size)]` field annotations
      generate corresponding fields in a `{Name}Instance` struct
- [ ] Generated instance struct has `#[repr(C)]`, `bytemuck::Pod`,
      `bytemuck::Zeroable`
- [ ] Proper WGSL alignment padding is inserted automatically (e.g., vec4 needs
      16-byte alignment)
- [ ] `From<&{Name}> for {Name}Instance` conversion is generated
- [ ] Generated structs pass the existing `MarkValidator` memory layout checks
- [ ] Documentation updated with instance buffer generation examples

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

- [ ] All acceptance criteria met
- [ ] Tests pass with `--test-threads=1`
- [ ] `mask all-fix` passes
- [ ] Documentation updated
