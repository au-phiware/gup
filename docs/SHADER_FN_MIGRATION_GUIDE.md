# Shader Function Migration Guide

## Overview

Gup provides two macro-based approaches for defining GPU shader functions:

| Approach            | Macro              | Body Syntax                        | Status |
| ------------------- | ------------------ | ---------------------------------- | ------ |
| **WGSL-native**     | `#[wgsl_function]` | WGSL in Rust function bodies       | Stable |
| **Rust-transpiled** | `#[shader_fn]`     | Idiomatic Rust, transpiled to WGSL | New    |

Both produce types implementing `ComposableShaderFunction`, making them fully
interchangeable. You can mix functions from either approach in the same
`ShaderPipeline`.

---

## Side-by-Side Examples

### Simple Function (no uniforms)

**`#[wgsl_function]`** — WGSL syntax in the function body:

```rust
use gup_macros::wgsl_function;

#[wgsl_function]
fn double_value(value: f32) -> f32 {
    return value * 2.0;
}
```

**`#[shader_fn]`** — Rust syntax, transpiled to WGSL:

```rust
use gup_macros::shader_fn;

#[shader_fn]
fn double_value(value: f32) -> f32 {
    return value * 2.0;
}
```

For simple expressions the bodies look identical. The difference shows up with
method calls, control flow, and Rust idioms.

### Function with Uniforms

Parameters after the first are packed into a uniform struct.

**`#[wgsl_function]`**:

```rust
#[wgsl_function]
fn linear_scale(value: f32, domain_min: f32, domain_max: f32,
                range_min: f32, range_max: f32) -> f32 {
    let normalised = (value - domain_min) / (domain_max - domain_min);
    return range_min + normalised * (range_max - range_min);
}
```

**`#[shader_fn]`**:

```rust
#[shader_fn]
fn linear_scale(value: f32, domain_min: f32, domain_max: f32,
                range_min: f32, range_max: f32) -> f32 {
    let normalised = (value - domain_min) / (domain_max - domain_min);
    range_min + normalised * (range_max - range_min)
}
```

Both generate:

- `LinearScale` struct with `domain_min`, `domain_max`, `range_min`, `range_max`
  fields
- `LinearScaleUniforms` GPU uniform struct
- `ComposableShaderFunction` implementation

### Method Calls

**`#[wgsl_function]`** uses WGSL built-in functions:

```rust
#[wgsl_function]
fn safe_sqrt(value: f32) -> f32 {
    return sqrt(clamp(value, 0.0, 100.0));
}
```

**`#[shader_fn]`** supports Rust method syntax — the transpiler maps to WGSL:

```rust
#[shader_fn]
fn safe_sqrt(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 100.0);
    return sqrt(clamped);
}
```

Supported method mappings include:

| Rust method                  | WGSL function                |
| ---------------------------- | ---------------------------- |
| `.abs()`                     | `abs(x)`                     |
| `.sqrt()`                    | `sqrt(x)`                    |
| `.sin()`, `.cos()`, `.tan()` | `sin(x)`, `cos(x)`, `tan(x)` |
| `.length()`                  | `length(x)`                  |
| `.normalize()`               | `normalize(x)`               |
| `.dot(other)`                | `dot(x, other)`              |
| `.cross(other)`              | `cross(x, other)`            |
| `.clamp(lo, hi)`             | `clamp(x, lo, hi)`           |
| `.min(other)`                | `min(x, other)`              |
| `.max(other)`                | `max(x, other)`              |
| `.to_f32()`                  | `f32(x)`                     |
| `.to_i32()`                  | `i32(x)`                     |
| `.to_u32()`                  | `u32(x)`                     |

### Control Flow

**`#[wgsl_function]`** — must write WGSL `if` statements:

```rust
#[wgsl_function]
fn classify(value: f32) -> f32 {
    if (value > 1.0) {
        return 2.0;
    } else if (value > 0.0) {
        return 1.0;
    } else {
        return 0.0;
    }
}
```

**`#[shader_fn]`** — writes Rust `if`/`else`, transpiled automatically:

```rust
#[shader_fn]
fn classify(value: f32) -> f32 {
    if value > 1.0 {
        return 2.0;
    } else if value > 0.0 {
        return 1.0;
    } else {
        return 0.0;
    }
}
```

