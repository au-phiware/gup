# GUP-005: Shader Function Trait

## Story Overview

**Title**: Implement Core ShaderFunction Trait  
**Epic**: Phase 1 Initiative 2 - Unified Shader Function System  
**Priority**: Critical  
**Story Points**: 13  

## Context

The `ShaderFunction` trait is Gup's core innovation - it treats all data transformations as composable WGSL functions that run on the GPU. This trait enables scales, color mappings, coordinate transforms, and custom processing to compose naturally through a unified abstraction. Success here determines the viability of the entire project.

## User Story

**As a** visualization developer  
**I want** all data transformations to be composable GPU functions  
**So that** I can build complex visualizations by naturally combining simple transformations with guaranteed type safety and GPU performance  

## Acceptance Criteria

### Core Trait Definition

```rust
pub trait ShaderFunction {
    type Input: ShaderType;   // What data this function processes
    type Output: ShaderType;  // What it produces
    type Uniforms: bytemuck::Pod + bytemunk::Zeroable = (); // GPU parameters

    // The WGSL code for this function
    fn wgsl_function() -> &'static str;

    // Parameters this function needs
    fn create_uniforms(&self) -> Option<Self::Uniforms> { None }

    // Metadata for composition validation
    fn function_name() -> &'static str;
}
```

### Trait Requirements

- [ ] **Universal Composability**: Any shader function can compose with any other when types align
- [ ] **Type Safety**: Rust's type system validates all compositions at compile time
- [ ] **Performance**: Zero runtime overhead for composition validation
- [ ] **WGSL Integration**: Generated WGSL code integrates seamlessly with wgpu

### Composition System

- [ ] **Automatic Chaining**: Functions compose when Input/Output types match
- [ ] **Type Validation**: Invalid compositions caught at compile time
- [ ] **Pipeline Generation**: Composed functions generate optimized WGSL
- [ ] **Uniform Management**: Automatic uniform buffer allocation and binding

## Technical Tasks

### 1. Core Trait Definition

- [ ] Define ShaderFunction trait with associated types
- [ ] Create ShaderType trait for type system integration
- [ ] Implement composition validation through type system
- [ ] Add trait bounds for WGSL code generation

### 2. Type System Integration

- [ ] Define ShaderType trait for GPU-compatible types
- [ ] Implement ShaderType for primitive types (f32, vec2, vec3, vec4, etc.)
- [ ] Create compatibility checking for Input/Output type alignment
- [ ] Add automatic type conversion where safe

### 3. Function Composition

- [ ] Implement composition operators for chaining functions
- [ ] Create FunctionChain type for representing composed functions
- [ ] Add compile-time validation of composition chains
- [ ] Generate optimized WGSL for composed functions

### 4. Uniform Buffer Integration

- [ ] Automatic uniform buffer generation from function parameters
- [ ] Uniform binding management in shader pipelines
- [ ] Type-safe uniform buffer updates
- [ ] Efficient uniform buffer reuse across function instances

## Detailed Requirements

### ShaderType Trait

```rust
pub trait ShaderType: Clone + Send + Sync + 'static {
    // WGSL type name (e.g., "f32", "vec2<f32>", "mat4x4<f32>")
    fn wgsl_type_name() -> &'static str;
    
    // WGSL struct definition for complex types
    fn wgsl_type_definition() -> Option<&'static str> { None }
    
    // Size in bytes for uniform buffer layout
    fn size_bytes() -> usize;
    
    // Alignment requirements for GPU
    fn alignment() -> usize;
}

// Implementations for common types
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
```

### Function Composition

```rust
pub struct FunctionChain<A: ShaderFunction, B: ShaderFunction> 
where 
    A::Output: Compatible<B::Input>
{
    first: A,
    second: B,
    _phantom: PhantomData<(A::Output, B::Input)>,
}

impl<A: ShaderFunction, B: ShaderFunction> ShaderFunction for FunctionChain<A, B>
where 
    A::Output: Compatible<B::Input>
{
    type Input = A::Input;
    type Output = B::Output;
    type Uniforms = ChainUniforms<A::Uniforms, B::Uniforms>;

    fn wgsl_function() -> &'static str {
        // Generate WGSL that calls first function, then second function
        &format!("
            fn {}_chain(input: {}) -> {} {{
                let intermediate = {}(input, first_uniforms);
                return {}(intermediate, second_uniforms);
            }}
        ", 
            Self::function_name(),
            A::Input::wgsl_type_name(),
            B::Output::wgsl_type_name(),
            A::function_name(),
            B::function_name()
        )
    }
}
```

