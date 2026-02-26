# GUP-213: Transpiler Custom Struct Support

## Story Overview

**Title**: Custom Struct Parameter Support in `#[shader_fn]`  
**Epic**: Phase 2 Initiative 3 - Rust-to-WGSL Transpilation Research  
**Priority**: Low  
**Story Points**: 5  
**Status**: ✅ Complete (2025-07-18)

## Context

The current `#[shader_fn]` transpiler supports primitive types, vectors, and
matrices as function parameters. Custom structs (defined with `#[wgsl_struct]`)
are not yet supported as uniform parameters, limiting the complexity of data
that can be passed to transpiled shader functions.

## User Story

**As a** developer writing transpiled shader functions  
**I want** to use custom structs as parameters in `#[shader_fn]` functions  
**So that** I can pass complex structured data to GPU shaders without falling
back to `#[wgsl_function]`

## Acceptance Criteria

- [x] `#[shader_fn]` functions accept parameters of types decorated with
      `#[wgsl_struct]`
- [x] The generated WGSL includes the struct definition
- [x] Field access on custom struct parameters transpiles correctly
- [x] Custom structs work as both input and uniform parameters
- [x] Memory layout alignment is correct per WGSL spec

## Technical Tasks

1. Extend `shader_fn.rs` to recognise custom struct types
2. Generate WGSL struct definitions from `#[wgsl_struct]` metadata
3. Handle field access transpilation for custom struct members
4. Validate memory layout alignment for custom structs
5. Add integration tests with GPU compilation validation

## Dependencies

- GUP-061: Integration with Shader Function System (complete)
- GUP-064-B: Custom Struct Code Generation (complete)

## Testing Strategy

- Unit tests for custom struct type resolution
- Integration tests with GPU compilation of struct-using shaders
- Memory alignment validation tests

## Success Metrics

- Custom structs work seamlessly in `#[shader_fn]` functions
- Generated WGSL compiles correctly on GPU
- No regressions in existing shader function tests

## Risk Assessment

- **Medium risk**: Requires coordination between `#[wgsl_struct]` and
  `#[shader_fn]` macro systems
- **Alignment concerns**: Must ensure WGSL struct alignment rules are followed

## Definition of Done

- [x] Custom struct parameters supported in `#[shader_fn]`
- [x] WGSL struct definitions generated correctly
- [x] Field access transpilation working
- [x] GPU compilation tests passing
- [x] Integration tests with existing shader pipeline

## Implementation Summary

### What Was Implemented

The infrastructure for custom struct support in `#[shader_fn]` was already
largely in place through prior work on GUP-061 and GUP-064-B. This story
validated, tested, and documented the end-to-end integration:

1. **Type recognition**: `rust_type_to_wgsl_string()` in `shader_fn.rs` already
   passes through unknown type names, and `is_custom_type()` correctly identifies
   non-primitive types. No code changes were needed.

2. **WGSL struct definition inclusion**: `generate_wgsl()` in the generated
   `ComposableShaderFunction` impl queries `ShaderType::wgsl_type_definition()`
   for each custom type and prepends the definitions. This works correctly for
   both input and uniform custom struct types.

3. **Field access transpilation**: The `RustToWgsl` converter in `convert.rs`
   handles field access on both input parameters (direct: `point.x`) and uniform
   parameters (via prefix: `uniforms.config.scale`). No changes needed.

4. **Memory layout alignment**: `#[derive(WgslStruct)]` enforces `#[repr(C)]`
   and generates proper alignment info. The generated uniform structs correctly
   embed custom struct types that are `bytemuck::Pod + Zeroable`.

### Key Files

- `gup-macros/src/shader_fn.rs` — 7 new unit tests for custom struct handling
- `tests/shader_fn_custom_struct_tests.rs` — 20 new integration tests

### Test Counts

- 7 new unit tests in `shader_fn::tests`
- 20 new integration tests in `shader_fn_custom_struct_tests`
- 3 GPU compilation validation tests
- All 1549+ existing tests continue to pass

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### Existing Infrastructure Was Sufficient

- **Challenge**: The story assumed custom struct support needed to be built from
  scratch, but the transpiler and macro systems already had the necessary
  infrastructure.
- **Solution**: The `rust_type_to_wgsl_string()` function already passes through
  unknown type names as-is, `is_custom_type()` already identifies non-primitive
  types, and `generate_wgsl()` already queries `ShaderType::wgsl_type_definition()`
  for custom types. The primary work was validation and testing.
- **Pattern**: When a story's implementation path looks surprisingly clear, it may
  be because prior stories (GUP-061, GUP-064-B) already laid the groundwork.
  Always check existing code before assuming new code is needed.

#### Field Access Transpilation for Uniform vs Input Parameters

- **Challenge**: Custom structs used as uniform parameters need field access to
  go through `uniforms.param_name.field_name`, while input parameters use direct
  `param_name.field_name`.
- **Solution**: The `RustToWgsl` converter already distinguishes between uniform
  params (in the `uniform_params` HashSet) and the input param. The converter
  wraps uniform parameter references with `uniforms.` prefix, and field access on
  uniform params becomes `uniforms.param.field`.
- **Pattern**: The uniform parameter transformation is a two-level operation:
  bare reference `config` → `uniforms.config`, and field access `config.scale` →
  `uniforms.config.scale`.

#### bytemuck Compatibility for Embedded Structs

- **Challenge**: The generated uniform struct embeds custom types directly, so
  they must satisfy `bytemuck::Pod + Zeroable`.
- **Solution**: `#[derive(WgslStruct)]` already requires and derives these traits,
  so any type usable as a `WgslStruct` is automatically compatible as a uniform
  struct field.
- **Pattern**: The `WgslStruct` derive macro enforces both `#[repr(C)]` and the
  bytemuck traits, making the type system enforce GPU compatibility at compile
  time.

### Architectural Decisions

#### Validation-Only Story

- **Decision**: Implement as a validation and test story rather than a feature
  development story.
- **Reasoning**: The existing infrastructure already supported custom structs.
  Adding tests and documentation was more valuable than refactoring working code.
- **Trade-off**: No new APIs or features were added, but confidence in the system
  was significantly increased through comprehensive testing.
- **Future**: This pattern of "validation stories" should be considered when a
  feature is believed to work but lacks test coverage.

### Development Workflow Insights

- The `mask all-fix` workflow works well for keeping code clean.
- GPU compilation tests (`validate_wgsl_compiles`) are valuable for catching
  issues that Rust's type system can't detect — they validate the generated WGSL
  actually compiles on the GPU.
- Making test struct types `pub` is required since `#[shader_fn]` generates
  public trait implementations that reference the types.

### Follow-up Stories

No follow-up stories identified — the custom struct support is complete and
well-tested.
