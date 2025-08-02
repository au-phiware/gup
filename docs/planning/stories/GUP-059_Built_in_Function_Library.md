# GUP-059: Built-in Function Library and Mathematical Operations

## Story Overview

**Title**: Implement Comprehensive Built-in Function Library for Rust-to-WGSL
Transpilation  
**Epic**: Phase 2 Initiative 5 - Rust-to-WGSL Transpilation  
**Priority**: Medium  
**Story Points**: 8

## Context

With expression transpilation and control flow established, we need to create a
comprehensive library of built-in mathematical functions, vector operations, and
GPU-specific utilities that can be called naturally from Rust and transpiled to
efficient WGSL.

## User Story

**As a** shader function developer  
**I want** access to a rich library of mathematical and GPU functions  
**So that** I can write complex shader logic without manually implementing
common operations

## Problem Statement

WGSL provides many built-in functions that don't have direct Rust equivalents,
and vice versa. We need a system that:

- Maps Rust standard library functions to WGSL built-ins where possible
- Provides GPU-specific functions in Rust-callable form
- Handles function overloading and type-specific variants
- Offers performance-optimized implementations for common operations

## Acceptance Criteria

### AC1: Mathematical Function Mapping

- [ ] Map Rust f32/f64 methods to WGSL math functions
- [ ] Support trigonometric functions (sin, cos, tan, etc.)
- [ ] Implement exponential and logarithmic functions
- [ ] Handle power and root functions
- [ ] Support interpolation and clamping functions

### AC2: Vector and Matrix Operations

- [ ] Implement vector arithmetic and utility functions
- [ ] Support matrix multiplication and transformation operations
- [ ] Handle dot product, cross product, and normalization
- [ ] Provide geometric functions (distance, reflect, refract)
- [ ] Support swizzling operations for vectors

### AC3: GPU-Specific Functions

- [ ] Implement texture sampling functions
- [ ] Support derivative functions (dpdx, dpdy, fwidth)
- [ ] Handle atomic operations for compute shaders
- [ ] Provide barrier and synchronization functions
- [ ] Support pack/unpack operations for data compression

### AC4: Type-Safe Function Overloading

- [ ] Handle function overloading based on parameter types
- [ ] Support generic functions with type constraints
- [ ] Provide compile-time function resolution
- [ ] Generate appropriate WGSL function calls
- [ ] Validate function availability for target WGSL version

## Technical Requirements

### Function Library Architecture

```rust
// Core trait for built-in function mapping
pub trait WgslBuiltinFunction {
    fn wgsl_function_name() -> &'static str;
    fn parameter_types() -> &'static [WgslType];
    fn return_type() -> WgslType;
    fn availability() -> WgslVersion;
    fn generate_call(&self, args: &[String]) -> String;
}

// Function registry for runtime lookup
pub struct BuiltinFunctionRegistry {
    functions: HashMap<FunctionSignature, Box<dyn WgslBuiltinFunction>>,
    overloads: HashMap<String, Vec<FunctionSignature>>,
}

impl BuiltinFunctionRegistry {
    pub fn resolve_function_call(&self, name: &str, arg_types: &[WgslType]) -> Result<&dyn WgslBuiltinFunction, FunctionResolutionError>;
    pub fn register_function<F: WgslBuiltinFunction + 'static>(&mut self, function: F);
    pub fn list_overloads(&self, name: &str) -> Vec<&FunctionSignature>;
}
```

### Mathematical Functions Library

```rust
// Trigonometric functions
pub mod trig {
    pub fn sin(x: f32) -> f32 { /* transpiles to sin(x) */ }
    pub fn cos(x: f32) -> f32 { /* transpiles to cos(x) */ }
    pub fn tan(x: f32) -> f32 { /* transpiles to tan(x) */ }
    pub fn asin(x: f32) -> f32 { /* transpiles to asin(x) */ }
    pub fn acos(x: f32) -> f32 { /* transpiles to acos(x) */ }
    pub fn atan(x: f32) -> f32 { /* transpiles to atan(x) */ }
    pub fn atan2(y: f32, x: f32) -> f32 { /* transpiles to atan2(y, x) */ }
}

// Exponential and logarithmic functions
pub mod exp {
    pub fn exp(x: f32) -> f32 { /* transpiles to exp(x) */ }
    pub fn exp2(x: f32) -> f32 { /* transpiles to exp2(x) */ }
    pub fn log(x: f32) -> f32 { /* transpiles to log(x) */ }
    pub fn log2(x: f32) -> f32 { /* transpiles to log2(x) */ }
    pub fn pow(x: f32, y: f32) -> f32 { /* transpiles to pow(x, y) */ }
    pub fn sqrt(x: f32) -> f32 { /* transpiles to sqrt(x) */ }
}

// Utility functions
pub mod util {
    pub fn abs(x: f32) -> f32 { /* transpiles to abs(x) */ }
    pub fn sign(x: f32) -> f32 { /* transpiles to sign(x) */ }
    pub fn floor(x: f32) -> f32 { /* transpiles to floor(x) */ }
    pub fn ceil(x: f32) -> f32 { /* transpiles to ceil(x) */ }
    pub fn round(x: f32) -> f32 { /* transpiles to round(x) */ }
    pub fn fract(x: f32) -> f32 { /* transpiles to fract(x) */ }
    pub fn min(a: f32, b: f32) -> f32 { /* transpiles to min(a, b) */ }
    pub fn max(a: f32, b: f32) -> f32 { /* transpiles to max(a, b) */ }
    pub fn clamp(x: f32, min_val: f32, max_val: f32) -> f32 { /* transpiles to clamp(x, min_val, max_val) */ }
    pub fn mix(a: f32, b: f32, t: f32) -> f32 { /* transpiles to mix(a, b, t) */ }
    pub fn step(edge: f32, x: f32) -> f32 { /* transpiles to step(edge, x) */ }
    pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 { /* transpiles to smoothstep(edge0, edge1, x) */ }
}
```

