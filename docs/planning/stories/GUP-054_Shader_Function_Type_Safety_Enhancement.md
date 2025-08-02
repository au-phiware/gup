# GUP-054: Shader Function Type Safety Enhancement

**Status**: Planned  
**Epic**: Shader Function System  
**Priority**: High  
**Complexity**: Medium

## Overview

Enhance the shader function system with improved type safety and automatic type
inference to prevent runtime WGSL compilation errors discovered during GUP-007
implementation.

## Motivation

During GUP-007 implementation, several categories of errors were discovered that
could be prevented with better type safety:

1. **Type Mismatch Errors**: Functions expecting vec2 called with f32 parameters
2. **Output Type Confusion**: Functions returning vec4 treated as f32 outputs
3. **Uniform Structure Mismatches**: WGSL struct fields not matching Rust
   uniform types
4. **Manual Type Mapping**: Hardcoded type knowledge instead of automatic
   inference

These issues required manual fixes and caused multiple GPU compilation failures
that could be caught at compile time with better type system integration.

## Goals

1. **Compile-Time Type Validation**: Catch type mismatches before GPU
   compilation
2. **Automatic Type Inference**: Eliminate manual type mapping in pipeline
   generation
3. **Type-Safe Function Composition**: Ensure output types match input
   requirements
4. **WGSL Generation Safety**: Automatic uniform struct generation from Rust
   types
5. **Clear Error Messages**: Helpful compile-time errors for type violations

## Non-Goals

- Runtime type checking (focus on compile-time safety)
- Breaking changes to existing shader function APIs
- Complex type transformation beyond basic conversions
- Support for arbitrary user-defined types without GPU compatibility

## Technical Approach

### Enhanced Trait System with Type Information

```rust
pub trait ComposableShaderFunction {
    type Input: ShaderType;
    type Output: ShaderType;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable + ShaderUniform;

    // Compile-time accessible type information
    fn input_type() -> ShaderTypeInfo;
    fn output_type() -> ShaderTypeInfo;
    fn uniform_layout() -> UniformLayout;
}

pub trait ShaderType {
    const WGSL_TYPE: &'static str;
    const COMPONENT_COUNT: usize;

    fn to_wgsl_value_expr(name: &str) -> String;
    fn from_wgsl_assignment(var_name: &str, expr: &str) -> String;
}

impl ShaderType for f32 {
    const WGSL_TYPE: &'static str = "f32";
    const COMPONENT_COUNT: usize = 1;

    fn to_wgsl_value_expr(name: &str) -> String {
        name.to_string()
    }

    fn from_wgsl_assignment(var_name: &str, expr: &str) -> String {
        format!("let {var_name} = {expr};")
    }
}

impl ShaderType for Vec2 {
    const WGSL_TYPE: &'static str = "vec2<f32>";
    const COMPONENT_COUNT: usize = 2;

    fn to_wgsl_value_expr(name: &str) -> String {
        format!("vec2<f32>({name}.x, {name}.y)")
    }

    fn from_wgsl_assignment(var_name: &str, expr: &str) -> String {
        match Self::COMPONENT_COUNT {
            1 => format!("let {var_name} = vec2<f32>({expr}, 0.0);"),
            2 => format!("let {var_name} = {expr};"),
            4 => format!("let {var_name} = {expr}.xy;"),
            _ => panic!("Invalid component count conversion"),
        }
    }
}
```

### Type-Safe Function Composition

```rust
pub trait TypeCompatible<T: ShaderType> {
    fn is_compatible() -> bool;
    fn conversion_wgsl() -> Option<String>;
}

// Automatic compatibility for same types
impl<T: ShaderType> TypeCompatible<T> for T {
    fn is_compatible() -> bool { true }
    fn conversion_wgsl() -> Option<String> { None }
}

// Explicit conversions for compatible types
impl TypeCompatible<Vec2> for f32 {
    fn is_compatible() -> bool { true }
    fn conversion_wgsl() -> Option<String> {
        Some("vec2<f32>({}, 0.0)".to_string())
    }
}

pub struct FunctionChain<A, B>
where
    A: ComposableShaderFunction,
    B: ComposableShaderFunction,
    A::Output: TypeCompatible<B::Input>, // Compile-time validation
{
    first: A,
    second: B,
}
```

### Automatic Uniform Struct Generation

```rust
pub trait ShaderUniform: bytemuck::Pod + bytemuck::Zeroable {
    fn wgsl_struct_definition() -> String;
    fn wgsl_type_name() -> &'static str;
}

// Derive macro for automatic implementation
#[derive(ShaderUniform)]
#[repr(C)]
struct LinearScaleUniforms {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
}

// Generates:
// impl ShaderUniform for LinearScaleUniforms {
//     fn wgsl_struct_definition() -> String {
//         "struct LinearScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n}"
//     }
//     fn wgsl_type_name() -> &'static str { "LinearScaleUniforms" }
// }
```

### Enhanced Pipeline Generation

