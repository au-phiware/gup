# Tutorial 3: Custom Shader Functions

> **Goal**: Move data transforms from the CPU to the GPU using the
> `#[wgsl_function]` macro and `attr_shader`.

## What You Will Learn

- When to use GPU shader functions versus CPU-side attribute closures
- How to annotate a Rust function with `#[wgsl_function]` to generate WGSL
- How to register a shader function via `Selection::attr_shader`
- How to parameterise shader functions with uniforms
- WGSL type constraints and the supported Rust type subset

## Prerequisites

Complete [Tutorial 2](02_data_binding.md). You should be comfortable with
`Selection<T, M>` and `.attr()` bindings.

## When to Use Shader Functions

In Tutorial 2 you used `.attr()` closures to transform data on the CPU. This
works well for small datasets, but it means every data point is transformed in
Rust, serialised, and uploaded to the GPU every frame.

**Shader functions** move the transform to the GPU:

| Approach | Where it runs | Best for |
|----------|--------------|----------|
| `.attr(name, closure)` | CPU | Simple mappings, string lookups, complex logic |
| `.attr_shader(name, extractor, shader_fn)` | GPU | Numeric transforms on large datasets |

With `attr_shader`, Gup uploads the raw extracted value once and applies the
transform in the vertex shader — thousands of times faster for large datasets.

## Step 1: Define a Shader Function

Use the `#[wgsl_function]` procedural macro from `gup_macros` to annotate a
Rust function. The macro transpiles the function body to WGSL and generates a
struct that implements `ComposableShaderFunction`:

```rust
use gup_macros::wgsl_function;

#[wgsl_function]
fn my_linear_scale(value: f32, scale: f32, offset: f32) -> f32 {
    return value * scale + offset;
}
```

This generates:

- A struct `MyLinearScale` with fields `scale: f32` and `offset: f32`
  (all parameters except the first are treated as uniforms).
- A `MyLinearScaleUniforms` struct that is `#[repr(C)]`, `Pod`, and `Zeroable`
  — ready for GPU upload.
- An implementation of `ComposableShaderFunction` that produces valid WGSL.

Create an instance by passing the uniform values:

```rust
let scale_fn = MyLinearScale::new(2.0, 1.0); // scale=2.0, offset=1.0
```

## Step 2: Wire It to a Selection

Use `attr_shader` instead of `attr`. The second argument is an extractor closure
that pulls the raw value from your data; the third is the shader function:

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct Measurement {
    timestamp: f32,
    value: f32,
}

let scale = MyLinearScale::new(2.0, -1.0); // map [0,1] → [-1, 1]

let mut selection = Selection::<Measurement, Circle>::from_data(data);
selection
    .attr("center", |d: &Measurement| [d.timestamp, 0.0]) // CPU binding for x
    .attr_shader("radius", |d: &Measurement| d.value, scale); // GPU binding
```

At render time, Gup injects the generated WGSL into the vertex shader and
uploads `MyLinearScaleUniforms { scale: 2.0, offset: -1.0 }` as a uniform
buffer. The GPU applies `value * 2.0 + (-1.0)` to every data point in parallel.

## Step 3: Use Vector Types

Shader functions are not limited to scalars. You can transform 2D positions,
colours, and more:

```rust
use gup::shader_function::Vec2;

#[wgsl_function]
fn viewport_transform(pos: Vec2, scale: Vec2) -> Vec2 {
    return pos * scale;
}

let transform = ViewportTransform::new(vec2![0.5, 0.5]);
selection.attr_shader("center", |d: &Measurement| [d.timestamp, d.value], transform);
```

The macro maps Rust types to WGSL types automatically:

| Rust type | WGSL type |
|-----------|-----------|
| `f32` | `f32` |
| `Vec2` | `vec2<f32>` |
| `Vec3` | `vec3<f32>` |
| `Vec4` | `vec4<f32>` |
| `Mat2` | `mat2x2<f32>` |
| `Mat3` | `mat3x3<f32>` |
| `Mat4` | `mat4x4<f32>` |

Only these types are supported in `#[wgsl_function]` signatures. For the full
transpiler reference see [Technical Approach](../TECHNICAL_APPROACH.md).

## Step 4: Validate the Generated WGSL

You can inspect the generated WGSL at any time:

```rust
let wgsl_code = MyLinearScale::wgsl_function();
println!("{}", wgsl_code);

// Check the function name
assert_eq!(MyLinearScale::function_name(), "my_linear_scale");
```

