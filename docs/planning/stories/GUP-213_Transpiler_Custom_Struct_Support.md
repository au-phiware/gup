# GUP-213: Transpiler Custom Struct Support

## Story Overview

**Title**: Custom Struct Parameter Support in `#[shader_fn]`  
**Epic**: Phase 2 Initiative 3 - Rust-to-WGSL Transpilation Research  
**Priority**: Low  
**Story Points**: 5  
**Status**: 🚧 In Progress

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

- [ ] `#[shader_fn]` functions accept parameters of types decorated with
      `#[wgsl_struct]`
- [ ] The generated WGSL includes the struct definition
- [ ] Field access on custom struct parameters transpiles correctly
- [ ] Custom structs work as both input and uniform parameters
- [ ] Memory layout alignment is correct per WGSL spec

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

- [ ] Custom struct parameters supported in `#[shader_fn]`
- [ ] WGSL struct definitions generated correctly
- [ ] Field access transpilation working
- [ ] GPU compilation tests passing
- [ ] Integration tests with existing shader pipeline
