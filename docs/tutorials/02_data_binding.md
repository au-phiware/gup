# Tutorial 2: Data Binding

> **Goal**: Bind a custom Rust struct to a `Selection<T, M>`, map its fields to
> visual attributes, and update the data at runtime.

## What You Will Learn

- How `Selection<T, M>` connects your data to the GPU
- What the mark type parameter `M` means and why it matters
- How to use `.attr()` to bind data fields to visual attributes
- How to update data with `set_data()` and re-render
- How to use `attr_parallel()` for batch bindings

## Prerequisites

Complete [Tutorial 1](01_getting_started.md) first. This tutorial builds on the
chart builder concepts introduced there by going one level deeper into the
`Selection` API that powers every Gup chart.

## The Selection Type

At the heart of Gup's rendering pipeline is `Selection<T, M>`:

```rust
pub struct Selection<T, M: Mark> { /* … */ }
```

- **`T`** — your data type (e.g. `SalesRecord`, `Measurement`, `Point`).
- **`M`** — the mark type that determines the visual representation. Built-in
  marks include `Circle`, `Rectangle`, `Line`, `BoxPlot`, `Text`, and `Path`.

The chart builders from Tutorial 1 (e.g. `scatter()`) create a
`Selection<T, Circle>` under the hood. When you need more control over how data
maps to visuals, you work with `Selection` directly.

### Why `M: Mark`?

The mark type parameter gives Gup compile-time knowledge of the GPU vertex
layout. Each `Mark` implementation defines:

- A `Vertex` type (the GPU-side data layout)
- Named attributes (e.g. `"center"`, `"radius"`, `"fill_color"`)
- Vertex and fragment shaders

This means Gup can catch mismatches between your attribute bindings and the mark
at compile time rather than at render time.

## Step 1: Define Your Data

Let's work with a richer data type than Tutorial 1's `Point`:

```rust
use gup::prelude::*;

#[derive(Debug, Clone)]
struct SalesRecord {
    product: String,
    revenue: f32,
    profit_margin: f32,
    category: u32,
}
```

Create some sample data:

```rust
let data = vec![
    SalesRecord { product: "Widget A".into(), revenue: 120.0, profit_margin: 0.35, category: 0 },
    SalesRecord { product: "Widget B".into(), revenue: 280.0, profit_margin: 0.22, category: 0 },
    SalesRecord { product: "Gadget X".into(), revenue: 450.0, profit_margin: 0.48, category: 1 },
    SalesRecord { product: "Gadget Y".into(), revenue: 310.0, profit_margin: 0.61, category: 1 },
    SalesRecord { product: "Gizmo Z".into(),  revenue: 190.0, profit_margin: 0.15, category: 2 },
];
```

## Step 2: Create a Selection

Create a `Selection` that binds your data to the `Circle` mark:

```rust
let mut selection = Selection::<SalesRecord, Circle>::from_data(data);
```

`from_data()` stores the data and prepares the selection for attribute binding.
No GPU work happens yet — Gup uses lazy evaluation so that composition is cheap
and expensive work is deferred to render time.

## Step 3: Bind Attributes with `.attr()`

The `.attr()` method maps a named attribute to a closure that extracts a value
from each data record. The closure's return type must implement `IntoAttrValue`:

```rust
selection
    .attr("center", |d: &SalesRecord| {
        // Map revenue and profit to screen coordinates [-1, 1]
        let x = d.revenue / 500.0 * 2.0 - 1.0;
        let y = d.profit_margin * 2.0 - 1.0;
        [x, y]   // [f32; 2] implements IntoAttrValue
    })
    .attr("radius", |d: &SalesRecord| {
        // Larger circles for higher-revenue products
        0.02 + d.revenue / 5000.0
    })
    .attr("fill_color", |d: &SalesRecord| {
        // Colour by category
        match d.category {
            0 => [0.9, 0.2, 0.2, 0.8],  // Red
            1 => [0.2, 0.8, 0.3, 0.8],  // Green
            _ => [0.2, 0.4, 0.9, 0.8],  // Blue
        }
    });
```

### Supported Return Types

Any type that implements `IntoAttrValue` works as a return type:

| Rust type | `AttrValue` variant | Typical attribute |
|-----------|-------------------|-------------------|
| `f32` | `Float` | `"radius"`, `"stroke_width"` |
| `[f32; 2]` | `Vec2` | `"center"`, `"position"` |
| `[f32; 4]` | `Vec4` | `"fill_color"`, `"stroke_color"` |

## Step 4: Batch Bindings with `attr_parallel()`

