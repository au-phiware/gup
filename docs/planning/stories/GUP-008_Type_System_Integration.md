# GUP-008: Type System Integration

## Story Overview

**Title**: Implement Comprehensive Type System Integration  
**Epic**: Phase 1 Initiative 2 - Unified Shader Function System  
**Priority**: Critical  
**Story Points**: 8  

## Context

Type system integration ensures that Rust's compile-time type checking validates shader function composition. This prevents runtime errors and provides clear feedback when shader functions are incompatibly composed. The system must map Rust types to WGSL types and validate compositions through trait bounds.

## User Story

**As a** visualization developer  
**I want** Rust's type system to validate my shader function compositions  
**So that** I catch type mismatches at compile time rather than runtime and get clear error messages for invalid compositions  

## Acceptance Criteria

### Type Safety Features

- [ ] **Compile-Time Validation**: Invalid shader function compositions caught at compile time
- [ ] **Clear Error Messages**: Helpful error messages explaining type mismatches
- [ ] **Type Inference**: Automatic type inference for common composition patterns
- [ ] **WGSL Mapping**: Accurate mapping between Rust types and WGSL types

### Core Type Traits

```rust
pub trait ShaderType: Clone + Send + Sync + 'static {
    // WGSL type representation
    fn wgsl_type_name() -> &'static str;
    fn wgsl_type_definition() -> Option<&'static str> { None }
    
    // Memory layout information
    fn size_bytes() -> usize;
    fn alignment() -> usize;
    
    // Composition compatibility
    fn is_compatible_with<T: ShaderType>() -> bool { 
        Self::wgsl_type_name() == T::wgsl_type_name() 
    }
}

pub trait Compatible<T: ShaderType>: ShaderType {
    // Marker trait for compatible types
}
```

### Type Validation

- [ ] **Automatic Compatibility**: Common type conversions (f32 -> vec2<f32>) handled automatically
- [ ] **Explicit Conversions**: Clear syntax for non-automatic type conversions
- [ ] **Struct Validation**: Custom struct types validated for WGSL compatibility
- [ ] **Array Support**: Array types with proper size and element type validation

## Technical Tasks

### 1. Core Type System

- [ ] Define ShaderType trait with essential methods
- [ ] Implement Compatible trait for type validation
- [ ] Create automatic type compatibility rules
- [ ] Add support for custom type definitions

### 2. Primitive Type Implementations

- [ ] Implement ShaderType for f32, i32, u32, bool
- [ ] Add vector types: Vec2, Vec3, Vec4
- [ ] Implement matrix types: Mat2, Mat3, Mat4
- [ ] Create array type support with const generics

### 3. Custom Type Support

- [ ] Create derive macro for ShaderType on custom structs
- [ ] Validate struct field types are WGSL-compatible
- [ ] Generate WGSL struct definitions automatically
- [ ] Handle struct alignment and padding requirements

### 4. Composition Validation

- [ ] Implement compilation-time validation for function chains
- [ ] Create helpful error messages for type mismatches
- [ ] Add suggestion system for common type conversion mistakes
- [ ] Support for generic shader functions with type constraints

## Detailed Requirements

### Primitive Type Implementations

```rust
impl ShaderType for f32 {
    fn wgsl_type_name() -> &'static str { "f32" }
    fn size_bytes() -> usize { 4 }
    fn alignment() -> usize { 4 }
}

impl ShaderType for Vec2 {
    fn wgsl_type_name() -> &'static str { "vec2<f32>" }
    fn size_bytes() -> usize { 8 }
    fn alignment() -> usize { 8 }
}

impl ShaderType for Vec4 {
    fn wgsl_type_name() -> &'static str { "vec4<f32>" }
    fn size_bytes() -> usize { 16 }
    fn alignment() -> usize { 16 }
}

// Automatic compatibility for safe conversions
impl Compatible<Vec2> for f32 {}  // f32 can be expanded to Vec2(x, x)
impl Compatible<Vec3> for f32 {}  // f32 can be expanded to Vec3(x, x, x)
impl Compatible<Vec4> for f32 {}  // f32 can be expanded to Vec4(x, x, x, x)
```

