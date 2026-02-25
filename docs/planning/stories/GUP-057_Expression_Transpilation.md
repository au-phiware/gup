# GUP-057: Expression and Operator Transpilation

## Story Overview

**Title**: Implement Rust Expression and Operator Transpilation to WGSL  
**Epic**: Phase 2 Initiative 3 - Rust-to-WGSL Transpilation  
**Priority**: High  
**Story Points**: 13  
**Status**: 🚧 In Progress

## Context

With the type system mapping established in GUP-056, we need to implement
comprehensive expression transpilation that can handle Rust expressions,
operators, method calls, and built-in functions, converting them to equivalent
WGSL syntax.

## User Story

**As a** shader function developer  
**I want** to write shader expressions using natural Rust syntax  
**So that** I can leverage familiar language constructs and IDE support while
targeting GPU execution

## Problem Statement

Rust and WGSL have different expression syntax, operator precedence, and
built-in function availability. We need a transpilation system that can
accurately convert Rust expressions while:

- Maintaining semantic equivalence
- Handling operator precedence correctly
- Mapping built-in functions appropriately
- Providing clear error messages for unsupported constructs

## Acceptance Criteria

### AC1: Arithmetic and Logical Operators

- [ ] Support basic arithmetic (+, -, \*, /, %)
- [ ] Handle comparison operators (==, !=, <, <=, >, >=)
- [ ] Implement logical operators (&&, \|\|, !)
- [ ] Support bitwise operations (&, \|, ^, <<, >>)
- [ ] Maintain correct operator precedence

### AC2: Variable Access and Assignment

- [ ] Handle local variable references
- [ ] Support struct field access (dot notation)
- [ ] Implement array/vector indexing
- [ ] Support uniform parameter access
- [ ] Handle mutable vs immutable variable semantics

### AC3: Function Calls and Methods

- [ ] Transpile function calls with proper argument mapping
- [ ] Support vector/matrix method calls (length, normalize, etc.)
- [ ] Handle built-in math functions (sin, cos, sqrt, etc.)
- [ ] Implement constructor calls for vectors and matrices
- [ ] Support method chaining where applicable

### AC4: Complex Expressions

- [ ] Handle nested expressions with correct parenthesization
- [ ] Support conditional expressions (if expressions)
- [ ] Implement tuple construction and destruction
- [ ] Handle type casting and conversions
- [ ] Support range expressions for loops

## Technical Requirements

### Expression AST Mapping

```rust
// Core expression transpiler trait
pub trait ExpressionTranspiler {
    fn transpile_expr(&mut self, expr: &Expr) -> Result<String, TranspileError>;
    fn transpile_binary_op(&mut self, left: &Expr, op: &BinOp, right: &Expr) -> Result<String, TranspileError>;
    fn transpile_method_call(&mut self, receiver: &Expr, method: &Ident, args: &[Expr]) -> Result<String, TranspileError>;
    fn transpile_function_call(&mut self, path: &Path, args: &[Expr]) -> Result<String, TranspileError>;
}

// Expression context for variable tracking
pub struct ExpressionContext {
    variables: HashMap<Ident, WgslTypeInfo>,
    functions: HashMap<Path, FunctionSignature>,
    current_scope: ScopeId,
    uniform_access: UniformAccessTracker,
}
```

### Operator Mapping Table

| Rust Operator | WGSL Equivalent | Notes          |
| ------------- | --------------- | -------------- |
| `+`           | `+`             | Direct mapping |
| `-`           | `-`             | Direct mapping |
| `*`           | `*`             | Direct mapping |
| `/`           | `/`             | Direct mapping |
| `%`           | `%`             | Direct mapping |
| `==`          | `==`            | Direct mapping |
| `!=`          | `!=`            | Direct mapping |
| `&&`          | `&&`            | Direct mapping |
| `!`           | `!`             | Direct mapping |

### Built-in Function Mapping

| Rust Function             | WGSL Equivalent    | Notes              |
| ------------------------- | ------------------ | ------------------ |
| `f32::sin(x)`             | `sin(x)`           | Direct mapping     |
| `f32::cos(x)`             | `cos(x)`           | Direct mapping     |
| `f32::sqrt(x)`            | `sqrt(x)`          | Direct mapping     |
| `f32::abs(x)`             | `abs(x)`           | Direct mapping     |
| `f32::min(a, b)`          | `min(a, b)`        | Direct mapping     |
| `f32::max(a, b)`          | `max(a, b)`        | Direct mapping     |
| `Vec3::length(&self)`     | `length(self)`     | Method to function |
| `Vec3::normalize(&self)`  | `normalize(self)`  | Method to function |
| `Vec3::dot(&self, other)` | `dot(self, other)` | Method to function |

