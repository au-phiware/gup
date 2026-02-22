# GUP-064-B: Custom Struct Code Generation for WGSL

**Status**: 📋 Planned  
**Priority**: Low  
**Estimated Effort**: 2-3 days  
**Prerequisites**: GUP-064 (Complete)

## Problem Statement

The current `#[wgsl_function]` macro supports all WGSL built-in types (matrices, textures, samplers), but does not automatically generate WGSL struct definitions from Rust structs. Users must manually define WGSL structs and keep them in sync with Rust definitions.

## Goals

Enable automatic WGSL struct generation from Rust types with proper GPU alignment:

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
    return color * mat.albedo * mat.metallic;
}
```

## User Story

**As a** shader developer  
**I want** to define custom structs in Rust and have them automatically work in WGSL  
**So that** I don't have to manually maintain parallel struct definitions

## Acceptance Criteria

### AC1: WgslStruct Derive Macro

- [ ] New `#[derive(WgslStruct)]` proc-macro in gup-macros
- [ ] Analyzes Rust struct fields and types
- [ ] Generates WGSL struct definition with proper field ordering
- [ ] Validates that struct implements `Pod + Zeroable`
- [ ] Provides clear errors for unsupported field types

### AC2: Integration with wgsl_function

- [ ] `#[wgsl_function]` recognizes WgslStruct-derived types
- [ ] Auto-includes struct definition in generated WGSL
- [ ] Supports nested structs (struct fields that are also WgslStructs)
- [ ] Handles proper WGSL naming (converts snake_case to PascalCase if needed)

### AC3: GPU Alignment Validation

- [ ] Validates field alignment matches GPU requirements
- [ ] Detects and reports alignment issues at compile time
- [ ] Suggests padding fields when needed
- [ ] Warns about inefficient layouts (excessive padding)

## Technical Approach

### 1. WgslStruct Derive Macro

Create a new proc-macro that:

1. Parses the struct definition using syn
2. Validates each field type is WGSL-compatible
3. Calculates GPU memory layout with proper alignment
4. Generates WGSL struct definition as a string constant
5. Implements a `WgslStructType` trait for discovery

```rust
pub trait WgslStructType {
    fn wgsl_struct_definition() -> &'static str;
    fn struct_name() -> &'static str;
}
```

### 2. Modified wgsl_function Macro

Update `rust_type_to_wgsl_type()` to:

1. Check if type implements `WgslStructType`
2. If yes, include its WGSL definition in output
3. Track included structs to avoid duplicates
4. Recursively include nested struct definitions

### 3. Validation Strategy

**At Macro Expansion Time**:
- Check `#[repr(C)]` attribute present
- Validate field types are WGSL-compatible
- Calculate expected memory layout

**At Compile Time**:
- Verify `Pod + Zeroable` implementation
- Check struct size matches expected layout

## Implementation Plan

### Phase 1: WgslStruct Derive Macro (1 day)

- [ ] Create derive macro skeleton in gup-macros
- [ ] Parse struct fields and types
- [ ] Generate basic WGSL struct definition
- [ ] Add WgslStructType trait implementation

### Phase 2: Integration with wgsl_function (1 day)

- [ ] Update rust_type_to_wgsl_type() to detect WgslStructType
- [ ] Auto-include struct definitions in WGSL output
- [ ] Handle nested structs
- [ ] Prevent duplicate struct definitions

### Phase 3: Validation and Polish (1 day)

- [ ] Add GPU alignment validation
- [ ] Comprehensive error messages
- [ ] Documentation and examples
- [ ] Integration tests with GPU compilation

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_derive_simple_struct() {
    #[derive(WgslStruct)]
    #[repr(C)]
    struct Simple {
        x: f32,
        y: f32,
    }
    
    let wgsl = Simple::wgsl_struct_definition();
    assert!(wgsl.contains("struct Simple"));
    assert!(wgsl.contains("x: f32"));
}

#[test]
fn test_derive_with_vectors() {
    #[derive(WgslStruct)]
    #[repr(C)]
    struct WithVectors {
        position: Vec3,
        color: Vec4,
    }
    
    let wgsl = WithVectors::wgsl_struct_definition();
    assert!(wgsl.contains("position: vec3<f32>"));
}
```

### Integration Tests

```rust
#[derive(WgslStruct)]
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Material {
    albedo: Vec3,
    metallic: f32,
    roughness: f32,
    padding: [f32; 3],
}

#[wgsl_function]
fn shade(color: Vec3, mat: Material) -> Vec3 {
    return color * mat.albedo * mat.metallic;
}

#[test]
fn test_custom_struct_in_shader() {
    let material = Material {
        albedo: vec3![1.0, 0.5, 0.0],
        metallic: 0.8,
        roughness: 0.2,
        padding: [0.0; 3],
    };
    
    let func = Shade::new(material);
    let wgsl = Shade::wgsl_function();
    
    // Should include struct definition
    assert!(wgsl.contains("struct Material"));
    assert!(wgsl.contains("albedo: vec3<f32>"));
    
    // Should compile on GPU
    let context = GupContext::headless().await?;
    context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
}
```

## Success Metrics

- [ ] Derive macro generates valid WGSL for all supported field types
- [ ] Integration with `#[wgsl_function]` is seamless
- [ ] Nested structs work correctly
- [ ] Alignment validation catches common issues
- [ ] Generated WGSL compiles on GPU
- [ ] Documentation includes migration guide from manual structs

## Risk Assessment

### Technical Risks

- **Medium**: Syn parsing complexity for nested types
- **Low**: GPU alignment calculation accuracy
- **Low**: Integration with existing macro code

### Mitigation Strategies

- Start with simple flat structs, add nesting later
- Test alignment with actual GPU buffers
- Keep derive macro separate from wgsl_function initially

## Definition of Done

- [ ] `#[derive(WgslStruct)]` macro implemented and tested
- [ ] Integration with `#[wgsl_function]` works
- [ ] Nested structs supported
- [ ] Alignment validation functional
- [ ] All unit and integration tests pass
- [ ] Documentation with examples
- [ ] GPU compilation validation tests pass

## Related Stories

- **GUP-064**: Advanced Type System Support (prerequisite)
- **GUP-006**: WGSL Function Macro (foundation)

## Notes

- This is a "nice-to-have" feature that improves developer experience
- Manual WGSL struct definitions remain supported
- Focus on common cases first (flat structs with basic types)
- Advanced features (unions, enums) can be future enhancements
