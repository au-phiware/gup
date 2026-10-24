# Graphics Programming Patterns

<!--
Gup - GPU-Accelerated Data Visualization Library
Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program. If not, see <https://www.gnu.org/licenses/>.
-->

This document contains GPU and graphics programming patterns extracted from the
codebase conventions. These patterns are specific to GPU/graphics programming
and provide guidance for implementing efficient, maintainable graphics code.

## Core Graphics Patterns

### Viewport State Management

**Learning**: Rendering operations that modify global state (like viewport) must
restore original state.

**Pattern**:

```rust
fn render_with_viewport(&mut self, context: &mut RenderContext) -> GupResult<()> {
    let original_viewport = context.viewport();

    // Modify viewport for this operation
    context.set_viewport(new_viewport)?;

    // Do rendering work
    self.component.render(context)?;

    // Always restore original state
    context.set_viewport(original_viewport)?;

    Ok(())
}
```

**Guidelines**:

- Always capture original state before modifications
- Use RAII patterns where possible (consider viewport guards)
- Document state modifications clearly

### Coordinate System Conventions

**Learning**: Be explicit about coordinate system conventions and document them
clearly.

**Convention**: Grid layouts use `(row, col)` indexing where:

- Rows: 0 to `num_rows - 1` (top to bottom)
- Columns: 0 to `num_cols - 1` (left to right)

**Pattern**:

```rust
pub struct GridPosition {
    pub row: u32, // 0-based row index
    pub col: u32, // 0-based column index
}

// Always validate bounds
fn is_valid_position(&self, pos: GridPosition) -> bool {
    pos.row < self.rows && pos.col < self.cols
}
```

### Viewport Calculation Caching

**Learning**: Repeated viewport calculations can be expensive - consider caching
when viewport doesn't change.

**Future Optimization**:

```rust
struct CachedViewportCalculation {
    original_viewport: Viewport,
    split_viewports: Option<(Viewport, Viewport)>,
    config_hash: u64,
}
```

## GPU Programming Patterns

### Type-Safe Shader Function System

**Learning**: GPU programming benefits from compile-time type validation to
prevent runtime shader errors.

**Pattern**: Use phantom types to preserve type information across GPU
boundaries:

```rust
// ✅ Phantom types preserve T for compile-time validation
pub struct PositionShaderFunction<F, T> {
    extractor: F,
    _phantom: PhantomData<T>,
}

impl<F, T> ShaderFunction for PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static, // 'static required for GPU storage
{
    type Input = T;
    type Output = [f32; 2];
}
```

**Benefits**:

- Compile-time validation of data-to-attribute mappings
- Type safety across CPU-GPU boundary
- Clear error messages for invalid shader function bindings

### Buffer Growth Strategies

**Learning**: GPU buffers should grow efficiently to handle dynamic data sizes
without frequent reallocations.

**Pattern**: Use growth factor of 1.5x for optimal memory vs. performance
trade-off:

```rust
pub fn write(&mut self, device: &Device, queue: &Queue, data: &[T]) -> GupResult<()> {
    if data.len() > self.capacity {
        // 1.5x growth factor balances memory usage and reallocation frequency
        let new_capacity = (data.len() as f64 * 1.5) as usize;
        self.resize_buffer(device, new_capacity)?;
    }
    queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    Ok(())
}
```

**Guidelines**:

- Provide both auto-resize and fixed-size options
- Use exponential growth (1.5x) to minimize allocations
- Always validate buffer capacity before writing
- Return clear error messages on capacity failures

### GPU Resource Context Sharing

**Learning**: WebGPU resources (Device, Queue) must be shared efficiently to
avoid resource conflicts.

**Problem**: Creating multiple contexts causes buffer ownership issues:

```rust
// ❌ Problematic - multiple contexts
let device1 = &RenderContext::new().await.unwrap().device().clone();
let device2 = &RenderContext::new().await.unwrap().device().clone();
// Results in: "Buffer does not exist" errors
```

**Solution**: Single shared context pattern:

```rust
// ✅ Better - single shared context
let context = RenderContext::new().await.unwrap();
let device = context.device();
let queue = context.queue();
```

### Lifetime Management for GPU Traits

**Learning**: GPU-bound traits require `'static` lifetimes for safe storage and
transfer.

**Pattern**: Add `'static` bounds to all GPU-destined types:

```rust
// ✅ Explicit 'static for GPU safety
impl<F, T> ShaderFunction for PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static, // 'static required for GPU storage
```

**Rationale**:

- GPU operations are asynchronous and may outlive stack frames
- Shader functions must be stored for pipeline compilation
- Static lifetimes ensure data remains valid during GPU processing

### Performance Benchmarking for GPU Code

**Learning**: GPU code requires different benchmarking approaches than CPU-only
code.

**Pattern**: Test both small-scale and large-scale performance:

```rust
#[tokio::test]
async fn bench_selection_render_10k_points() {
    let large_data: Vec<Data> = generate_test_data(10_000);
    let mut selection = Selection::new(large_data, context).unwrap();

    let start = Instant::now();
    selection.render().unwrap();
    let duration = start.elapsed();

    // Target: <50ms for 10K points on moderate hardware
    assert!(duration.as_millis() < 50);
}
```

**Guidelines**:

- Test with datasets of 1K, 10K, and 100K+ elements
- Measure both CPU preparation and GPU execution time
- Include buffer allocation overhead in benchmarks
- Set realistic performance targets based on GPU capabilities

### Type Compatibility Systems

**Learning**: Flexible type compatibility checks enable gradual typing while
maintaining safety.

**Pattern**: Use marker traits for opt-in compatibility validation:

```rust
// ✅ Flexible compatibility with reasonable defaults
pub trait Compatible<T> {
    fn is_compatible() -> bool {
        true
    } // Permissive default
}

// Opt-in strict validation for specific types
impl Compatible<f32> for String {
    fn is_compatible() -> bool {
        false
    } // Strict validation
}
```

---

_This document contains GPU and graphics programming patterns extracted from
project conventions. Update it as new patterns are discovered during
development._
