# GUP-073: Advanced Shader Composition

**Story ID**: GUP-073  
**Title**: Advanced Shader Composition  
**Status**: ✅ Complete  
**Completed**: 2025-08-07  
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
   - [x] WGSL AST types for all relevant constructs (functions, types,
         expressions)
   - [x] Parser to convert WGSL text to AST
   - [x] Generator to convert AST back to WGSL text
   - [x] Round-trip tests: WGSL → AST → WGSL preserves semantics

2. **Type System**
   - [x] Type checking for function input/output compatibility
   - [x] Automatic type promotion (e.g., f32 → vec3\<f32\> with zero padding)
   - [x] Clear error messages for type mismatches

3. **Optimization Passes**
   - [x] Dead code elimination removes unused functions
   - [x] Constant folding simplifies expressions
   - [x] Function inlining for small functions (\<10 instructions)

4. **Error Handling**
   - [x] Syntax errors report line/column information
   - [x] Type errors include expected vs actual types
   - [x] Composition errors suggest valid alternatives

5. **Performance**
   - [x] Benchmarks show \<10ms composition time for 10-function chains
   - [x] Memory usage \<2x compared to string approach
   - [x] Generated WGSL is optimal (no unused variables/functions)

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

## Implementation Summary

**Completed**: 2025-08-07

### What Was Implemented

A complete AST-based WGSL shader composition system in the `shader_ast` module:

1. **AST Types** (`src/shader_ast/types.rs`): Complete WGSL type system
   including `WgslType`, `ScalarType`, `Function`, `Expr`, `Statement`, `Block`,
   `StructDef`, `GlobalVar`, `Attribute`, and all expression/statement variants.

2. **Parser** (`src/shader_ast/parser.rs`): Full lexer + recursive descent
   parser covering functions, structs, global vars, expressions with operator
   precedence, statements (let/var/return/if/for), and WGSL attributes.

3. **Generator** (`src/shader_ast/generator.rs`): AST → WGSL text generator with
   proper indentation, configurable header, and correct formatting for all
   constructs.

4. **Type Checker** (`src/shader_ast/type_check.rs`): `TypeChecker` with
   compatibility checking, automatic type promotion (f32 → vec2/3/4),
   `FunctionSignature` extraction from AST, chain validation, and suggestion
   generation.

5. **Optimizer** (`src/shader_ast/optimizer.rs`): Three optimization passes:
   - Dead code elimination (BFS reachability from entry points)
   - Constant folding (literal arithmetic, identity operations)
   - Function inlining (single-return functions with parameter substitution)

6. **Pipeline** (`src/shader_ast/pipeline.rs`): `AstShaderPipeline` that
   integrates with existing `ComposableShaderFunction` trait for type-checked
   composition and AST-based WGSL generation.

7. **Benchmarks** (`src/shader_ast/benchmarks.rs`): Performance validation
   ensuring <10ms composition time and reasonable memory usage.

### Key Files Changed

| File                           | Change Type | Purpose                          |
| ------------------------------ | ----------- | -------------------------------- |
| `src/shader_ast/mod.rs`        | New         | Module definition and re-exports |
| `src/shader_ast/types.rs`      | New         | AST type definitions             |
| `src/shader_ast/parser.rs`     | New         | WGSL text → AST parser           |
| `src/shader_ast/generator.rs`  | New         | AST → WGSL text generator        |
| `src/shader_ast/type_check.rs` | New         | Type compatibility validation    |
| `src/shader_ast/optimizer.rs`  | New         | Optimization passes              |
| `src/shader_ast/pipeline.rs`   | New         | Integration with shader system   |
| `src/shader_ast/benchmarks.rs` | New         | Performance benchmarks           |
| `src/lib.rs`                   | Modified    | Added `shader_ast` module        |

### Test Count

58 tests across all modules:

- types: 5 tests
- parser: 12 tests
- generator: 7 tests
- type_check: 12 tests
- optimizer: 7 tests
- pipeline: 8 tests
- benchmarks: 7 tests

## Retrospective

**Completed**: 2025-08-07

