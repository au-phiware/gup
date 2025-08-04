# GUP-073: Advanced Shader Composition

**Story ID**: GUP-073  
**Title**: Advanced Shader Composition  
**Status**: Planned  
**Priority**: Medium  
**Effort**: 8 story points  
**Created**: 2025-08-04  
**Dependencies**: GUP-011 (Mark-Shader Integration)

## Summary

Enhance the shader function composition system with AST-based WGSL generation
for better type safety, validation, and optimization capabilities.

## Background

GUP-011 implemented basic shader function integration using string-based WGSL
composition. While effective, this approach has limitations:

- Limited compile-time validation of WGSL syntax
- No type checking between composed functions
- Manual string manipulation for shader generation
- Difficult to implement advanced optimizations

Moving to AST-based composition would provide:

- Full WGSL syntax validation
- Type-safe function composition
- Advanced optimization passes
- Better error reporting

## Requirements

### Functional Requirements

1. **AST-Based WGSL Generation**

   - Replace string-based shader composition with proper AST manipulation
   - Parse existing WGSL templates into AST representation
   - Generate WGSL from AST with proper formatting

2. **Type-Safe Function Composition**

   - Validate input/output types between composed functions at composition time
   - Provide clear error messages for type mismatches
   - Support automatic type conversion where appropriate

3. **Advanced Optimizations**

   - Dead code elimination for unused shader functions
   - Constant folding across function boundaries
   - Loop unrolling for small constant iterations
   - Common sub-expression elimination

4. **Enhanced Error Reporting**
   - Pinpoint exact locations of composition errors
   - Suggest fixes for common composition mistakes
   - Validate WGSL syntax before compilation

### Non-Functional Requirements

1. **Performance**: Composition time should remain \<10ms for typical use cases
2. **Compatibility**: Maintain backward compatibility with GUP-011 string-based
   API
3. **Memory**: AST representation should not exceed 2x memory usage of string
   approach

## Acceptance Criteria

1. **AST Implementation**

   - [ ] WGSL AST types for all relevant constructs (functions, types,
         expressions)
   - [ ] Parser to convert WGSL text to AST
   - [ ] Generator to convert AST back to WGSL text
   - [ ] Round-trip tests: WGSL → AST → WGSL preserves semantics

2. **Type System**

   - [ ] Type checking for function input/output compatibility
   - [ ] Automatic type promotion (e.g., f32 → vec3\<f32\> with zero padding)
   - [ ] Clear error messages for type mismatches

3. **Optimization Passes**

   - [ ] Dead code elimination removes unused functions
   - [ ] Constant folding simplifies expressions
   - [ ] Function inlining for small functions (\<10 instructions)

4. **Error Handling**

   - [ ] Syntax errors report line/column information
   - [ ] Type errors include expected vs actual types
   - [ ] Composition errors suggest valid alternatives

5. **Performance**
   - [ ] Benchmarks show \<10ms composition time for 10-function chains
   - [ ] Memory usage \<2x compared to string approach
   - [ ] Generated WGSL is optimal (no unused variables/functions)

## Technical Design

### AST Structure

```rust
#[derive(Debug, Clone)]
pub enum WgslType {
    Scalar(ScalarType),
    Vector(ScalarType, u8), // type, dimension
    Matrix(ScalarType, u8, u8), // type, cols, rows
    Array(Box<WgslType>, Option<u32>), // element type, size
    Struct(String), // struct name
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<WgslType>,
    pub body: Block,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub struct ComposableShaderPipelineAST {
    functions: Vec<Function>,
    type_registry: TypeRegistry,
    optimization_passes: Vec<Box<dyn OptimizationPass>>,
}
```

### Type Checking System

```rust
pub trait TypeChecker {
    fn check_compatibility(&self, output: &WgslType, input: &WgslType) -> Result<(), TypeError>;
    fn suggest_conversion(&self, from: &WgslType, to: &WgslType) -> Option<String>;
    fn validate_function_chain(&self, functions: &[Function]) -> Result<(), CompositionError>;
}
```

### Optimization Framework

```rust
pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&self, ast: &mut WgslAST) -> Result<bool, OptimizationError>; // returns true if changed
}

pub struct DeadCodeElimination;
pub struct ConstantFolding;
pub struct FunctionInlining { max_instructions: usize };
```

## Implementation Plan

### Phase 1: AST Foundation (3 points)

- Define AST types for WGSL constructs
- Implement basic parser (WGSL → AST)
- Implement generator (AST → WGSL)
- Add round-trip tests

### Phase 2: Type System (3 points)

- Implement type checking framework
- Add type compatibility validation
- Create helpful error messages
- Add type conversion suggestions

### Phase 3: Optimization (2 points)

- Implement dead code elimination
- Add constant folding pass
- Create function inlining optimization
- Add performance benchmarks

## Risks and Mitigations

1. **Risk**: AST parsing complexity for full WGSL syntax

   - **Mitigation**: Start with subset needed for shader functions, expand
     incrementally

2. **Risk**: Performance regression compared to string approach

   - **Mitigation**: Implement caching and lazy evaluation, benchmark
     continuously

3. **Risk**: Breaking changes to existing API
   - **Mitigation**: Maintain string-based API alongside AST approach, deprecate
     gradually

## Success Metrics

- Type errors caught at composition time (not WGSL compilation time)
- Generated WGSL is 10-20% smaller due to optimizations
- Composition errors provide actionable error messages
- Performance benchmarks show \<10ms for typical use cases

## Future Considerations

- Integration with WGSL language server for IDE support
- Advanced optimizations (loop vectorization, memory layout optimization)
- Support for compute shader composition
- Integration with shader debugging tools
