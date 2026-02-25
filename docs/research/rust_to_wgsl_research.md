# Rust-to-WGSL Transpilation: Research Report

## Executive Summary

This report analyzes existing approaches to Rust-to-GPU compilation and WGSL
generation, evaluating their suitability for Gup's shader function system. We
compare three primary strategies: leveraging rust-gpu's SPIR-V backend, using
naga as an intermediate representation, and building a custom syn-based
transpiler. Our recommendation is to extend Gup's existing `#[wgsl_function]`
proc macro with a modular Rust-to-WGSL transpilation pipeline built on `syn`,
producing WGSL AST nodes that integrate with the existing `shader_ast` module.

## 1. Existing Solutions Analysis

### 1.1 rust-gpu (Embark Studios)

**Architecture**: Custom `rustc` codegen backend that compiles Rust to SPIR-V.

**Strengths**:

- Full Rust language support including traits, generics, and closures
- Leverages rustc's optimizer (LLVM) for code quality
- True Rust compilation — not a subset or DSL
- Active open-source project with community contributions

**Limitations**:

- Requires a custom toolchain (`rust-toolchain.toml` with specific nightly)
- Heavy dependency: pulls in the entire rustc compilation pipeline
- SPIR-V output requires translation to WGSL (via naga) — extra step
- Complex build setup not suitable for library consumers
- Limited to GPU-compatible Rust subset at runtime (panics on unsupported ops)
- Nightly-only, frequent breakage with Rust version updates

**Relevance to Gup**: rust-gpu's approach is too heavyweight for a library that
wants zero-friction proc macro usage. However, its handling of type mapping
(Rust types ↔ SPIR-V types) provides valuable reference material.

### 1.2 naga (gfx-rs)

**Architecture**: Multi-target shader IR (Intermediate Representation) with
frontends (WGSL, GLSL, SPIR-V) and backends (WGSL, SPIR-V, MSL, HLSL, GLSL).

**Strengths**:

- Mature, battle-tested (used by wgpu, Firefox)
- Excellent WGSL validation capabilities
- Already a transitive dependency via wgpu
- Rich IR that captures GPU semantics precisely
- Cross-platform shader translation

**Limitations**:

- No Rust frontend — designed for shader language inputs only
- IR is lower-level than what we need for Rust expression mapping
- Building a Rust → naga IR frontend would be substantial work
- naga's IR assumes GPU execution model from the start

**Relevance to Gup**: naga is invaluable for **validation** of generated WGSL
(already available via `wgpu::Device::create_shader_module`), but building a
Rust-to-naga-IR frontend is disproportionate effort. Better to generate WGSL
text directly and validate with naga.

### 1.3 syn (dtolnay)

**Architecture**: Complete Rust syntax parser producing a full AST.

**Strengths**:

- De facto standard for Rust proc macros (used by serde, tokio, etc.)
- Complete Rust syntax support with `features = ["full"]`
- Excellent error reporting with span information
- Already a dependency of gup-macros
- Zero runtime cost — all work happens at compile time
- Stable, well-maintained, minimal breaking changes

**Limitations**:

- Parses syntax only — no type resolution or trait solving
- Cannot determine concrete types for generics
- No semantic analysis (e.g., cannot resolve `foo.bar()` method)

**Relevance to Gup**: syn is the ideal foundation for our transpiler. The
`#[wgsl_function]` macro already uses syn for parsing. The gap is in the
**translation layer** — converting `syn::Expr` variants systematically into
WGSL constructs.

### 1.4 Comparison Matrix

| Criterion          | rust-gpu      | naga IR       | syn + custom  |
| ------------------ | ------------- | ------------- | ------------- |
| Setup complexity   | Very High     | Medium        | **Low**       |
| Rust coverage      | **Full**      | N/A           | Subset        |
| Build time impact  | Very High     | Medium        | **Minimal**   |
| WGSL output        | Indirect      | **Direct**    | **Direct**    |
| Validation         | Strong        | **Strongest** | Good (+ naga) |
| Maintenance burden | High          | Low           | **Medium**    |
| Library-friendly   | No            | Partial       | **Yes**       |
| Existing in Gup    | No            | Via wgpu      | **Yes**       |

## 2. Transpilation Strategies

### 2.1 Direct String Generation (Current Approach)

The existing `#[wgsl_function]` macro translates Rust expressions directly to
WGSL strings via `translate_expr_to_wgsl()`. This works but has limitations:

- No intermediate representation — hard to optimize or validate
- Ad-hoc mapping — each expression type handled case-by-case
- Limited composability — string concatenation doesn't preserve structure
- Difficult to extend with new Rust constructs

### 2.2 Rust syn → WGSL AST → WGSL Text (Recommended)

Introduce a two-phase pipeline:

1. **Phase A**: Convert `syn::Expr` / `syn::Stmt` → `shader_ast::types::Expr` /
   `Statement`
2. **Phase B**: Use existing `shader_ast::generator` to produce WGSL text

**Benefits**:

- Leverages existing AST infrastructure (types, generator, optimizer)
- Enables optimization passes on the intermediate AST
- Clean separation of concerns (parsing vs generation)
- Extensible — new Rust constructs require only Phase A additions

**Challenge**: The `shader_ast` types live in the main `gup` crate, while proc
macros live in `gup-macros`. Due to Rust's proc macro compilation model, proc
macros cannot depend on the crate they're used in (circular dependency).

**Solutions**:

1. **Shared types crate**: Extract AST types into `gup-ast-types` crate
2. **Duplicate lightweight types**: Define a minimal AST in gup-macros that
   mirrors the essential `shader_ast::types` structures