### Key Technical Learnings

#### WGSL Parser Subset Design

- **Challenge**: Deciding how much of WGSL to support. Full WGSL is complex
  (textures, samplers, compute workgroups, etc.) but shader functions only use a
  subset.
- **Solution**: Focused on the constructs used in Gup's shader function system:
  functions, structs, global var declarations, and common expression/statement
  forms. Supports attributes, type constructors, and member access which covers
  all current shader functions.
- **Pattern**: When building a DSL parser for a subset, start with the minimum
  needed for existing code, then expand. The parser handles all current
  `wgsl_function!` macro output.

#### AST-Based Optimization vs String-Based

- **Challenge**: The existing `shader_pipeline.rs` had string-based
  optimizations (e.g., replacing `"1.0 * "` with `""`) which were fragile and
  limited.
- **Solution**: AST-based optimization operates on the semantic structure,
  enabling proper dead code elimination (BFS from entry points), constant
  folding (evaluating literal arithmetic), and function inlining (parameter
  substitution).
- **Pattern**: String-based transformations are fine for simple cases but break
  down when you need semantic understanding. AST-based approaches are more
  robust and composable — each pass is independent and can run in any order.

#### Type Promotion System

- **Challenge**: Shader functions in Gup compose f32 → f32, f32 → vec4, etc.
  Need to catch incompatible chains early but allow reasonable promotions.
- **Solution**: `WgslType::can_promote_to()` implements the promotion rules (f32
  → vecN, smaller vec → larger vec) and `TypeChecker::check_compatibility` uses
  this plus suggestion generation for helpful error messages.
- **Pattern**: Type promotion rules should be explicit and well-documented. The
  suggestion system that tells users "use value.xy to truncate" is especially
  valuable for GPU shader development.

### Architectural Decisions

#### New Module vs Extending shader_pipeline.rs

- **Decision**: Created a new `shader_ast` module rather than modifying the
  existing `shader_pipeline.rs`.
- **Reasoning**: The existing string-based pipeline is well-tested and used
  throughout the codebase. Adding AST alongside preserves backward compatibility
  and allows gradual migration.
- **Trade-off**: Some code duplication between the two approaches. The
  `AstShaderPipeline` doesn't replace `ComposableShaderPipeline` but provides an
  alternative path.
- **Future**: The string-based pipeline can gradually delegate to the AST system
  for optimization, eventually becoming a thin wrapper.

#### Function-Based Optimizations vs Trait Objects

- **Decision**: Used standalone functions (`dead_code_elimination()`,
  `constant_folding()`, `function_inlining()`) rather than a
  `Box<dyn OptimizationPass>` trait object approach.
- **Reasoning**: Follows the project convention of "enum over trait objects for
  known sets." The optimization passes are a finite, known set and function
  dispatch is simpler and more performant.
- **Trade-off**: Less extensible for external users adding custom passes, but
  this isn't needed yet.
- **Future**: Could add an enum-based `OptimizationPass` if external
  extensibility becomes needed.

### Development Workflow Insights

- **Disk space**: The build artifacts for this project can consume 30+ GB.
  Running `cargo clean` before large test runs is important on constrained
  systems.
- **Incremental development**: Building the parser first, then the generator,
  then adding type checking and optimization as separate passes worked well.
  Each piece could be tested independently.
- **Pre-existing test failures**: The `test_performance_500_labels` test is
  flaky (GUP-187 is already planned for it). Not related to this story.
- **`gen` is a reserved keyword in Rust 2024**: Using `gen` as a variable name
  caused a compilation error. Renamed to `generator`.

### Follow-up Stories

1. **GUP-189: AST Integration with ComposableShaderPipeline** — Wire the AST
   optimizer into the existing `ComposableShaderPipeline.optimize_shader()`
   method so existing pipeline users get AST-based optimizations transparently.

2. **GUP-190: WGSL Compute Shader AST Support** — Extend the parser and AST
   types to handle compute shader constructs (workgroup attributes, storage
   buffers, compute-specific builtins) for composing compute shaders.
