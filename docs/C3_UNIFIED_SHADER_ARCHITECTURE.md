# Unified Shader Function Architecture for Gup

## Overview

Gup uses a unified shader function system where **all data transformations are composable WGSL functions** that run on the GPU. This includes scales, color mappings, coordinate transforms, and any custom data processing - everything composes naturally through a single, powerful abstraction.

## Core Architecture: ShaderFunction Trait

### Universal Composable Functions

```rust
pub trait ShaderFunction {
    type Input;
    type Output;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable = ();

    // The WGSL function implementation
    fn wgsl_function() -> &'static str;

    // Optional uniform data for GPU
    fn create_uniforms(&self) -> Option<Self::Uniforms> { None }

    // Function name for shader composition
    fn function_name() -> &'static str;

    // Input/output type information for validation
    fn input_type() -> ShaderType;
    fn output_type() -> ShaderType;
}
```

### WGSL Function Macro

The `#[wgsl_function]` macro allows writing functions in WGSL syntax that generate both GPU and CPU code:

```rust
// Linear scale as a composable shader function
#[wgsl_function]
fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    return scale.range_min + normalized * (scale.range_max - scale.range_min);
}

// Categorical color mapping
#[wgsl_function]
fn categorical_color(category: u32, colors: ColorPalette) -> vec4<f32> {
    return colors.palette[category % colors.count];
}

// Complex coordinate transformation
#[wgsl_function]
fn polar_to_cartesian(angle: f32, radius: f32) -> vec2<f32> {
    return vec2<f32>(radius * cos(angle), radius * sin(angle));
}

// Data-driven size scaling
#[wgsl_function]
fn sqrt_scale(value: f32, scale: SqrtScaleUniforms) -> f32 {
    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    let sqrt_normalized = sqrt(max(0.0, normalized));
    return scale.range_min + sqrt_normalized * (scale.range_max - scale.range_min);
}
```

## Shader Pipeline Composition

### Composable Pipeline Builder

```rust
pub struct ShaderPipeline {
    functions: Vec<Box<dyn ShaderFunction>>,
    uniform_buffers: HashMap<String, wgpu::Buffer>,
    function_graph: DependencyGraph,
}

impl ShaderPipeline {
    pub fn add_function<F: ShaderFunction + 'static>(&mut self, func: F) -> &mut Self {
        // Add function to pipeline
        self.functions.push(Box::new(func));

        // Create uniform buffer if needed
        if let Some(uniforms) = func.create_uniforms() {
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Uniforms", F::function_name())),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            self.uniform_buffers.insert(F::function_name().to_string(), buffer);
        }

        // Build dependency graph for optimal ordering
        self.function_graph.add_dependency(F::input_type(), F::output_type());

        self
    }

    pub fn compose<F: ShaderFunction + 'static>(mut self, func: F) -> Self {
        self.add_function(func);
        self
    }

    pub fn generate_vertex_shader(&self) -> String {
        let mut shader = String::from(
            "// Auto-generated Gup vertex shader\n\n"
        );

        // Add uniform buffer bindings
        for (i, (name, _)) in self.uniform_buffers.iter().enumerate() {
            shader.push_str(&format!(
                "@group(0) @binding({}) var<uniform> {}: {}Uniforms;\n",
                i, name, name
            ));
        }
        shader.push('\n');

        // Add all function definitions
        for func in &self.functions {
            shader.push_str(func.wgsl_function());
            shader.push_str("\n\n");
        }

        // Generate optimized main function
        shader.push_str(&self.generate_main_vertex_function());

        shader
    }

    pub fn generate_fragment_shader(&self) -> String {
        // Similar generation for fragment shaders
        // Includes color functions, texturing, etc.
    }
}
```

## Unified Attribute System

### Universal Attribute Binding