### Loops

**`#[shader_fn]`** supports `for`, `while`, and `loop`:

```rust
#[shader_fn]
fn sum_range(n: i32) -> i32 {
    let mut sum = 0;
    for i in 0..n {
        sum += i;
    }
    return sum;
}
```

Transpiles to:

```wgsl
fn sum_range(n: i32) -> i32 {
    var sum = 0;
    for (var i = 0; i < n; i++) {
        sum += i;
    }
    return sum;
}
```

### Vector and Matrix Types

Both macros support the same type names (`Vec2`, `Vec3`, `Vec4`, `Mat2`, `Mat3`,
`Mat4`, etc.), which map to WGSL vector and matrix types.

**`#[shader_fn]`** additionally supports constructor syntax:

```rust
#[shader_fn]
fn make_position(x: f32) -> Vec2 {
    return Vec2(x, 0.0);
}
```

---

## Generated Output

Both macros produce the same output structure:

```rust
// Configuration struct
pub struct LinearScale {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
}

// GPU uniform struct
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearScaleUniforms {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
}

impl ComposableShaderFunction for LinearScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = LinearScaleUniforms;
    // ...
}
```

---

## Mixing in a Pipeline

Functions from both macros can be freely combined:

```rust
use gup::shader_pipeline::ComposableShaderPipeline;
use gup_macros::{shader_fn, wgsl_function};

#[wgsl_function]
fn scale(value: f32, factor: f32) -> f32 {
    return value * factor;
}

#[shader_fn]
fn clamp_positive(value: f32) -> f32 {
    if value < 0.0 {
        return 0.0;
    }
    return value;
}

let mut pipeline = ComposableShaderPipeline::new();
pipeline.add_function(Scale::new(2.0));
pipeline.add_function(ClampPositive::new());
```

---

## Feature Comparison

| Feature                | `#[wgsl_function]` | `#[shader_fn]`              |
| ---------------------- | ------------------ | --------------------------- |
| Arithmetic expressions | ✅                 | ✅                          |
| Comparison/logical ops | ✅                 | ✅                          |
| `let` bindings         | ✅                 | ✅ (including `let mut`)    |
| `if`/`else`            | ✅                 | ✅                          |
| `for` loops            | ✅ (WGSL syntax)   | ✅ (`for i in 0..n`)        |
| `while`/`loop`         | ✅ (WGSL syntax)   | ✅                          |
| `break`/`continue`     | ✅                 | ✅                          |
| Built-in functions     | ✅ (WGSL names)    | ✅ (Rust method syntax too) |
| Vector constructors    | ✅ (WGSL syntax)   | ✅ (`Vec3(x, y, z)`)        |
| Type casts             | ✅ (WGSL syntax)   | ✅ (`x as f32`)             |
| Uniform parameters     | ✅                 | ✅                          |
| Custom struct types    | ✅                 | ✅                          |
| Pipeline composition   | ✅                 | ✅                          |
| IDE autocompletion     | Partial            | ✅                          |
| `match` expressions    | ❌                 | ❌                          |
| Closures               | ❌                 | ❌                          |
| Generics               | ❌                 | ❌                          |

---

## When to Use Which

**Use `#[wgsl_function]`** when:

- You need precise control over the generated WGSL
- You're porting existing WGSL code
- You want to use WGSL-specific features

**Use `#[shader_fn]`** when:

- You prefer writing idiomatic Rust
- You want IDE support (autocompletion, type checking on Rust syntax)
- You're iterating on logic and want Rust compile errors instead of WGSL errors
- You want to use Rust method syntax (`.abs()`, `.normalize()`)

---

## Migration Steps

To convert an existing `#[wgsl_function]` to `#[shader_fn]`:

1. Change the attribute from `#[wgsl_function]` to `#[shader_fn]`
2. Replace WGSL-specific syntax with Rust equivalents:
   - Add conditions without parentheses: `if (x > 0.0)` → `if x > 0.0`
   - Use Rust `for`: `for (var i = 0; i < n; i++)` → `for i in 0..n`
   - Optionally use method syntax: `sqrt(x)` → `x.sqrt()` (both work)
3. Verify compilation — the transpiler will report errors for unsupported
   constructs

No changes are needed to any calling code; the generated types are identical.