3. **Generate WGSL strings directly**: Keep using string generation in the
   macro, but structure the code as a modular pipeline

We recommend option (2) for the prototype, with a path to option (1) for
production. The duplication is manageable since the AST types are simple enums
and structs with no complex behavior.

### 2.3 Embedded DSL (eDSL) Approach

Instead of transpiling Rust syntax, define a Rust eDSL that builds shader IR:

```rust
let shader = wgsl! {
    let x = input.value * uniform.scale;
    let y = clamp(x, 0.0, 1.0);
    output(y)
};
```

**Pros**: Maximum control over semantics, no parsing ambiguity.
**Cons**: Different syntax from standard Rust, learning curve, poor IDE support.

**Assessment**: Not recommended — it sacrifices the primary goal of "write
natural Rust syntax."

## 3. Supported Rust Language Features

### 3.1 Well-Supported (Direct Mapping)

| Rust Construct              | WGSL Equivalent         | Complexity |
| --------------------------- | ----------------------- | ---------- |
| `let x = expr;`            | `let x = expr;`         | Trivial    |
| `let mut x = expr;`        | `var x = expr;`         | Trivial    |
| `x + y`, `x * y`, etc.     | `x + y`, `x * y`        | Trivial    |
| `return expr;`             | `return expr;`          | Trivial    |
| `if cond { } else { }`     | `if (cond) { } else {}` | Simple     |
| `expr.field`               | `expr.field`            | Simple     |
| `func(args)`               | `func(args)`            | Simple     |
| `-x`, `!x`                 | `-x`, `!x`             | Trivial    |
| `(expr)`                   | `(expr)`               | Trivial    |
| Float/int/bool literals    | Same                    | Trivial    |
| `x as f32` (numeric casts) | `f32(x)`               | Simple     |
| `for` loops                | `for` loops             | Moderate   |

### 3.2 Requires Translation

| Rust Construct        | WGSL Equivalent                 | Complexity |
| --------------------- | ------------------------------- | ---------- |
| `x.abs()`             | `abs(x)`                        | Moderate   |
| `x.min(y)`            | `min(x, y)`                     | Moderate   |
| `x.sqrt()`            | `sqrt(x)`                       | Moderate   |
| `Vec2 { x, y }`       | `vec2<f32>(x, y)`               | Moderate   |
| `array[i]`            | `array[i]`                      | Simple     |
| Tuple struct access   | Member access                   | Moderate   |
| Type aliases          | N/A (inline)                    | Moderate   |

### 3.3 Unsupported (Must Error)

| Rust Construct       | Reason                                  |
| -------------------- | --------------------------------------- |
| Closures             | No GPU equivalent                       |
| Trait methods         | No trait dispatch on GPU                |
| Heap allocation      | No heap on GPU                          |
| String operations    | No strings on GPU                       |
| References/borrowing | GPU memory model is different            |
| Pattern matching     | No direct WGSL equivalent (use if/else) |
| Iterators            | No iterator protocol on GPU             |
| `async`/`await`      | No async on GPU                         |
| Generics             | WGSL has limited generics               |

## 4. Performance Implications

### 4.1 Compile-Time Overhead

Proc macro transpilation adds compile-time cost. Measurements with the
current `#[wgsl_function]` macro show negligible overhead (<10ms per function).
Adding an AST intermediate step should add minimal additional cost since:

- AST construction is O(n) in expression tree size
- Typical shader functions have <50 AST nodes
- No I/O or network operations involved

### 4.2 Generated WGSL Quality

Direct transpilation from Rust produces WGSL that is semantically equivalent
to hand-written code. The existing `shader_ast::optimizer` can then apply:

- **Constant folding**: Evaluate compile-time-known expressions
- **Dead code elimination**: Remove unreachable branches
- **Function inlining**: Inline small helper functions

These optimizations ensure generated WGSL matches hand-optimized performance.

### 4.3 Runtime Impact

Zero runtime impact — all transpilation happens at compile time. The generated
WGSL is a static string embedded in the binary, identical to hand-written WGSL.

## 5. Recommendations

### Primary Recommendation

Build a modular **Rust syn → WGSL** transpilation pipeline in `gup-macros`:

1. **Use `syn` for parsing** — already a dependency, well-understood
2. **Define lightweight AST types in gup-macros** — mirrors `shader_ast::types`
3. **Convert `syn::Expr` → internal AST → WGSL string**
4. **Validate with naga** (via wgpu) in integration tests
5. **Start with the supported subset** (§3.1) and expand incrementally

### Architecture Phases

1. **GUP-055** (this story): Research + prototype demonstrating the pipeline
2. **GUP-056**: Full type system mapping (Rust types ↔ WGSL types)
3. **GUP-057**: Complete expression and operator transpilation
4. **GUP-058**: Control flow (if/else, for loops, early return)
5. **GUP-059**: Built-in function library (math functions, type constructors)
6. **GUP-060**: Optimization and error reporting
7. **GUP-061**: Integration with existing shader function system

### Risk Mitigation

- **Start minimal**: Support only arithmetic, variables, and return statements
- **Validate early**: Use naga/wgpu to validate every generated WGSL snippet
- **Maintain backward compatibility**: Keep the current string-based system
  working alongside the new transpiler
- **Clear error messages**: When a Rust construct isn't supported, provide a
  helpful error with the span location

## Appendix: References

- [rust-gpu repository](https://github.com/EmbarkStudios/rust-gpu)
- [naga documentation](https://docs.rs/naga/)
- [syn documentation](https://docs.rs/syn/)
- [WGSL specification](https://www.w3.org/TR/WGSL/)
- [wgpu documentation](https://docs.rs/wgpu/)
