# GUP-064: Advanced Type System Support for WGSL Macros

**Status**: ✅ Complete (Partial)  
**Completed**: 2025-01-27
**Priority**: Low  
**Estimated Effort**: 2-3 days  
**Actual Effort**: 0.5 days
**Prerequisites**: GUP-006 (Complete)

## Implementation Summary

Successfully implemented Phase 1 (Extended Built-in Types) with comprehensive support for:

**Type Additions**:
- All non-square matrix types: Mat2x3, Mat2x4, Mat3x2, Mat3x4, Mat4x2, Mat4x3
- Texture types: Texture1D, Texture2D, Texture3D, TextureCube, Texture2DArray, TextureCubeArray, TextureMultisampled2D
- Storage texture types: TextureStorage1D, TextureStorage2D, TextureStorage3D
- Sampler types: Sampler, SamplerComparison

**Testing**:
- Added 3 new unit tests covering all matrix and texture type mappings (18 total tests passing)
- Added integration tests for matrix types with GPU compilation validation
- All tests pass with WGSL compilation confirmed on GPU

**Technical Notes**:
- Non-square matrix types map to WGSL but don't have Rust equivalents in glam (users can define custom types)
- Texture/sampler types properly flagged as non-uniform-compatible (must use bindings)
- Matrix types work as both input parameters and uniform parameters
- Custom struct support deferred to follow-up story (GUP-064-B)

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

- [x] Add comprehensive matrix type support
- [x] Implement texture and sampler type mapping  
- [x] Add validation for new built-in types

### Phase 2: Custom Struct Support (1-2 days)

- [ ] Design custom struct parameter parsing
- [ ] Implement struct field validation
- [ ] Add WGSL struct definition generation

**Note**: Custom struct support deferred to future story (see Follow-up Stories section). Current implementation allows custom type names to pass through for manual WGSL struct definitions.

### Phase 3: Advanced Arrays and Validation (1 day)

- [x] Enhanced array type support with size validation (already in GUP-006)
- [ ] Multi-dimensional array support
- [x] Comprehensive type compatibility checking

## Success Criteria

### Must Have

- [x] Support for all standard WGSL built-in types
- [ ] Custom struct parameters work correctly (deferred - requires significant refactoring)
- [x] All new types generate valid WGSL definitions
- [x] Comprehensive validation prevents invalid type usage

### Should Have

- [x] Matrix operations and transformations supported
- [x] Texture sampling functions work correctly (type mapping in place)
- [ ] Complex nested type structures supported (deferred)

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

## Retrospective

**Completed**: 2025-01-27

### Key Technical Learnings

#### Extended Type System for WGSL
- **Challenge**: WGSL supports many more types than the initial macro implementation
- **Solution**: Extended `rust_type_to_wgsl_type()` to map all matrix variants, textures, and samplers
- **Pattern**: Simple string mapping for type conversion with validation in separate function

#### Type Validation Strategy
- **Challenge**: Different types have different constraints (uniforms vs bindings)
- **Solution**: Separate validation function `is_uniform_compatible_type()` that checks GPU uniform compatibility
- **Trade-off**: Texture/sampler types correctly identified as non-uniform-compatible but still allowed for future binding support

#### Non-Square Matrix Support
- **Challenge**: glam doesn't provide Mat2x3, Mat3x2, etc.
- **Solution**: Macro maps type names to WGSL equivalents; users can define custom Rust types with matching names
- **Pattern**: Allow type names to pass through for manual struct definitions

### Architectural Decisions

#### Phase 1 Only Implementation
- **Decision**: Implement only extended built-in types (Phase 1), defer custom structs to future story
- **Reasoning**: Custom struct support requires:
  1. Parsing Rust struct definitions to generate WGSL structs
  2. Complex field validation and alignment calculation
  3. Reflection or proc-macro attribute cooperation
  4. Significant refactoring of validation logic
- **Trade-off**: Users must manually define WGSL structs for custom types, but all built-in types fully supported
- **Future**: Create GUP-064-B for proper custom struct codegen

#### Comprehensive Test Coverage
- **Decision**: Add both unit tests and integration tests with GPU compilation
- **Reasoning**: Type mapping failures only show up at WGSL compilation time
- **Pattern**: Unit tests for type mapping, integration tests for end-to-end validation

### Development Workflow Insights

- **Rapid Implementation**: Phase 1 completed in 0.5 days vs 3-day estimate due to focused scope
- **Test-Driven**: Created failing tests first, then implemented type mappings
- **GPU Validation**: Integration tests that compile actual WGSL caught issues early
- **Documentation**: Clear error messages and test names make debugging straightforward

### Follow-up Stories

#### GUP-064-B: Custom Struct Code Generation
**Priority**: Low (nice-to-have feature)

Support automatic WGSL struct generation from Rust types:

```rust
#[derive(WgslStruct)]  // New derive macro
#[repr(C)]
struct MaterialProps {
    albedo: Vec3,
    metallic: f32,
    roughness: f32,
}

#[wgsl_function]
fn apply_material(color: Vec3, mat: MaterialProps) -> Vec3 {
    // Macro auto-generates WGSL struct definition
}
```

**Technical Requirements**:
- New proc-macro `#[derive(WgslStruct)]` to analyze struct layout
- Field-by-field WGSL type generation with alignment validation
- Integration with existing `#[wgsl_function]` macro
- Validation that derived structs implement Pod + Zeroable

**Estimated Effort**: 2-3 days
**Dependencies**: None (can build on GUP-064 work)

#### GUP-064-C: Advanced Array Types
**Priority**: Very Low

Support multi-dimensional arrays and dynamic-sized arrays:

```rust
#[wgsl_function]
fn process_grid(data: [[f32; 4]; 4]) -> f32 {
    // 2D array support
}
```

**Estimated Effort**: 1 day