When multiple attributes depend on the same computation, use `attr_parallel()`
to compute them in a single pass over your data:

```rust
selection.attr_parallel(
    |d: &SalesRecord| {
        let x = d.revenue / 500.0 * 2.0 - 1.0;
        let y = d.profit_margin * 2.0 - 1.0;
        let position = [x, y];
        let color = [
            1.0 - d.profit_margin,  // More red when less profitable
            d.profit_margin,         // More green when more profitable
            0.3,
            0.7,
        ];
        (position, color)
    },
    ["center", "fill_color"],
);
```

This avoids redundant work when the same raw data feeds multiple attributes.

## Step 5: Update Data and Re-render

Gup supports updating the selection's data without recreating the entire
pipeline. Call `set_data()` with a new `Vec<T>`:

```rust
// New quarter's data arrives
let updated_data = vec![
    SalesRecord { product: "Widget A".into(), revenue: 150.0, profit_margin: 0.40, category: 0 },
    SalesRecord { product: "Widget B".into(), revenue: 260.0, profit_margin: 0.25, category: 0 },
    SalesRecord { product: "Gadget X".into(), revenue: 500.0, profit_margin: 0.52, category: 1 },
    SalesRecord { product: "Gadget Y".into(), revenue: 330.0, profit_margin: 0.58, category: 1 },
    SalesRecord { product: "Gizmo Z".into(),  revenue: 210.0, profit_margin: 0.19, category: 2 },
];

selection.set_data(updated_data);
```

Your attribute bindings are preserved — Gup re-evaluates them against the new
data the next time `prepare_render_bound()` is called. This is the *update*
half of the join/update pattern: existing bindings are replayed on fresh data.

## Step 6: Render

When you are ready to send the selection to the GPU, call `prepare_render_bound`
and then `render`:

```rust
selection.prepare_render_bound(device, queue, None, None)?;
selection.render(&mut render_pass)?;
```

The `prepare_render_bound` call evaluates all `.attr()` bindings, builds GPU
buffers, and compiles the render pipeline. The `render` call draws instanced
geometry in a single draw call.

## Full Example

```rust
use gup::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct SalesRecord {
    product: String,
    revenue: f32,
    profit_margin: f32,
    category: u32,
}

#[tokio::main]
async fn main() -> GupResult<()> {
    let data = vec![
        SalesRecord { product: "Widget A".into(), revenue: 120.0, profit_margin: 0.35, category: 0 },
        SalesRecord { product: "Widget B".into(), revenue: 280.0, profit_margin: 0.22, category: 0 },
        SalesRecord { product: "Gadget X".into(), revenue: 450.0, profit_margin: 0.48, category: 1 },
        SalesRecord { product: "Gadget Y".into(), revenue: 310.0, profit_margin: 0.61, category: 1 },
        SalesRecord { product: "Gizmo Z".into(),  revenue: 190.0, profit_margin: 0.15, category: 2 },
    ];

    let context = Arc::new(RenderContext::new().await?);

    let mut selection = Selection::<SalesRecord, Circle>::from_data(data);
    selection
        .attr("center", |d: &SalesRecord| {
            let x = d.revenue / 500.0 * 2.0 - 1.0;
            let y = d.profit_margin * 2.0 - 1.0;
            [x, y]
        })
        .attr("radius", |d: &SalesRecord| 0.02 + d.revenue / 5000.0)
        .attr("fill_color", |d: &SalesRecord| match d.category {
            0 => [0.9, 0.2, 0.2, 0.8],
            1 => [0.2, 0.8, 0.3, 0.8],
            _ => [0.2, 0.4, 0.9, 0.8],
        });

    println!("Selection bound with {} records", selection.len());
    println!("Ready for GPU rendering!");

    Ok(())
}
```

## Key Concepts

| Concept | What It Does |
|---------|-------------|
| `Selection<T, M>` | Connects data of type `T` to a mark of type `M` |
| `.attr(name, closure)` | Maps a named visual attribute to a data field |
| `.attr_parallel(closure, names)` | Computes multiple attributes in one pass |
| `set_data(new_data)` | Replaces the data, preserving bindings |
| `prepare_render_bound()` | Evaluates bindings and uploads to GPU |
| `render()` | Issues the GPU draw call |

## Next Steps

- **[Tutorial 3: Custom Shader Functions](03_custom_shader_functions.md)** —
  move data transforms to the GPU for maximum performance.
- **[`attr_binding_demo` example](../../examples/attr_binding_demo.rs)** —
  a complete windowed example of attribute binding.
- **[Mark System docs](../mark-system/README.md)** — learn about built-in mark
  types and their attributes.
