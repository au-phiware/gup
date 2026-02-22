# GUP-064: Advanced Type System Support for WGSL Macros

**Status**: 🚧 In Progress  
**Started**: 2025-01-27  
**Priority**: Low  
**Estimated Effort**: 2-3 days  
**Prerequisites**: GUP-006 (Complete)

## Problem Statement

The current `#[wgsl_function]` macro supports only basic types (f32, i32, u32,
Vec2, Vec3, Vec4). Many advanced WGSL and GPU programming scenarios require
support for additional types like matrices, custom structs, arrays, and texture
types.

## Current Limitations

1. **Limited Matrix Support**: Only basic matrix types are supported
2. **No Custom Struct Support**: Cannot use user-defined structs in function
   parameters
3. **Basic Array Support**: Limited array type validation and generation
4. **No Texture Types**: No support for texture sampling and image operations
5. **Missing WGSL Built-ins**: No support for vertex attributes, fragment
   outputs, etc.

## Goals

### Primary Goals

- Extend type system to support all WGSL built-in types
- Add support for custom struct parameters and return types
- Implement comprehensive matrix type support
- Add texture and sampler type support

### Secondary Goals

- Support for WGSL vertex attributes and fragment outputs
- Advanced array and buffer type validation
- Type inference for complex nested structures

## Technical Approach

### 1. Extended Type Mapping

```rust
fn rust_type_to_wgsl_type(ty: &Type) -> Result<String> {
    match ty {
        // Extended matrix support
        "Mat2x3" => Ok("mat2x3<f32>".to_string()),
        "Mat2x4" => Ok("mat2x4<f32>".to_string()),
        "Mat3x2" => Ok("mat3x2<f32>".to_string()),
        "Mat3x4" => Ok("mat3x4<f32>".to_string()),
        "Mat4x2" => Ok("mat4x2<f32>".to_string()),
        "Mat4x3" => Ok("mat4x3<f32>".to_string()),

        // Texture types
        "Texture1D" => Ok("texture_1d<f32>".to_string()),
        "Texture2D" => Ok("texture_2d<f32>".to_string()),
        "Texture3D" => Ok("texture_3d<f32>".to_string()),
        "TextureCube" => Ok("texture_cube<f32>".to_string()),

        // Sampler types
        "Sampler" => Ok("sampler".to_string()),
        "SamplerComparison" => Ok("sampler_comparison".to_string()),

        // Custom struct support
        Type::Path(path) => handle_custom_struct(path),

        // Advanced array support
        Type::Array(array) => handle_advanced_array(array),
    }
}
```

### 2. Custom Struct Integration

```rust
// Support for custom structs as parameters
#[derive(Debug, Clone)]
pub struct MaterialProperties {
    pub albedo: Vec3,
    pub metallic: f32,
    pub roughness: f32,
}

#[wgsl_function]
fn apply_material(base_color: Vec3, material: MaterialProperties) -> Vec3 {
    // Complex material calculations
}
```

### 3. Advanced Validation

- Validate struct field types for GPU compatibility
- Check alignment requirements for complex types
- Ensure all referenced types have proper WGSL definitions

## Implementation Plan

### Phase 1: Extended Built-in Types (1 day)

- [ ] Add comprehensive matrix type support
- [ ] Implement texture and sampler type mapping
- [ ] Add validation for new built-in types

### Phase 2: Custom Struct Support (1-2 days)

- [ ] Design custom struct parameter parsing
- [ ] Implement struct field validation
- [ ] Add WGSL struct definition generation

### Phase 3: Advanced Arrays and Validation (1 day)

- [ ] Enhanced array type support with size validation
- [ ] Multi-dimensional array support
- [ ] Comprehensive type compatibility checking

## Success Criteria

### Must Have

- [ ] Support for all standard WGSL built-in types
- [ ] Custom struct parameters work correctly
- [ ] All new types generate valid WGSL definitions
- [ ] Comprehensive validation prevents invalid type usage

### Should Have

- [ ] Matrix operations and transformations supported
- [ ] Texture sampling functions work correctly
- [ ] Complex nested type structures supported

### Could Have

- [ ] Type inference for complex expressions
- [ ] Automatic texture binding generation
- [ ] Advanced GPU memory layout optimization

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_matrix_types() {
    let mat3x4_type: Type = parse_quote!(Mat3x4);
    assert_eq!(rust_type_to_wgsl_type(&mat3x4_type).unwrap(), "mat3x4<f32>");
}

#[test]
fn test_custom_struct_support() {
    // Test custom struct in function parameters
}

#[test]
fn test_texture_type_mapping() {
    let texture2d_type: Type = parse_quote!(Texture2D);
    assert_eq!(rust_type_to_wgsl_type(&texture2d_type).unwrap(), "texture_2d<f32>");
}
```

### Integration Tests

```rust
#[derive(Debug, Clone)]
#[repr(C)]
struct LightData {
    position: Vec3,
    color: Vec3,
    intensity: f32,
}

#[wgsl_function]
fn calculate_lighting(surface_pos: Vec3, light: LightData) -> Vec3 {
    // Complex lighting calculations
}

#[test]
fn test_lighting_function() {
    let light = LightData {
        position: Vec3::new(0.0, 10.0, 0.0),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 1.0,
    };

    let lighting_func = CalculateLighting::new(light);
    // Test that the function compiles and works correctly
}
```

## Related Stories

- **GUP-006**: WGSL Function Macro (prerequisite)
- **GUP-063**: Enhanced WGSL Code Generation (complementary)
- **GUP-053**: Advanced Shader Function Library (would benefit from this)

## Notes

- Should maintain backward compatibility with existing simple types
- Consider GPU memory alignment for complex types
- Type validation should provide clear error messages for unsupported
  combinations