Write a test to verify your shader function compiles and produces the expected
WGSL:

```rust
#[test]
fn test_my_scale_generates_valid_wgsl() {
    let wgsl = MyLinearScale::wgsl_function();
    assert!(wgsl.contains("fn my_linear_scale"));
    assert!(wgsl.contains("f32"));

    let scale = MyLinearScale::new(2.0, 1.0);
    let uniforms = scale.create_uniforms().unwrap();
    assert_eq!(uniforms.scale, 2.0);
    assert_eq!(uniforms.offset, 1.0);
}
```

Run with:

```bash
cargo test test_my_scale -- --test-threads=1
```

## Step 5: Update Uniforms at Runtime

Shader function uniforms can be updated without rebuilding the pipeline. Use
`update_shader_uniforms` on the selection:

```rust
let new_scale = MyLinearScale::new(3.0, 0.5);
selection.update_shader_uniforms("radius", new_scale, &queue)?;
```

This uploads new uniform values to the existing GPU buffer — no recompilation,
no re-upload of vertex data.

## Built-in Shader Functions

Gup ships many pre-built shader functions in `gup::prelude`. Here are the most
common:

```rust
// Linear mapping from [domain_min, domain_max] to [range_min, range_max]
let scale = LinearScale::new(0.0, 100.0, -1.0, 1.0);

// Colour gradient between two endpoints
let gradient = ColorGradient::new(
    vec4![0.1, 0.2, 0.8, 1.0],   // start colour (blue)
    vec4![0.9, 0.1, 0.1, 1.0],   // end colour (red)
);
```

These work as drop-in arguments to `attr_shader`:

```rust
selection
    .attr_shader("center", |d: &MyData| [d.x, d.y], scale)
    .attr_shader("fill_color", |d: &MyData| d.value, gradient);
```

## Full Example

```rust
use gup::prelude::*;
use gup_macros::wgsl_function;
use std::sync::Arc;

#[wgsl_function]
fn temperature_to_radius(temp: f32, min_radius: f32, max_radius: f32) -> f32 {
    return min_radius + (max_radius - min_radius) * temp;
}

#[derive(Debug, Clone)]
struct WeatherReading {
    longitude: f32,
    latitude: f32,
    temperature: f32,    // normalised to [0, 1]
}

#[tokio::main]
async fn main() -> GupResult<()> {
    let data = vec![
        WeatherReading { longitude: 0.2, latitude: 0.3, temperature: 0.7 },
        WeatherReading { longitude: 0.5, latitude: 0.8, temperature: 0.4 },
        WeatherReading { longitude: 0.9, latitude: 0.1, temperature: 0.9 },
    ];

    let context = Arc::new(RenderContext::new().await?);
    let radius_fn = TemperatureToRadius::new(0.01, 0.08);

    let mut selection = Selection::<WeatherReading, Circle>::from_data(data);
    selection
        .attr("center", |d: &WeatherReading| {
            [d.longitude * 2.0 - 1.0, d.latitude * 2.0 - 1.0]
        })
        .attr_shader("radius", |d: &WeatherReading| d.temperature, radius_fn)
        .attr("fill_color", |d: &WeatherReading| {
            [d.temperature, 0.3, 1.0 - d.temperature, 0.8]
        });

    println!("Selection with GPU shader function ready ({} points)", selection.len());

    // Inspect the generated WGSL
    println!("Generated WGSL:\n{}", TemperatureToRadius::wgsl_function());

    Ok(())
}
```

## Key Concepts

| Concept | What It Does |
|---------|-------------|
| `#[wgsl_function]` | Transpiles a Rust function to WGSL and generates a shader struct |
| `attr_shader(name, extractor, shader_fn)` | Binds an attribute to a GPU shader function |
| `ComposableShaderFunction` | Trait implemented by all shader functions |
| `update_shader_uniforms()` | Updates uniform values without pipeline rebuild |

## Next Steps

- **[Tutorial 4: Interactions](04_interactions.md)** — add hover, click, and
  zoom/pan to your charts.
- **[Technical Approach](../TECHNICAL_APPROACH.md)** — deep dive into the
  unified shader function architecture and transpiler details.
- **[`shader_pipeline_demo` example](../../examples/shader_pipeline_demo.rs)** —
  composing multiple shader functions into a pipeline.
