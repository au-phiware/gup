# GUP-056: Rust-to-WGSL Type System Mapping

## Story Overview

**Title**: Implement Comprehensive Type System Mapping Between Rust and WGSL  
**Epic**: Phase 2 Initiative 2 - Rust-to-WGSL Transpilation  
**Priority**: High  
**Story Points**: 8  
**Status**: ✅ Complete (2025-07-19)

## Context

Building on the research from GUP-055, we need to implement a robust type system
that can accurately map Rust types to their WGSL equivalents while handling
alignment, layout, and semantic differences between the languages.

## User Story

**As a** shader function developer  
**I want** automatic type mapping between Rust and WGSL  
**So that** I can write shader functions using natural Rust types without
worrying about GPU-specific type constraints

## Problem Statement

WGSL has different type semantics, memory layout requirements, and naming
conventions compared to Rust. We need a system that can:

- Map Rust primitive types to WGSL equivalents
- Handle vector and matrix type conversions
- Manage struct layout and alignment differences
- Provide clear error messages for unsupported types

## Acceptance Criteria

### AC1: Primitive Type Mapping

- [x] Map Rust primitives (i32, u32, f32, bool) to WGSL types
- [x] Handle type size and alignment requirements
- [x] Support explicit type annotations for ambiguous cases
- [x] Validate type compatibility across function boundaries

### AC2: Composite Type Support

- [x] Implement vector type mapping (Vec2, Vec3, Vec4)
- [x] Support matrix types (Mat2x2, Mat3x3, Mat4x4, etc.)
- [x] Handle array types with proper WGSL syntax
- [x] Support nested struct definitions

### AC3: Memory Layout Compatibility

- [x] Ensure proper alignment for uniform buffer layouts
- [x] Handle padding requirements for WGSL structs
- [x] Validate bytemuck compatibility for existing buffer system
- [x] Support explicit layout attributes where needed

### AC4: Error Handling and Diagnostics

- [x] Provide clear error messages for unsupported types
- [x] Suggest corrections for common type mapping issues
- [x] Validate type consistency in function signatures
- [x] Report alignment and layout warnings

## Technical Requirements

### Type Mapping Table

| Rust Type       | WGSL Type     | Notes                 |
| --------------- | ------------- | --------------------- |
| `bool`          | `bool`        | Direct mapping        |
| `i32`           | `i32`         | Direct mapping        |
| `u32`           | `u32`         | Direct mapping        |
| `f32`           | `f32`         | Direct mapping        |
| `f64`           | Error         | Not supported in WGSL |
| `[f32; 2]`      | `vec2<f32>`   | Convert to vector     |
| `[f32; 3]`      | `vec3<f32>`   | Convert to vector     |
| `[f32; 4]`      | `vec4<f32>`   | Convert to vector     |
| `[[f32; 2]; 2]` | `mat2x2<f32>` | Convert to matrix     |
| Custom structs  | WGSL struct   | With proper alignment |

### Implementation Architecture

```rust
// Core type mapping trait
pub trait WgslTypeMapping {
    fn wgsl_type_name() -> &'static str;
    fn wgsl_type_definition() -> Option<String>;
    fn validate_gpu_compatibility() -> Result<(), TypeMappingError>;
    fn memory_layout() -> MemoryLayout;
}

// Type mapping context for complex conversions
pub struct TypeMapper {
    type_cache: HashMap<TypeId, WgslTypeInfo>,
    struct_definitions: Vec<String>,
    layout_validator: LayoutValidator,
}

impl TypeMapper {
    pub fn map_rust_type(&mut self, ty: &Type) -> Result<WgslTypeInfo, TypeMappingError>;
    pub fn generate_struct_definitions(&self) -> String;
    pub fn validate_function_signature(&self, sig: &Signature) -> Result<(), TypeError>;
}
```

### Supported Type Categories

1. **Primitive Types**

   ```rust
   // Supported
   let value: f32 = 1.0;
   let count: u32 = 42;
   let flag: bool = true;

   // Not supported (with clear error messages)
   let precise: f64 = 1.0;  // Error: f64 not supported in WGSL
   let text: &str = "hello"; // Error: strings not supported in shaders
   ```

2. **Vector Types**

   ```rust
   // Using array syntax (auto-converted)
   let pos: [f32; 2] = [1.0, 2.0];      // -> vec2<f32>
   let color: [f32; 4] = [1.0, 0.0, 0.0, 1.0]; // -> vec4<f32>

   // Using dedicated vector types
   let pos: Vec2 = Vec2::new(1.0, 2.0); // -> vec2<f32>
   let color: Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0); // -> vec4<f32>
   ```