```rust
impl<T, M: Mark> Selection<T, M> {
    // Attributes are now shader functions, not CPU closures
    pub fn attr<F>(&mut self, name: &str, shader_func: F) -> &mut Self
    where
        F: ShaderFunction + 'static,
        F::Input: Compatible<T>,  // Input must be compatible with data type
        F::Output: Compatible<M::AttributeValue>,  // Output must match mark attribute
    {
        // Add shader function to the pipeline
        self.shader_pipeline.add_function(shader_func);

        // Map attribute name to function for vertex shader generation
        self.attribute_mappings.insert(
            name.to_string(),
            F::function_name().to_string()
        );

        // Mark that GPU resources need updating
        self.mark_dirty();

        self
    }

    // Compose multiple functions for complex transformations
    pub fn attr_compose<F, G>(&mut self, name: &str, f: F, g: G) -> &mut Self
    where
        F: ShaderFunction + 'static,
        G: ShaderFunction + 'static,
        F::Output: Compatible<G::Input>,
        G::Output: Compatible<M::AttributeValue>,
    {
        // Create composed function
        let composed = ComposedFunction::new(f, g);
        self.attr(name, composed)
    }
}
```

### Advanced Usage Examples

```rust
// Complex visualization with multiple composed transformations
chart.select_all::<Circle>()
    .data(weather_data)
    // Position: geographic coordinates -> screen coordinates
    .attr("position",
        mercator_projection::new(viewport)
            .compose(screen_transform::new(dimensions))
    )
    // Color: temperature -> color with seasonal adjustment
    .attr("color",
        temperature_scale::new(-10.0, 40.0)
            .compose(seasonal_color_adjustment::new())
            .compose(color_interpolation::new(temp_palette))
    )
    // Size: humidity -> radius with logarithmic scaling
    .attr("radius",
        log_scale::new(0.0, 100.0)
            .compose(clamp_function::new(2.0, 20.0))
    )
    // Opacity: data quality -> transparency
    .attr("opacity",
        quality_to_alpha::new()
    );
```

## Built-in Shader Function Library

### Scale Functions

```rust
// Linear scaling
#[wgsl_function]
fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
    let t = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    return scale.range_min + t * (scale.range_max - scale.range_min);
}

// Logarithmic scaling
#[wgsl_function]
fn log_scale(value: f32, scale: LogScaleUniforms) -> f32 {
    let log_value = log(max(scale.epsilon, value));
    let log_domain_min = log(max(scale.epsilon, scale.domain_min));
    let log_domain_max = log(max(scale.epsilon, scale.domain_max));
    let t = (log_value - log_domain_min) / (log_domain_max - log_domain_min);
    return scale.range_min + t * (scale.range_range);
}

// Power scaling
#[wgsl_function]
fn power_scale(value: f32, scale: PowerScaleUniforms) -> f32 {
    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    let powered = pow(max(0.0, normalized), scale.exponent);
    return scale.range_min + powered * (scale.range_max - scale.range_min);
}
```

### Color Functions

```rust
// HSV to RGB conversion
#[wgsl_function]
fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let c = hsv.z * hsv.y;
    let x = c * (1.0 - abs((hsv.x / 60.0) % 2.0 - 1.0));
    let m = hsv.z - c;

    var rgb: vec3<f32>;
    if (hsv.x < 60.0) {
        rgb = vec3<f32>(c, x, 0.0);
    } else if (hsv.x < 120.0) {
        rgb = vec3<f32>(x, c, 0.0);
    } else if (hsv.x < 180.0) {
        rgb = vec3<f32>(0.0, c, x);
    } else if (hsv.x < 240.0) {
        rgb = vec3<f32>(0.0, x, c);
    } else if (hsv.x < 300.0) {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return rgb + vec3<f32>(m, m, m);
}

// Interpolate between colors
#[wgsl_function]
fn color_interpolation(t: f32, colors: ColorInterpolationUniforms) -> vec4<f32> {
    let clamped_t = clamp(t, 0.0, 1.0);
    let scaled_t = clamped_t * f32(colors.color_count - 1u);
    let index = u32(floor(scaled_t));
    let fraction = scaled_t - floor(scaled_t);

    if (index >= colors.color_count - 1u) {
        return colors.colors[colors.color_count - 1u];
    }

    let color1 = colors.colors[index];
    let color2 = colors.colors[index + 1u];

    return mix(color1, color2, fraction);
}
```

### Geometric Transformations