### Vector Operations Library

```rust
// Vector-specific functions
impl Vec2 {
    pub fn length(&self) -> f32 { /* transpiles to length(self) */ }
    pub fn length_squared(&self) -> f32 { /* transpiles to dot(self, self) */ }
    pub fn normalize(&self) -> Vec2 { /* transpiles to normalize(self) */ }
    pub fn dot(&self, other: Vec2) -> f32 { /* transpiles to dot(self, other) */ }
    pub fn distance(&self, other: Vec2) -> f32 { /* transpiles to distance(self, other) */ }
    pub fn reflect(&self, normal: Vec2) -> Vec2 { /* transpiles to reflect(self, normal) */ }
}

impl Vec3 {
    pub fn length(&self) -> f32 { /* transpiles to length(self) */ }
    pub fn normalize(&self) -> Vec3 { /* transpiles to normalize(self) */ }
    pub fn dot(&self, other: Vec3) -> f32 { /* transpiles to dot(self, other) */ }
    pub fn cross(&self, other: Vec3) -> Vec3 { /* transpiles to cross(self, other) */ }
    pub fn reflect(&self, normal: Vec3) -> Vec3 { /* transpiles to reflect(self, normal) */ }
    pub fn refract(&self, normal: Vec3, eta: f32) -> Vec3 { /* transpiles to refract(self, normal, eta) */ }
}

impl Vec4 {
    pub fn length(&self) -> f32 { /* transpiles to length(self) */ }
    pub fn normalize(&self) -> Vec4 { /* transpiles to normalize(self) */ }
    pub fn dot(&self, other: Vec4) -> f32 { /* transpiles to dot(self, other) */ }
}
```

### GPU-Specific Functions

```rust
// Derivative functions for fragment shaders
pub mod derivatives {
    pub fn dpdx(p: f32) -> f32 { /* transpiles to dpdx(p) */ }
    pub fn dpdy(p: f32) -> f32 { /* transpiles to dpdy(p) */ }
    pub fn fwidth(p: f32) -> f32 { /* transpiles to fwidth(p) */ }

    // Vector variants
    pub fn dpdx_vec2(p: Vec2) -> Vec2 { /* transpiles to dpdx(p) */ }
    pub fn dpdy_vec2(p: Vec2) -> Vec2 { /* transpiles to dpdy(p) */ }
    pub fn fwidth_vec2(p: Vec2) -> Vec2 { /* transpiles to fwidth(p) */ }
}

// Texture sampling functions
pub mod texture {
    pub fn sample(texture: Texture2D, sampler: Sampler, coord: Vec2) -> Vec4 { /* transpiles to textureSample(texture, sampler, coord) */ }
    pub fn sample_level(texture: Texture2D, sampler: Sampler, coord: Vec2, level: f32) -> Vec4 { /* transpiles to textureSampleLevel(texture, sampler, coord, level) */ }
    pub fn sample_bias(texture: Texture2D, sampler: Sampler, coord: Vec2, bias: f32) -> Vec4 { /* transpiles to textureSampleBias(texture, sampler, coord, bias) */ }
    pub fn sample_grad(texture: Texture2D, sampler: Sampler, coord: Vec2, ddx: Vec2, ddy: Vec2) -> Vec4 { /* transpiles to textureSampleGrad(texture, sampler, coord, ddx, ddy) */ }
}
```

### Function Mapping Table

| Rust Function               | WGSL Function        | Parameter Types        | Return Type | Notes              |
| --------------------------- | -------------------- | ---------------------- | ----------- | ------------------ |
| `f32::sin(x)`               | `sin(x)`             | `f32`                  | `f32`       | Direct mapping     |
| `f32::abs(x)`               | `abs(x)`             | `f32`                  | `f32`       | Direct mapping     |
| `Vec3::length(&self)`       | `length(self)`       | `vec3<f32>`            | `f32`       | Method to function |
| `Vec3::normalize(&self)`    | `normalize(self)`    | `vec3<f32>`            | `vec3<f32>` | Method to function |
| `Vec3::dot(&self, other)`   | `dot(self, other)`   | `vec3<f32>, vec3<f32>` | `f32`       | Method to function |
| `Vec3::cross(&self, other)` | `cross(self, other)` | `vec3<f32>, vec3<f32>` | `vec3<f32>` | Method to function |