### Example Shader Functions

```rust
// Linear scale transformation
pub struct LinearScale {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
}

impl ShaderFunction for LinearScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = LinearScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
            let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
            return scale.range_min + normalized * (scale.range_max - scale.range_min);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(LinearScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
        })
    }

    fn function_name() -> &'static str { "linear_scale" }
}
```

## Dependencies

### Prerequisite Stories

- GUP-003: GPU Buffer Management (for uniform buffers)
- GUP-004: Basic Render Context (for shader compilation)

### Enables Stories

- GUP-006: WGSL Function Macro
- GUP-007: Shader Pipeline Builder
- GUP-008: Mark System Integration
- GUP-010: Example Shader Functions

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_shader_function_trait() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let uniforms = scale.create_uniforms().unwrap();
    
    assert_eq!(LinearScale::function_name(), "linear_scale");
    assert!(LinearScale::wgsl_function().contains("linear_scale"));
}

#[test]
fn test_function_composition() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(color_palette);
    
    // This should compile (f32 -> f32 -> vec4<f32>)
    let composed = scale.compose(color_map);
    assert_eq!(composed.input_type(), "f32");
    assert_eq!(composed.output_type(), "vec4<f32>");
}

#[test]
fn test_type_safety() {
    let position_func = PositionTransform::new();  // f32 -> vec2<f32>
    let color_func = ColorMap::new();              // f32 -> vec4<f32>
    
    // This should NOT compile (vec2<f32> ≠ f32)
    // let invalid = position_func.compose(color_func);
}
```

### Integration Tests

- [ ] Test shader function integration with actual GPU compilation
- [ ] Verify uniform buffer generation and binding
- [ ] Test complex composition chains with multiple functions
- [ ] Validate generated WGSL compiles and runs correctly

### Property-Based Tests

```rust
#[quickcheck]
fn test_composition_associativity(
    a: LinearScale, 
    b: ColorMap, 
    c: PositionTransform
) -> bool {
    // Test that (a.compose(b)).compose(c) == a.compose(b.compose(c))
    let left_assoc = (a.compose(b.clone())).compose(c.clone());
    let right_assoc = a.compose(b.compose(c));
    
    generated_wgsl_equivalent(&left_assoc, &right_assoc)
}
```

## Success Metrics

### Functional Requirements

- [ ] **Type Safety**: 100% of invalid compositions caught at compile time
- [ ] **Performance**: Zero runtime overhead for type validation
- [ ] **WGSL Quality**: Generated WGSL compiles without errors on all target platforms
- [ ] **Composability**: Complex 5+ function chains work correctly

### Quality Requirements

- [ ] **Test Coverage**: >90% test coverage for trait implementation
- [ ] **Documentation**: Complete rustdoc with composition examples
- [ ] **Error Messages**: Clear compile-time errors for invalid compositions
- [ ] **Cross-Platform**: Identical behavior across all wgpu backends

## Risk Assessment

### Technical Risks

- **High**: Complex type system integration could make trait unusable
- **High**: Generated WGSL quality might not meet performance requirements
- **Medium**: Uniform buffer management complexity could introduce bugs

### Mitigation Strategies

- **Prototype First**: Build minimal working version to validate approach
- **Incremental Complexity**: Start with simple compositions, add complexity gradually
- **Performance Testing**: Benchmark every design decision against hand-written shaders

## Implementation Notes

### Design Decisions

- Use associated types rather than generic parameters for cleaner composition APIs
- Generate WGSL strings at compile time for better performance
- Implement uniform buffer packing automatically based on WGSL std430 layout
- Use trait bounds to enforce type compatibility rather than runtime checks

### Type System Strategy

- Leverage Rust's type system for composition validation
- Use phantom types to carry type information without runtime cost
- Implement automatic type coercion for compatible types (e.g., f32 -> vec2<f32>)
- Create clear error messages for incompatible type combinations

### WGSL Generation Strategy

- Template-based WGSL generation for consistent output
- Automatic function naming to avoid conflicts in composed shaders
- Optimized uniform buffer layout following WGSL alignment rules
- Dead code elimination for unused uniform fields

## Definition of Done

- [ ] ShaderFunction trait compiles and passes all tests
- [ ] Function composition works with complex chains
- [ ] Generated WGSL compiles and runs correctly on GPU
- [ ] Type safety validated with comprehensive test cases
- [ ] Performance benchmarks show zero composition overhead
- [ ] Uniform buffer management working correctly
- [ ] Documentation includes comprehensive composition examples
- [ ] Code review completed and approved