```rust
impl ComposableShaderPipeline {
    fn generate_type_safe_uniforms(&self) -> String {
        let mut definitions = String::new();
        let mut defined_types = HashSet::new();

        for function in &self.functions {
            let type_name = function.uniform_type_name();
            if !defined_types.contains(type_name) {
                definitions.push_str(&function.uniform_struct_definition());
                defined_types.insert(type_name);
            }
        }
        definitions
    }

    fn generate_type_safe_function_call(&self, function: &PipelineFunction, input_type: ShaderTypeInfo) -> String {
        let expected_input = function.input_type();
        if input_type.is_compatible_with(&expected_input) {
            if let Some(conversion) = input_type.conversion_to(&expected_input) {
                format!("{}({}(input), uniforms)", function.name(), conversion)
            } else {
                format!("{}(input, uniforms)", function.name())
            }
        } else {
            panic!("Type mismatch: expected {:?}, got {:?}", expected_input, input_type);
        }
    }
}
```

### Compile-Time Type Validation

```rust
// Type-safe pipeline building
impl ComposableShaderPipeline {
    pub fn add_typed_function<F>(&mut self, function: F) -> TypedPipelineBuilder<F::Output>
    where
        F: ComposableShaderFunction,
    {
        self.functions.push(PipelineFunction::new(function));
        TypedPipelineBuilder::new(self)
    }
}

pub struct TypedPipelineBuilder<'a, T: ShaderType> {
    pipeline: &'a mut ComposableShaderPipeline,
    _phantom: PhantomData<T>,
}

impl<'a, T: ShaderType> TypedPipelineBuilder<'a, T> {
    pub fn then<F>(self, function: F) -> TypedPipelineBuilder<'a, F::Output>
    where
        F: ComposableShaderFunction,
        T: TypeCompatible<F::Input>, // Compile-time validation
    {
        self.pipeline.functions.push(PipelineFunction::new(function));
        TypedPipelineBuilder::new(self.pipeline)
    }
}
```

## Implementation Plan

### Phase 1: Core Type System

- [ ] Define ShaderType trait and implementations for basic types
- [ ] Implement TypeCompatible trait with conversion logic
- [ ] Create ShaderUniform trait for automatic struct generation
- [ ] Basic compile-time validation for function composition

### Phase 2: Enhanced Pipeline Generation

- [ ] Automatic uniform struct definition generation
- [ ] Type-aware function call generation
- [ ] Elimination of hardcoded type mappings
- [ ] Comprehensive type conversion support

### Phase 3: Type-Safe Builder API

- [ ] TypedPipelineBuilder with compile-time validation
- [ ] Fluent API for type-safe function composition
- [ ] Clear compile-time error messages
- [ ] Migration path from existing APIs

### Phase 4: Advanced Type Features

- [ ] Custom type registration system
- [ ] Advanced type conversions (matrix types, etc.)
- [ ] Performance optimization for type-safe operations
- [ ] Integration with existing shader function macros

## Expected Benefits

### Compile-Time Error Prevention

```rust
// ❌ This would now fail at compile time instead of runtime
pipeline.add_function(position_transform) // expects Vec2
    .with_input_from(linear_scale);        // outputs f32 - COMPILE ERROR

// ✅ This provides automatic conversion
pipeline.add_function(position_transform)
    .with_converted_input_from(linear_scale); // f32 -> Vec2 conversion
```

### Automatic Type Inference

```rust
// Before: Manual type mapping
let uniform_type = match function.name() {
    "linear_scale" => "LinearScaleUniforms",
    "color_map" => "ColorMapUniforms",
    // ...
};

// After: Automatic inference
let uniform_type = F::Uniforms::wgsl_type_name();
let struct_def = F::Uniforms::wgsl_struct_definition();
```

## Testing Strategy

- **Compile-time tests**: Verify type errors are caught at compile time
- **Integration tests**: Type-safe pipelines generate correct WGSL
- **Performance tests**: Ensure type safety doesn't impact generation
  performance
- **Migration tests**: Existing APIs continue to work during transition
- **Error message tests**: Verify helpful error messages for type violations

## Acceptance Criteria

- [ ] All type mismatches caught at compile time instead of GPU compilation
- [ ] Automatic uniform struct generation eliminates manual type mapping
- [ ] Type-safe function composition with clear conversion paths
- [ ] Backward compatibility with existing shader function APIs
- [ ] Performance impact <5% on shader generation times
- [ ] Clear, actionable compile-time error messages for type violations
- [ ] Comprehensive type conversion support for common GPU types

## Dependencies

- Completed GUP-007 (Shader Pipeline Builder)
- Enhanced gup-macros for derive macro support
- Proc-macro development for ShaderUniform derive

## References

- GUP-007 type safety issues and manual fixes
- Rust type system best practices
- GPU type system requirements
- WGSL type compatibility rules

---

**Story Created**: 2025-08-02  
**Last Updated**: 2025-08-02