### Custom Type Derive Macro

```rust
#[derive(ShaderType)]
struct WeatherData {
    #[shader_type(f32)]
    longitude: f32,
    
    #[shader_type(f32)]
    latitude: f32,
    
    #[shader_type(f32)]
    temperature: f32,
    
    #[shader_type(vec3<f32>)]
    wind_vector: Vec3,
}

// Generated implementation:
impl ShaderType for WeatherData {
    fn wgsl_type_name() -> &'static str { "WeatherData" }
    
    fn wgsl_type_definition() -> Option<&'static str> {
        Some(r#"
        struct WeatherData {
            longitude: f32,
            latitude: f32,
            temperature: f32,
            wind_vector: vec3<f32>,
        }
        "#)
    }
    
    fn size_bytes() -> usize { 20 } // 4 + 4 + 4 + 12, with padding
    fn alignment() -> usize { 16 }  // vec3 alignment requirement
}
```

### Composition Validation

```rust
// Successful composition (types match)
fn valid_composition() {
    let position_func: impl ShaderFunction<Input=WeatherData, Output=Vec2> = get_position;
    let color_func: impl ShaderFunction<Input=Vec2, Output=Vec4> = map_color;
    
    // This compiles successfully
    let composed = position_func.compose(color_func);
}

// Failed composition (types don't match)
fn invalid_composition() {
    let position_func: impl ShaderFunction<Input=WeatherData, Output=Vec2> = get_position;
    let scale_func: impl ShaderFunction<Input=f32, Output=f32> = linear_scale;
    
    // This fails to compile with clear error message:
    // "Cannot compose functions: Output type Vec2 is not compatible with Input type f32"
    let composed = position_func.compose(scale_func); // ❌ Compile error
}
```

### Generic Function Support

```rust
// Generic shader function with type constraints
pub struct VectorScale<T: VectorType> {
    scale_factor: f32,
    _phantom: PhantomData<T>,
}

impl<T: VectorType> ShaderFunction for VectorScale<T> {
    type Input = T;
    type Output = T;
    type Uniforms = VectorScaleUniforms;
    
    fn wgsl_function() -> &'static str {
        // Generate WGSL based on T's type information
        &format!("fn vector_scale(input: {}) -> {} {{ ... }}", 
                T::wgsl_type_name(), 
                T::wgsl_type_name())
    }
}

// Type constraint trait
pub trait VectorType: ShaderType + Mul<f32, Output=Self> {}
impl VectorType for Vec2 {}
impl VectorType for Vec3 {}
impl VectorType for Vec4 {}
```

## Dependencies

### Prerequisite Stories

- GUP-005: Shader Function Trait (defines types being integrated)

### Enables Stories

- GUP-006: WGSL Function Macro (uses type system for validation)
- GUP-007: Shader Pipeline Builder (relies on type information)
- All shader function implementations

## Testing Strategy

### Compile-Time Tests

```rust
// These tests validate that code compiles or fails to compile as expected

#[test]
fn test_valid_compositions_compile() {
    // Valid composition should compile
    let _valid = LinearScale::new(0.0, 1.0, 0.0, 1.0)
        .compose(ColorMap::new(palette))
        .compose(AlphaBlend::new(0.8));
}

// Compile-fail test (using trybuild or similar)
#[test]
fn test_invalid_compositions_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/invalid-composition.rs");
}
```

### Type Compatibility Tests

```rust
#[test]
fn test_primitive_type_compatibility() {
    assert!(f32::is_compatible_with::<f32>());
    assert!(f32::is_compatible_with::<Vec2>());
    assert!(!Vec4::is_compatible_with::<f32>());
}

#[test]
fn test_custom_type_generation() {
    #[derive(ShaderType)]
    struct TestData {
        x: f32,
        y: f32,
    }
    
    assert_eq!(TestData::wgsl_type_name(), "TestData");
    assert!(TestData::wgsl_type_definition().is_some());
}
```