## Dependencies

- GUP-055: AST parsing for function call recognition
- GUP-056: Type system for function signature matching
- GUP-057: Expression transpilation for function arguments
- GUP-058: Control flow for function call context

## Definition of Done

- [ ] Comprehensive built-in function library covering math, vector, and GPU
      operations
- [ ] Type-safe function overload resolution system
- [ ] Integration with expression transpiler for seamless function calls
- [ ] Performance benchmarks showing efficient function call generation
- [ ] Documentation with examples for all available functions
- [ ] Test suite covering all function categories and overloads

## Test Requirements

### Unit Tests

```rust
#[test]
fn test_math_function_mapping() {
    let registry = BuiltinFunctionRegistry::new();
    let func = registry.resolve_function_call("sin", &[WgslType::F32]).unwrap();
    assert_eq!(func.wgsl_function_name(), "sin");
    assert_eq!(func.generate_call(&["x".to_string()]), "sin(x)");
}

#[test]
fn test_vector_method_mapping() {
    let registry = BuiltinFunctionRegistry::new();
    let func = registry.resolve_function_call("length", &[WgslType::Vec3F32]).unwrap();
    assert_eq!(func.wgsl_function_name(), "length");
    assert_eq!(func.generate_call(&["v".to_string()]), "length(v)");
}

#[test]
fn test_function_overload_resolution() {
    let registry = BuiltinFunctionRegistry::new();

    // Test f32 variant
    let func_f32 = registry.resolve_function_call("abs", &[WgslType::F32]).unwrap();
    assert_eq!(func_f32.return_type(), WgslType::F32);

    // Test vec3 variant
    let func_vec3 = registry.resolve_function_call("abs", &[WgslType::Vec3F32]).unwrap();
    assert_eq!(func_vec3.return_type(), WgslType::Vec3F32);
}
```

### Integration Tests

```rust
#[test]
fn test_complex_mathematical_shader() {
    let shader_fn = wgsl_function! {
        fn lighting_model(
            normal: Vec3,
            light_dir: Vec3,
            view_dir: Vec3,
            roughness: f32
        ) -> Vec3 {
            let n_dot_l = normal.dot(light_dir).max(0.0);
            let reflect_dir = light_dir.reflect(normal);
            let spec_angle = view_dir.dot(reflect_dir).max(0.0);
            let specular = spec_angle.pow(32.0 * (1.0 - roughness));

            Vec3::new(n_dot_l, n_dot_l, n_dot_l) * 0.8 + Vec3::new(specular, specular, specular) * 0.2
        }
    };

    let wgsl = shader_fn.generated_wgsl();
    assert!(wgsl.contains("max(dot(normal, light_dir), 0.0)"));
    assert!(wgsl.contains("reflect(light_dir, normal)"));
    assert!(wgsl.contains("pow(max(dot(view_dir, reflect_dir), 0.0), 32.0 * (1.0 - roughness))"));
}

#[test]
fn test_texture_sampling_operations() {
    let shader_fn = wgsl_function! {
        fn sample_texture_with_filtering(
            tex: Texture2D,
            samp: Sampler,
            uv: Vec2,
            lod_bias: f32
        ) -> Vec4 {
            let base_sample = texture::sample(tex, samp, uv);
            let biased_sample = texture::sample_bias(tex, samp, uv, lod_bias);

            // Mix based on distance from center
            let center_dist = (uv - Vec2::new(0.5, 0.5)).length();
            mix(base_sample, biased_sample, center_dist)
        }
    };

    let wgsl = shader_fn.generated_wgsl();
    assert!(wgsl.contains("textureSample(tex, samp, uv)"));
    assert!(wgsl.contains("textureSampleBias(tex, samp, uv, lod_bias)"));
    assert!(wgsl.contains("mix(base_sample, biased_sample, center_dist)"));
}
```

### Error Handling Tests

```rust
#[test]
fn test_function_resolution_errors() {
    let registry = BuiltinFunctionRegistry::new();

    // Test unknown function
    let result = registry.resolve_function_call("unknown_func", &[WgslType::F32]);
    assert!(matches!(result, Err(FunctionResolutionError::FunctionNotFound { .. })));

    // Test invalid argument types
    let result = registry.resolve_function_call("sin", &[WgslType::Bool]);
    assert!(matches!(result, Err(FunctionResolutionError::NoMatchingOverload { .. })));
}
```

## Performance Considerations

- **Function Call Overhead**: Ensure zero runtime overhead for function call
  transpilation
- **Type Resolution**: Optimize function overload resolution for compile-time
  performance
- **Generated Code Quality**: Verify transpiled function calls produce optimal
  WGSL
- **Registry Size**: Balance comprehensive function coverage with compilation
  memory usage

## Future Considerations

This implementation enables:

- GUP-060: Advanced optimization passes that can inline and optimize function
  calls
- Custom user-defined function libraries with similar transpilation support
- Platform-specific function variants for different GPU architectures
- Integration with GPU profiling tools for function performance analysis
- Support for newer WGSL built-in functions as they become available