3. **Struct Types**

   ```rust
   // Automatic WGSL struct generation
   struct MaterialProperties {
       diffuse: [f32; 3],    // -> vec3<f32>
       roughness: f32,       // -> f32
       metallic: f32,        // -> f32
       // Automatic padding handled
   }
   ```

### Memory Layout Validation

```rust
// Ensure compatibility with existing uniform buffer system
#[repr(C)]
#[derive(WgslTypeMapping)]
struct TransformUniforms {
    #[wgsl(vec3)]
    position: [f32; 3],
    #[wgsl(vec4)]
    rotation: [f32; 4],
    #[wgsl(f32)]
    scale: f32,
    // Automatic padding: _pad: [u8; 12]
}
```

## Dependencies

- GUP-055: Research and prototype foundation
- syn crate: For parsing Rust type definitions
- quote crate: For generating WGSL type definitions
- Existing ShaderType trait: For integration with current system

## Definition of Done

- [x] Complete type mapping system for all supported Rust types
- [x] Integration with existing ShaderType trait system
- [x] Comprehensive test suite covering all type mappings
- [x] Clear error messages for unsupported type combinations
- [x] Documentation with examples for all supported types
- [x] Performance benchmarks showing minimal overhead

## Test Requirements

### Unit Tests

```rust
#[test]
fn test_primitive_type_mapping() {
    assert_eq!(f32::wgsl_type_name(), "f32");
    assert_eq!(u32::wgsl_type_name(), "u32");
    assert_eq!(bool::wgsl_type_name(), "bool");
}

#[test]
fn test_vector_type_mapping() {
    assert_eq!(<[f32; 3]>::wgsl_type_name(), "vec3<f32>");
    assert_eq!(<[i32; 2]>::wgsl_type_name(), "vec2<i32>");
}

#[test]
fn test_struct_generation() {
    let mapper = TypeMapper::new();
    let wgsl = mapper.generate_struct_for::<MaterialProperties>();
    assert!(wgsl.contains("struct MaterialProperties"));
    assert!(wgsl.contains("diffuse: vec3<f32>"));
}
```

### Integration Tests

```rust
#[test]
fn test_uniform_buffer_compatibility() {
    // Ensure generated types work with existing buffer system
    let uniforms = TransformUniforms::default();
    let buffer = UniformBuffer::new();
    buffer.upload(&device, &queue, &uniforms).unwrap();
}
```

## Future Considerations

This implementation enables:

- GUP-057: Expression and operator transpilation with proper type checking
- GUP-058: Control flow handling with type-aware variable management
- Advanced generic type support in future stories
- Integration with Rust's type inference system

## Implementation Summary

### What Was Implemented

1. **TypeMapper** (`gup-macros/src/transpile/type_map.rs`): Central type mapping
   system that converts Rust `syn::Type` nodes to WGSL types with memory layout
   tracking, struct registration, and comprehensive error diagnostics.

2. **Comprehensive Type Table**: 28 known type mappings covering:
   - 4 scalar types: `f32`, `i32`, `u32`, `bool`
   - 12 vector types: `Vec2-4`, `IVec2-4`, `UVec2-4`, `BVec2-4`
   - 9 matrix types: `Mat2-4` (square) + 6 non-square variants
   - Array types with literal lengths
   - Custom struct types with field mapping

3. **Memory Layout System**: `MemoryLayout` and `wgsl_type_layout()` computing
   size and alignment per WGSL specification rules (including vec3 16-byte
   alignment). Struct layout computation with proper padding.

4. **Error Diagnostics**: `TypeMappingError` with 7 error kinds, 15 explicitly
   unsupported types with specific error messages and suggestions, complex path
   detection, and invalid struct field reporting.

5. **Converter Integration**: `RustToWgsl` now delegates to `TypeMapper` instead
   of inline type matching, gaining broader type support and better error
   messages. Added integer/unsigned/boolean vector constructor support.

6. **Function Signature Validation**: `validate_function_signature()` checks all
   parameter and return types for WGSL compatibility.

### Key Files Changed

| File                                                     | Change                            |
| -------------------------------------------------------- | --------------------------------- |
| `gup-macros/src/transpile/type_map.rs`                   | New — core type mapping module    |
| `gup-macros/src/transpile/type_map_integration_tests.rs` | New — 24 integration tests        |
| `gup-macros/src/transpile/convert.rs`                    | Updated to delegate to TypeMapper |
| `gup-macros/src/transpile/mod.rs`                        | Updated docs, module registration |

### Test Counts

- 42 unit tests in `type_map.rs`
- 24 integration tests in `type_map_integration_tests.rs`
- All existing 17 pipeline tests continue to pass
- All 8 WGSL validation tests continue to pass
