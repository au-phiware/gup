# Type Construction Guide

## Overview

Gup provides ergonomic macro-based constructors for GPU-compatible vector and
matrix types. These macros ensure proper memory layout for GPU operations while
providing a clean, concise syntax.

## Quick Start

```rust
use gup::*;

// Create vectors with macros
let position_2d = vec2![1.0, 2.0];
let position_3d = vec3![1.0, 2.0, 3.0];
let color = vec4![1.0, 0.5, 0.0, 1.0];

// Create matrices with macros
let identity_2x2 = mat2![
    1.0, 0.0,
    0.0, 1.0
];

let identity_4x4 = mat4![
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
];
```

## Available Macros

### Vector Macros

#### `vec2![x, y]`

Creates a 2D vector with proper GPU alignment.

```rust
let position = vec2![10.0, 20.0];
assert_eq!(position.x, 10.0);
assert_eq!(position.y, 20.0);
```

**Memory Layout**: 8 bytes (4 bytes per component)

#### `vec3![x, y, z]`

Creates a 3D vector with automatic GPU padding for 16-byte alignment.

```rust
let position = vec3![1.0, 2.0, 3.0];
assert_eq!(position.x, 1.0);
assert_eq!(position.y, 2.0);
assert_eq!(position.z, 3.0);
// Note: _padding field is automatically set to 0.0
```

**Memory Layout**: 16 bytes (12 bytes of data + 4 bytes padding)

**Important**: The padding is automatically handled by the macro. You never need
to specify or worry about it.

#### `vec4![x, y, z, w]`

Creates a 4D vector commonly used for colors with alpha channel or homogeneous
coordinates.

```rust
let color = vec4![1.0, 0.5, 0.0, 1.0]; // Orange with full opacity
let position_homogeneous = vec4![10.0, 20.0, 0.0, 1.0];
```

**Memory Layout**: 16 bytes (4 bytes per component)

### Matrix Macros

All matrix macros use **row-major** order for natural reading.

#### `mat2![m00, m01, m10, m11]`

Creates a 2x2 matrix (4 components).

```rust
let rotation_90 = mat2![
    0.0, -1.0,
    1.0,  0.0
];
```

**Memory Layout**: 16 bytes (8 bytes data + 8 bytes padding)

#### `mat3![m00, m01, m02, m10, m11, m12, m20, m21, m22]`

Creates a 3x3 matrix (9 components).

```rust
let transform = mat3![
    1.0, 0.0, 10.0,  // Scale X, Shear XY, Translate X
    0.0, 1.0, 20.0,  // Shear YX, Scale Y, Translate Y
    0.0, 0.0,  1.0   // Perspective
];
```

**Memory Layout**: 48 bytes with GPU-standard padding between rows

#### `mat4![...]`

Creates a 4x4 matrix (16 components).

```rust
let projection = mat4![
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
];
```

**Memory Layout**: 64 bytes with proper GPU alignment

## Why Use Macros?

### Ergonomics

Macro syntax is significantly cleaner than constructor calls:

```rust
// ❌ Old constructor syntax (verbose)
let v = Vec3::new(1.0, 2.0, 3.0);
let m = Mat4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
);

// ✅ New macro syntax (clean)
let v = vec3![1.0, 2.0, 3.0];
let m = mat4![
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
];
```

### GPU Compatibility

The macros automatically handle GPU-specific requirements:

- **Alignment**: Ensures proper memory alignment for GPU buffers
- **Padding**: Adds necessary padding bytes (e.g., Vec3 needs 4 bytes of
  padding)
- **Layout**: Follows GPU memory layout standards (std140/std430)

### Performance

- **Zero Runtime Cost**: Macros expand at compile time
- **Compile-Time Validation**: Type errors caught during compilation
- **Inlining**: Macro expansions are always inlined

## Migration Guide

If you have existing code using constructors, migration is straightforward:

### Find and Replace Patterns

```rust
// Vec2::new(x, y) → vec2![x, y]
- let pos = Vec2::new(10.0, 20.0);
+ let pos = vec2![10.0, 20.0];

// Vec3::new(x, y, z) → vec3![x, y, z]
- let pos = Vec3::new(1.0, 2.0, 3.0);
+ let pos = vec3![1.0, 2.0, 3.0];

// Vec4::new(x, y, z, w) → vec4![x, y, z, w]
- let color = Vec4::new(1.0, 0.5, 0.0, 1.0);
+ let color = vec4![1.0, 0.5, 0.0, 1.0];
```