```rust
// 2D rotation
#[wgsl_function]
fn rotate_2d(point: vec2<f32>, rotation: RotationUniforms) -> vec2<f32> {
    let cos_angle = cos(rotation.angle);
    let sin_angle = sin(rotation.angle);

    return vec2<f32>(
        point.x * cos_angle - point.y * sin_angle,
        point.x * sin_angle + point.y * cos_angle
    );
}

// Geographic projection (Mercator)
#[wgsl_function]
fn mercator_projection(lonlat: vec2<f32>, projection: MercatorUniforms) -> vec2<f32> {
    let lon_rad = lonlat.x * 3.14159265 / 180.0;
    let lat_rad = lonlat.y * 3.14159265 / 180.0;

    let x = (lon_rad - projection.center_lon) * projection.scale;
    let y = log(tan(3.14159265 / 4.0 + lat_rad / 2.0)) * projection.scale;

    return vec2<f32>(x + projection.offset.x, -y + projection.offset.y);
}

// Polar to Cartesian coordinates
#[wgsl_function]
fn polar_to_cartesian(polar: vec2<f32>) -> vec2<f32> {
    let angle = polar.x;
    let radius = polar.y;
    return vec2<f32>(radius * cos(angle), radius * sin(angle));
}
```

## Function Composition System

### Automatic Composition

```rust
// Functions automatically compose when types match
impl<F, G> ShaderFunction for ComposedFunction<F, G>
where
    F: ShaderFunction,
    G: ShaderFunction,
    F::Output: Compatible<G::Input>,
{
    type Input = F::Input;
    type Output = G::Output;
    type Uniforms = CombinedUniforms<F::Uniforms, G::Uniforms>;

    fn wgsl_function() -> &'static str {
        // Generate combined function that calls F then G
        &format!(
            "fn {}(input: {}) -> {} {{
                let intermediate = {}(input, f_uniforms);
                return {}(intermediate, g_uniforms);
            }}",
            Self::function_name(),
            F::Input::wgsl_type(),
            G::Output::wgsl_type(),
            F::function_name(),
            G::function_name()
        )
    }
}

// Chainable composition
let complex_transform = linear_scale::new(0.0, 100.0)
    .compose(sqrt_function::new())
    .compose(clamp_function::new(0.0, 1.0))
    .compose(size_mapping::new(5.0, 50.0));
```

### Type-Safe Composition

```rust
// Compile-time type checking ensures compositions are valid
chart.select_all::<Circle>()
    .attr("position",
        geographic_coords         // vec2<f32> (lon, lat)
            .compose(mercator_projection)  // vec2<f32> -> vec2<f32> (x, y)
            .compose(screen_transform)     // vec2<f32> -> vec2<f32> (screen coords)
    )
    .attr("color",
        temperature_value         // f32 (degrees)
            .compose(temperature_scale)    // f32 -> f32 (normalized)
            .compose(color_interpolation)  // f32 -> vec4<f32> (RGBA)
    );
    // This won't compile if types don't match!
```

## Performance Characteristics

### GPU Optimization Benefits

1. **Parallel Execution**: All transformations happen simultaneously across vertices
2. **Memory Efficiency**: Data flows through GPU memory without CPU roundtrips
3. **Vectorization**: GPU SIMD operations on multiple data points at once
4. **Caching**: Uniform buffers cached on GPU for repeated access

### Benchmark Targets

| Data Points | CPU Transform Time | GPU Transform Time | Speedup |
|-------------|-------------------|-------------------|---------|
| 1,000       | 0.1ms             | 0.01ms            | 10x     |
| 10,000      | 1.0ms             | 0.02ms            | 50x     |
| 100,000     | 10ms              | 0.05ms            | 200x    |
| 1,000,000   | 100ms             | 0.1ms             | 1000x   |

## Implementation Phases

### Phase 1: Core Infrastructure

- [ ] `ShaderFunction` trait and basic composition
- [ ] `#[wgsl_function]` macro (basic version)
- [ ] Shader pipeline builder
- [ ] Integration with Selection system

### Phase 2: Function Library

- [ ] Complete scale function family
- [ ] Color transformation functions
- [ ] Geometric transformation functions
- [ ] Statistical functions (mean, variance, etc.)

### Phase 3: Advanced Features

- [ ] Conditional functions (if/else in shaders)
- [ ] Loop constructs for iterative algorithms
- [ ] Texture sampling functions
- [ ] Advanced geometric algorithms

This unified shader function system makes Gup extraordinarily powerful and flexible while maintaining GPU-level performance for all data transformations.