### Implementation Examples

1. **Basic Arithmetic**

   ```rust
   // Rust input
   let result = a * 2.0 + b / c;

   // WGSL output
   let result = a * 2.0 + b / c;
   ```

2. **Vector Operations**

   ```rust
   // Rust input
   let magnitude = position.length();
   let normalized = velocity.normalize();

   // WGSL output
   let magnitude = length(position);
   let normalized = normalize(velocity);
   ```

3. **Complex Expressions**

   ```rust
   // Rust input
   let distance = (point1 - point2).length();
   let interpolated = a * (1.0 - t) + b * t;

   // WGSL output
   let distance = length(point1 - point2);
   let interpolated = a * (1.0 - t) + b * t;
   ```

4. **Conditional Expressions**

   ```rust
   // Rust input
   let result = if condition { value_a } else { value_b };

   // WGSL output
   let result = select(value_b, value_a, condition);
   ```

### Error Handling Strategy

```rust
#[derive(Debug, Clone)]
pub enum TranspileError {
    UnsupportedOperator { operator: String, span: Span },
    UnsupportedFunction { function: String, span: Span },
    TypeMismatch { expected: String, found: String, span: Span },
    UndefinedVariable { name: String, span: Span },
    InvalidMethodCall { receiver_type: String, method: String, span: Span },
}

impl TranspileError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        // Generate helpful error messages with suggestions
        match self {
            UnsupportedOperator { operator, span } => {
                Diagnostic::error(format!("Operator '{}' is not supported in WGSL", operator))
                    .span(*span)
                    .suggestion("Consider using an equivalent WGSL operation")
            }
            // ... other error types
        }
    }
}
```

## Dependencies

- GUP-055: AST parsing foundation
- GUP-056: Type system mapping for expression type checking
- syn crate: For parsing Rust expressions
- proc-macro2: For span information and error reporting

## Definition of Done

- [ ] Complete expression transpilation for all supported operators
- [ ] Built-in function mapping system with extensible architecture
- [ ] Comprehensive error handling with helpful diagnostics
- [ ] Integration with type system for expression validation
- [ ] Performance benchmarks showing efficient transpilation
- [ ] Test suite covering all expression types and edge cases

## Test Requirements

### Unit Tests

```rust
#[test]
fn test_arithmetic_transpilation() {
    let input = parse_expr("a + b * c").unwrap();
    let transpiler = ExpressionTranspiler::new();
    let result = transpiler.transpile_expr(&input).unwrap();
    assert_eq!(result, "a + b * c");
}

#[test]
fn test_vector_method_transpilation() {
    let input = parse_expr("position.length()").unwrap();
    let transpiler = ExpressionTranspiler::new();
    let result = transpiler.transpile_expr(&input).unwrap();
    assert_eq!(result, "length(position)");
}

#[test]
fn test_conditional_expression() {
    let input = parse_expr("if flag { a } else { b }").unwrap();
    let transpiler = ExpressionTranspiler::new();
    let result = transpiler.transpile_expr(&input).unwrap();
    assert_eq!(result, "select(b, a, flag)");
}
```

### Integration Tests

```rust
#[test]
fn test_complex_shader_expression() {
    let shader_fn = wgsl_function! {
        fn lighting_calculation(position: Vec3, normal: Vec3, light_pos: Vec3) -> f32 {
            let light_dir = (light_pos - position).normalize();
            let diffuse = normal.dot(light_dir).max(0.0);
            diffuse * 0.8 + 0.2
        }
    };

    let wgsl = shader_fn.generated_wgsl();
    assert!(wgsl.contains("normalize(light_pos - position)"));
    assert!(wgsl.contains("max(dot(normal, light_dir), 0.0)"));
}
```

### Error Handling Tests

```rust
#[test]
fn test_unsupported_operator_error() {
    let input = parse_expr("a << b").unwrap(); // Bitshift not supported
    let transpiler = ExpressionTranspiler::new();
    let result = transpiler.transpile_expr(&input);

    assert!(matches!(result, Err(TranspileError::UnsupportedOperator { .. })));
}
```

## Performance Considerations

- **Compilation Time**: Expression transpilation should add minimal overhead to
  macro expansion
- **Generated Code Quality**: Ensure transpiled expressions are as efficient as
  hand-written WGSL
- **Memory Usage**: Minimize allocations during transpilation process
- **Caching**: Cache commonly used expression patterns for faster transpilation

## Future Considerations

This implementation enables:

- GUP-058: Control flow handling with expression-aware condition transpilation
- GUP-059: Built-in function library expansion
- Advanced optimization passes for common expression patterns
- Support for custom operator overloading in future versions