### Matrix Migration

For matrices with many parameters, the macro syntax is especially beneficial:

```rust
// Mat4::new(...16 parameters...) → mat4![...]
- let m = Mat4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
+ let m = mat4![
+     1.0, 0.0, 0.0, 0.0,
+     0.0, 1.0, 0.0, 0.0,
+     0.0, 0.0, 1.0, 0.0,
+     0.0, 0.0, 0.0, 1.0
+ ];
```

## Import Requirements

All macros are exported at the crate root and available via:

```rust
use gup::*;  // Includes vec2!, vec3!, vec4!, mat2!, mat3!, mat4!

// Or import specific items
use gup::{vec2, vec3, vec4};
use gup::{mat2, mat3, mat4};
```

### Prelude

The `prelude` module includes all commonly used types and macros:

```rust
use gup::prelude::*;

// All vector and matrix macros are now available
let position = vec3![0.0, 1.0, 0.0];
```

## Common Patterns

### Data Transformation Pipelines

```rust
use gup::*;

// Transform data points to screen coordinates
let data_points = vec![
    vec2![10.0, 20.0],
    vec2![30.0, 40.0],
    vec2![50.0, 60.0],
];

// Create transformation matrix
let scale = mat3![
    0.1, 0.0, 0.0,
    0.0, 0.1, 0.0,
    0.0, 0.0, 1.0
];
```

### Color Gradients

```rust
use gup::*;

let gradient_stops = vec![
    vec4![1.0, 0.0, 0.0, 1.0],  // Red
    vec4![1.0, 1.0, 0.0, 1.0],  // Yellow
    vec4![0.0, 1.0, 0.0, 1.0],  // Green
];
```

### GPU Buffer Creation

```rust
use gup::*;

// Prepare vertex data for GPU
let vertices = vec![
    vec3![0.0, 0.5, 0.0],   // Top
    vec3![-0.5, -0.5, 0.0], // Bottom left
    vec3![0.5, -0.5, 0.0],  // Bottom right
];
```

## Troubleshooting

### Import Errors

**Problem**: "cannot find macro `vec3` in this scope"

**Solution**: Add `use gup::*;` or `use gup::vec3;` at the top of your file.

### Type Mismatches

**Problem**: "expected `Vec3`, found a different type"

**Solution**: Ensure you're using the correct macro (`vec3!` for `Vec3`, etc.)

### Clippy Warnings

**Problem**: Clippy suggests using `::new()` constructors

**Solution**: Macros are the preferred approach. The constructors still exist
for compatibility but macros provide better ergonomics.

## Performance Notes

### Compile-Time Benefits

- Macro expansion happens during compilation
- No runtime overhead compared to constructors
- Full compiler optimizations apply to expanded code

### Runtime Performance

- Identical to hand-written struct initialization
- Zero-cost abstraction
- Memory layout optimized for GPU access patterns

### Benchmark Results

The macro approach has been validated to have:

- **Zero overhead** compared to direct struct construction
- **Better code generation** in some cases due to simpler expansion
- **Identical GPU performance** with proper alignment guarantees

## Advanced Usage

### Generic Functions

Macros work seamlessly with generic code:

```rust
fn transform<T>(input: T) -> Vec3
where
    T: Into<Vec3>,
{
    input.into()
}

let result = transform(vec3![1.0, 2.0, 3.0]);
```

### Const Evaluation

Macros can be used in const contexts:

```rust
const ORIGIN: Vec2 = vec2![0.0, 0.0];
const IDENTITY_2X2: Mat2 = mat2![
    1.0, 0.0,
    0.0, 1.0
];
```

## Best Practices

1. **Always use macros for new code**: Provides consistency and ergonomics
2. **Format matrices on multiple lines**: Makes transformation matrices readable
3. **Use descriptive variable names**: `position`, `color`, `transform` are
   clearer than `v`, `c`, `m`
4. **Group related vectors**: Keep coordinate data together for better cache
   locality

## See Also

- [Shader Function Guide](./TECHNICAL_APPROACH.md) - Using types in shader
  functions
- [Custom Mark Guide](./CUSTOM_MARK_GUIDE.md) - Creating custom visualization
  marks
- [API Documentation](https://docs.rs/gup) - Full API reference
