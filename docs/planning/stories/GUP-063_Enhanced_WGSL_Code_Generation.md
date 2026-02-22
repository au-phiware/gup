# GUP-063: Enhanced WGSL Code Generation

**Status**: 🚧 In Progress  
**Started**: 2025-01-08  
**Priority**: Medium  
**Estimated Effort**: 3-5 days  
**Prerequisites**: GUP-006 (Complete)

## Problem Statement

The current `#[wgsl_function]` procedural macro generates placeholder WGSL code
that lacks struct definitions and complete function implementations. This
prevents the generated WGSL from being compiled directly on the GPU, limiting
the macro's usefulness for actual shader execution.

## Current Limitations

1. **Missing Struct Definitions**: Generated WGSL references uniform structs
   that aren't defined
2. **Placeholder Function Bodies**: Functions contain TODO comments instead of
   actual WGSL implementations
3. **No Type Definitions**: Custom types used in functions aren't generated in
   WGSL
4. **Limited WGSL Validation**: No compile-time verification that generated WGSL
   is valid

## Goals

### Primary Goals

- Generate complete, compilable WGSL code including struct definitions
- Add support for parsing actual WGSL function bodies from Rust syntax
- Implement WGSL type definition generation for custom structs
- Provide compile-time WGSL validation during macro expansion

### Secondary Goals

- Support for WGSL-specific features (built-in functions, vertex attributes)
- Automatic WGSL optimization and formatting
- Better error reporting for WGSL compilation failures

## Technical Approach

### 1. WGSL Struct Generation

```rust
// Generate complete uniform struct definitions
fn generate_uniform_struct_wgsl(&self) -> String {
    format!(
        "struct {} {{\n{}\n}}",
        self.uniforms_name,
        self.uniform_params.iter()
            .map(|param| format!("    {}: {}", param.name, param.wgsl_type))
            .collect::<Vec<_>>()
            .join(",\n")
    )
}
```

### 2. Function Body Parsing

- Parse Rust expressions and convert to WGSL syntax
- Support for basic arithmetic, function calls, and control flow
- Validation of WGSL-compatible operations

### 3. Type System Extension

- Generate WGSL type definitions for custom structs
- Support for nested types and arrays
- Proper WGSL alignment and padding generation

### 4. WGSL Validation

- Integration with wgpu's shader validation
- Compile-time checking of generated WGSL
- Clear error messages for invalid shader code

## Implementation Plan

### Phase 1: Complete Struct Generation (1-2 days)

- [ ] Implement uniform struct WGSL generation
- [ ] Add proper WGSL type mapping for all supported types
- [ ] Generate complete shader module with all dependencies

### Phase 2: Function Body Parsing (2-3 days)

- [ ] Design Rust-to-WGSL expression translation
- [ ] Implement basic arithmetic and function call translation
- [ ] Add support for WGSL built-in functions

### Phase 3: WGSL Validation (1 day)

- [ ] Integrate with wgpu shader compilation
- [ ] Add compile-time validation of generated WGSL
- [ ] Improve error reporting for shader compilation failures

## Success Criteria

### Must Have

- [ ] Generated WGSL compiles successfully with wgpu
- [ ] All uniform structs have proper WGSL definitions
- [ ] Basic function bodies translate correctly from Rust to WGSL
- [ ] Integration tests pass with actual GPU compilation

### Should Have

- [ ] Support for common WGSL built-in functions
- [ ] Clear error messages for unsupported Rust syntax
- [ ] Performance equivalent to hand-written WGSL

### Could Have

- [ ] Advanced WGSL optimizations
- [ ] Support for complex control flow (loops, conditionals)
- [ ] Integration with WGSL debugging tools

## Testing Strategy

### Unit Tests

- Test WGSL struct generation for various type combinations
- Verify Rust-to-WGSL expression translation accuracy
- Test error handling for invalid syntax

### Integration Tests

- GPU compilation tests for generated WGSL
- End-to-end tests with actual shader execution
- Performance benchmarks vs. hand-written WGSL

### Example Tests

```rust
#[wgsl_function]
fn enhanced_linear_scale(value: f32, scale: f32, offset: f32) -> f32 {
    value * scale + offset
}

// Should generate complete, compilable WGSL:
// struct EnhancedLinearScaleUniforms {
//     scale: f32,
//     offset: f32,
// }
//
// fn enhanced_linear_scale(value: f32, uniforms: EnhancedLinearScaleUniforms) -> f32 {
//     return value * uniforms.scale + uniforms.offset;
// }
```

## Related Stories

- **GUP-006**: WGSL Function Macro (prerequisite)
- **GUP-052**: Shader Pipeline Builder (would benefit from this)
- **GUP-053**: Advanced Shader Function Library (would benefit from this)

## Notes

- This builds directly on the foundation laid by GUP-006
- Should maintain backward compatibility with existing macro usage
- Consider performance impact of additional WGSL generation and validation
