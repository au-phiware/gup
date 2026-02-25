# Rust-to-WGSL Transpilation: Architecture Design

## 1. System Overview

The transpilation system converts Rust functions annotated with
`#[wgsl_function]` into valid WGSL shader code at compile time. It is designed
as a modular pipeline with clear interfaces between phases, enabling incremental
feature additions and independent testing of each component.

```
┌─────────────────────────────────────────────────────────────────┐
│                     #[wgsl_function]                            │
│                     (proc macro entry)                          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                   ┌───────▼───────┐
                   │  syn Parser   │  Rust source → syn AST
                   │ (Phase 1)     │  (existing infrastructure)
                   └───────┬───────┘
                           │ syn::ItemFn
                   ┌───────▼───────┐
                   │  RustToWgsl   │  syn AST → WGSL AST
                   │  Converter    │  (transpile/convert.rs)
                   │  (Phase 2)    │
                   └───────┬───────┘
                           │ WgslFunction + WgslModule
                   ┌───────▼───────┐
                   │  Optimizer    │  WGSL AST → optimised AST
                   │  (Phase 3)    │  (optional, future)
                   └───────┬───────┘
                           │ WgslFunction (optimised)
                   ┌───────▼───────┐
                   │  WgslCodeGen  │  WGSL AST → WGSL text
                   │  (Phase 4)    │  (transpile/codegen.rs)
                   └───────┬───────┘
                           │ String (WGSL source)
                   ┌───────▼───────┐
                   │  Code Gen     │  WGSL text → Rust tokens
                   │  (Phase 5)    │  (quote! macro expansion)
                   └───────────────┘
```

## 2. Module Architecture

### 2.1 gup-macros/src/transpile/

The transpiler lives in the proc macro crate as a set of modules:

```
gup-macros/src/transpile/
├── mod.rs          — Module root, re-exports
├── ast.rs          — WGSL AST type definitions
├── convert.rs      — syn::Expr → WgslExpr conversion
├── codegen.rs      — WgslExpr → WGSL text generation
├── types.rs        — Type mapping (Rust ↔ WGSL) [future: GUP-056]
├── builtins.rs     — Built-in function registry [future: GUP-059]
├── control_flow.rs — Control flow translation [future: GUP-058]
└── pipeline_tests.rs — End-to-end pipeline tests
```

### 2.2 Interface Boundaries

Each phase has a clear input/output contract:

| Phase    | Input              | Output          | Module       |
| -------- | ------------------ | --------------- | ------------ |
| Parse    | Rust source tokens | `syn::ItemFn`   | syn          |
| Convert  | `syn::ItemFn`      | `WgslFunction`  | convert.rs   |
| Optimise | `WgslFunction`     | `WgslFunction`  | optimizer.rs |
| Generate | `WgslFunction`     | `String` (WGSL) | codegen.rs   |
| Emit     | `String` (WGSL)    | `TokenStream`   | quote!       |

### 2.3 AST Type Hierarchy

```
WgslModule
├── Vec<WgslStructDef>
│   └── Vec<WgslField> { name, ty: WgslType }
└── Vec<WgslFunction>
    ├── name: String
    ├── params: Vec<WgslParam> { name, ty: WgslType }
    ├── return_type: WgslType
    └── body: Vec<WgslStatement>
        ├── Let { name, ty, value: WgslExpr, mutable }
        ├── Return(Option<WgslExpr>)
        ├── If { condition, body, else_body }
        ├── Expression(WgslExpr)
        └── Assign(WgslExpr, WgslExpr)

WgslExpr
├── Literal(Literal)        — 1.0, 42, true
├── Ident(String)           — variable reference
├── Binary(Box, BinaryOp, Box)  — a + b
├── Unary(UnaryOp, Box)     — -x, !b
├── Call(String, Vec)        — clamp(x, 0.0, 1.0)
├── TypeConstructor(WgslType, Vec) — vec3<f32>(x, y, z)
├── MemberAccess(Box, String) — uniforms.scale
├── IndexAccess(Box, Box)    — arr[i]
├── Paren(Box)              — (expr)
└── Cast(WgslType, Box)     — f32(x)

WgslType
├── Scalar(ScalarType)      — f32, i32, u32, bool
├── Vector(ScalarType, u8)  — vec2<f32>, vec3<f32>, vec4<f32>
├── Matrix(ScalarType, u8, u8) — mat4x4<f32>
├── Array(Box, u32)         — array<f32, 4>
├── Struct(String)          — MyUniforms
└── Void                    — no return value
```

## 3. Extensibility Design

### 3.1 Adding New Expression Types

To support a new Rust expression type:

1. Add a variant to `WgslExpr` (if needed) in `ast.rs`
2. Add a conversion case in `RustToWgsl::convert_expr()` in `convert.rs`
3. Add a generation case in `WgslCodeGen::generate_expr()` in `codegen.rs`
4. Add unit tests for the new expression type
5. Add a WGSL validation test in `tests/transpile_wgsl_validation.rs`

### 3.2 Adding New Built-in Functions

The method-to-function mapping in `convert.rs` uses a match statement. To add a
new built-in:

1. Add the method name to the appropriate match arm in
   `RustToWgsl::convert_expr()` (method call case)
2. If the function has a different name in WGSL, add the mapping

A future `builtins.rs` module (GUP-059) will centralise this mapping into a
registry data structure for easier maintenance.

### 3.3 Adding New Type Mappings

To support a new Rust → WGSL type mapping:

1. Add the type to `RustToWgsl::convert_type()` in `convert.rs`
2. Add display formatting in `WgslType::Display` in `ast.rs`
3. Ensure the existing `TYPE_CACHE` in `wgsl_function.rs` stays in sync

A future `types.rs` module (GUP-056) will provide a comprehensive type registry.

## 4. Integration Strategy

### 4.1 Coexistence with Current System

The transpiler **coexists** with the current string-based `#[wgsl_function]`
system. No existing code needs to change. The integration path is:

1. **Phase A (current)**: Prototype transpiler exists alongside the current
   macro. Users continue using the existing macro.

2. **Phase B (GUP-061)**: Wire the transpiler into `#[wgsl_function]` as an
   alternative code path. Add a macro attribute to opt in:

   ```rust
   #[wgsl_function(transpile)]  // Uses new transpiler
   fn my_func(value: f32) -> f32 { ... }
   ```

3. **Phase C (future)**: Make transpiler the default, with string-based system
   as fallback for constructs not yet supported.

4. **Phase D (future)**: Deprecate string-based system once transpiler reaches
   feature parity.

### 4.2 Connection to shader_ast

The existing `shader_ast` module (WGSL parser, optimizer, generator) operates on
WGSL text and its own AST. The transpiler's WGSL AST is a lightweight mirror of
`shader_ast::types` designed to avoid circular dependencies.

Future integration options:

- **Option A**: Extract shared types into `gup-ast-types` crate used by both
  `gup-macros` and `gup`
- **Option B**: Generate WGSL text from the transpiler, then parse it with
  `shader_ast::parser` for optimization. This adds a text round-trip but
  requires no shared dependency.
- **Option C**: Keep the types separate but ensure they're structurally
  identical, allowing serialisation-based conversion.

**Recommendation**: Option B for simplicity — the text round-trip is cheap and
avoids any new crate management overhead.

### 4.3 Uniform Parameter Handling

The transpiler automatically rewrites uniform parameters:

```rust
// Input Rust:
fn scale(value: f32, factor: f32, offset: f32) -> f32 {
    return value * factor + offset;
}

// Generated WGSL:
fn scale(value: f32, uniforms: ScaleUniforms) -> f32 {
    return value * uniforms.factor + uniforms.offset;
}
```

This matches the existing `#[wgsl_function]` behavior, ensuring backward
compatibility.

## 5. Testing Strategy

### 5.1 Test Layers

| Layer             | What it tests                    | Where                      |
| ----------------- | -------------------------------- | -------------------------- |
| Unit tests        | Individual conversion functions  | `convert.rs`, `codegen.rs` |
| Pipeline tests    | Full Rust → WGSL pipeline        | `pipeline_tests.rs`        |
| Validation tests  | Generated WGSL compiles (naga)   | `tests/transpile_*.rs`     |
| Integration tests | Works with shader function trait | Future (GUP-061)           |

### 5.2 Test Patterns

Every new feature should include:

1. A unit test in the converter module
2. A pipeline test demonstrating the full flow
3. A WGSL validation test confirming the output compiles
4. A negative test verifying clear errors for unsupported input

## 6. Story Dependency Graph

```
GUP-055 (Research + Prototype) ← YOU ARE HERE
    │
    ├── GUP-056 (Type System Mapping)
    │   └── Full Rust↔WGSL type mapping, struct handling
    │
    ├── GUP-057 (Expression Transpilation)
    │   └── Complete expression coverage, operator precedence
    │
    ├── GUP-058 (Control Flow)
    │   └── if/else, for, while, match→if lowering
    │
    ├── GUP-059 (Built-in Function Library)
    │   └── Comprehensive math/vector function registry
    │
    ├── GUP-060 (Optimisation + Errors)
    │   └── Constant folding, dead code, error formatting
    │
    └── GUP-061 (Integration)
        └── Wire into #[wgsl_function], migration path
            │
            └── GUP-062 (Community Validation)
                └── Real-world testing, documentation
```

## 7. Design Decisions Log

| #   | Decision                             | Reasoning                          | Trade-off                                      |
| --- | ------------------------------------ | ---------------------------------- | ---------------------------------------------- |
| 1   | Lightweight AST in gup-macros        | Avoids circular dependency         | Small type duplication                         |
| 2   | Direct WGSL text output              | Simplicity, no shared crate needed | Text round-trip if AST optimisation needed     |
| 3   | Method→function mapping in converter | Rust idioms feel natural           | Requires explicit listing of supported methods |
| 4   | Uniform param rewriting              | Consistent with existing system    | Extra complexity in converter                  |
| 5   | TranspileError with Span             | Good IDE error experience          | Slightly more complex error handling           |
| 6   | Separate pipeline_tests module       | Clean test organisation            | Additional file to maintain                    |