### WGSL Generation Tests

```rust
#[test]
fn test_wgsl_type_mapping() {
    assert_eq!(f32::wgsl_type_name(), "f32");
    assert_eq!(Vec2::wgsl_type_name(), "vec2<f32>");
    assert_eq!(Mat4::wgsl_type_name(), "mat4x4<f32>");
}

#[test]
fn test_struct_definition_generation() {
    #[derive(ShaderType)]
    struct ComplexData {
        position: Vec3,
        color: Vec4,
        intensity: f32,
    }
    
    let definition = ComplexData::wgsl_type_definition().unwrap();
    assert!(definition.contains("struct ComplexData"));
    assert!(definition.contains("position: vec3<f32>"));
    assert!(definition.contains("color: vec4<f32>"));
    assert!(definition.contains("intensity: f32"));
}
```

### Error Message Tests

```rust
#[test]
fn test_clear_error_messages() {
    // Use a test harness to capture compiler error messages
    let error_output = capture_compiler_error(r#"
        let position_func = PositionTransform::new(); // Vec2 output
        let color_func = ColorScale::new();           // f32 input
        let invalid = position_func.compose(color_func);
    "#);
    
    assert!(error_output.contains("type mismatch"));
    assert!(error_output.contains("Vec2"));
    assert!(error_output.contains("f32"));
    assert!(error_output.contains("not compatible"));
}
```

## Success Metrics

### Type Safety Requirements

- [ ] **Compile-Time Validation**: 100% of type mismatches caught at compile time
- [ ] **Error Message Quality**: Error messages include type names and suggestions
- [ ] **Zero Runtime Overhead**: Type validation adds no runtime cost
- [ ] **WGSL Accuracy**: Generated WGSL types match Rust type definitions exactly

### Developer Experience Requirements

- [ ] **IDE Support**: Full autocomplete and error highlighting in IDEs
- [ ] **Documentation**: Clear examples of type usage and conversion patterns
- [ ] **Migration Support**: Clear guidance for converting between compatible types
- [ ] **Performance**: Type checking adds <10% to compilation time

## Risk Assessment

### Technical Risks

- **Medium**: Complex type relationships could make error messages confusing
- **Medium**: WGSL type system differences might not map cleanly to Rust
- **Low**: Generic type support might be overly complex

### Mitigation Strategies

- **Error Message Focus**: Prioritize clear, actionable error messages
- **Incremental Implementation**: Start with simple types, add complexity gradually
- **User Testing**: Get feedback on error message clarity from developers

## Implementation Notes

### Design Decisions

- Use marker traits (Compatible<T>) for type validation rather than runtime checks
- Generate WGSL struct definitions automatically from Rust struct definitions
- Prioritize clear error messages over type system flexibility
- Use const generics for array types to maintain size information

### WGSL Mapping Strategy

- Direct mapping for primitive types (f32 -> f32, Vec3 -> vec3<f32>)
- Automatic struct generation with proper field ordering
- Respect WGSL alignment rules in generated structs
- Handle WGSL reserved keywords through automatic renaming

### Error Handling Strategy

- Custom trait implementations to provide better error messages
- Include type information in error context
- Suggest common fixes for type mismatches
- Link to documentation for complex type relationships

## Definition of Done

- [ ] ShaderType trait implemented for all common types
- [ ] Compilation-time validation catches type mismatches
- [ ] Error messages are clear and actionable
- [ ] Custom type derive macro generates correct WGSL
- [ ] Generic function support working with type constraints
- [ ] Cross-platform type compatibility verified
- [ ] Performance impact on compilation is acceptable
- [ ] Documentation includes comprehensive type usage examples
- [ ] Code review completed and approved
