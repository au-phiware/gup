# Rust Type System Integration: Making Shader Functions Type-Safe

## The Core Challenge

When you write:

```rust
chart.select_all::<Circle>()
    .data(weather_data)  // weather_data: Vec<WeatherData>
    .attr("position", position_transform)
```

The type system must ensure:

1. `position_transform` can process `WeatherData` input
2. `position_transform` output is compatible with `Circle`'s position attribute
3. The generated vertex shader correctly maps data → shader function → vertex buffer
4. All of this is validated at compile time, not runtime

## Architectural Solution: Associated Types and Generic Constraints

### Mark-Specific Attribute Types

```rust
pub trait Mark: Clone + Send + Sync + 'static {
    type Vertex: bytemunk::Pod + bytemunk::Zeroable;

    // Associated types for each attribute a mark supports
    type PositionAttribute: ShaderType;
    type ColorAttribute: ShaderType;
    type SizeAttribute: ShaderType;
    // ... other attributes

    // Vertex creation from processed attributes
    fn create_vertex(
        position: Self::PositionAttribute,
        color: Self::ColorAttribute,
        size: Self::SizeAttribute,
        // ... other attributes
    ) -> Self::Vertex;

    // Attribute name to type mapping for validation
    fn attribute_type(name: &str) -> Option<TypeId>;
}

// Circle defines what types it expects for each attribute
impl Mark for Circle {
    type Vertex = CircleVertex;
    type PositionAttribute = [f32; 2];  // 2D position
    type ColorAttribute = [f32; 4];     // RGBA color
    type SizeAttribute = f32;           // Radius

    fn create_vertex(
        position: [f32; 2],
        color: [f32; 4],
        size: f32,
    ) -> CircleVertex {
        CircleVertex { position, color, radius: size }
    }

    fn attribute_type(name: &str) -> Option<TypeId> {
        match name {
            "position" => Some(TypeId::of::<[f32; 2]>()),
            "color" => Some(TypeId::of::<[f32; 4]>()),
            "size" => Some(TypeId::of::<f32>()),
            _ => None,
        }
    }
}
```

### Generic Shader Functions with Type Constraints

```rust
pub trait ShaderFunction {
    type Input: ShaderType;
    type Output: ShaderType;
    type Uniforms: bytemunk::Pod + bytemunk::Zeroable = ();

    fn wgsl_function() -> &'static str;
    fn create_uniforms(&self) -> Option<Self::Uniforms> { None }
    fn function_name() -> &'static str;
}

// Generic shader function that works with any data type that has x,y fields
pub struct PositionTransform<T> {
    x_scale: LinearScale,
    y_scale: LinearScale,
    _phantom: PhantomData<T>,
}

impl<T> ShaderFunction for PositionTransform<T>
where
    T: HasFields<x = f32, y = f32>,  // Type-level field access
{
    type Input = T;
    type Output = [f32; 2];
    type Uniforms = PositionTransformUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn position_transform(data: DataVertex, uniforms: PositionTransformUniforms) -> vec2<f32> {
            let x_scaled = linear_scale(data.x, uniforms.x_scale);
            let y_scaled = linear_scale(data.y, uniforms.y_scale);
            return vec2<f32>(x_scaled, y_scaled);
        }
        "#
    }

    fn function_name() -> &'static str { "position_transform" }
}

// Usage with compile-time validation
let position_transform = PositionTransform::<WeatherData>::new(x_scale, y_scale);
// ✅ This compiles because WeatherData has x,y fields

let invalid_transform = PositionTransform::<String>::new(x_scale, y_scale);
// ❌ This fails to compile because String doesn't have x,y fields
```

### Type-Safe Selection with Attribute Validation

```rust
impl<T, M: Mark> Selection<T, M> {
    // Type-safe attribute binding with compile-time validation
    pub fn attr<F, A>(&mut self, name: &str, shader_func: F) -> &mut Self
    where
        F: ShaderFunction<Input = T, Output = A> + 'static,
        A: Compatible<M>,  // A must be compatible with mark M's expected attribute type
        M: HasAttribute<A>,  // Mark M must support attribute type A
    {
        // Compile-time validation that attribute exists and types match
        let expected_type = M::attribute_type(name)
            .ok_or_else(|| format!("Mark {} doesn't support attribute '{}'",
                std::any::type_name::<M>(), name))?;

        assert_eq!(expected_type, TypeId::of::<A>(),
            "Attribute '{}' expects type {:?}, got {:?}",
            name, expected_type, TypeId::of::<A>());

        self.shader_pipeline.add_function(shader_func);
        self.attribute_mappings.insert(name.to_string(), F::function_name().to_string());
        self
    }
}

// Example that compiles successfully
chart.select_all::<Circle>()
    .data(weather_data)  // T = WeatherData
    .attr("position", position_transform)  // F::Input = WeatherData, F::Output = [f32; 2]
    // ✅ Compiles because Circle::PositionAttribute = [f32; 2]

    .attr("color", temperature_to_color)   // F::Output = [f32; 4]
    // ✅ Compiles because Circle::ColorAttribute = [f32; 4]

    .attr("size", humidity_to_radius);     // F::Output = f32
    // ✅ Compiles because Circle::SizeAttribute = f32

// Example that fails to compile
chart.select_all::<Circle>()
    .data(weather_data)
    .attr("position", temperature_to_color);  // F::Output = [f32; 4]
    // ❌ Compile error: expected [f32; 2] for position, got [f32; 4]
```

### Structured Data Access in WGSL

```rust
// Data types generate WGSL struct definitions
#[derive(ShaderType)]
pub struct WeatherData {
    pub longitude: f32,
    pub latitude: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
}

// The derive macro generates:
impl ShaderType for WeatherData {
    fn wgsl_type_definition() -> &'static str {
        r#"
        struct WeatherData {
            longitude: f32,
            latitude: f32,
            temperature: f32,
            humidity: f32,
            pressure: f32,
        }
        "#
    }

    fn wgsl_size() -> u32 { 20 }  // 5 * 4 bytes
    fn wgsl_alignment() -> u32 { 4 }
}

// Shader functions reference the correct struct fields
#[wgsl_function]
fn weather_position_transform(data: WeatherData, uniforms: ProjectionUniforms) -> vec2<f32> {
    // WGSL knows about WeatherData structure
    return mercator_projection(vec2<f32>(data.longitude, data.latitude), uniforms);
}
```

### Advanced Type-Level Validation

```rust
// Trait for types that have specific fields
pub trait HasField<const FIELD: &'static str, T> {
    fn get_field(&self) -> T;
}

// Derive macro implementation
#[derive(HasFields)]
pub struct WeatherData {
    pub longitude: f32,  // Generates HasField<"longitude", f32>
    pub latitude: f32,   // Generates HasField<"latitude", f32>
    pub temperature: f32, // Generates HasField<"temperature", f32>
    // ...
}

// Generic shader functions can specify field requirements
pub struct GeographicTransform<T> {
    projection: Projection,
    _phantom: PhantomData<T>,
}

impl<T> ShaderFunction for GeographicTransform<T>
where
    T: HasField<"longitude", f32> + HasField<"latitude", f32>,
{
    type Input = T;
    type Output = [f32; 2];

    fn wgsl_function() -> &'static str {
        r#"
        fn geographic_transform(data: DataVertex, projection: ProjectionUniforms) -> vec2<f32> {
            let coords = vec2<f32>(data.longitude, data.latitude);
            return mercator_projection(coords, projection);
        }
        "#
    }
}

// Usage with automatic field validation
let geo_transform = GeographicTransform::<WeatherData>::new(projection);
// ✅ Compiles because WeatherData has longitude and latitude fields

let invalid_geo = GeographicTransform::<PersonData>::new(projection);
// ❌ Compile error: PersonData doesn't have longitude/latitude fields
```

### Vertex Buffer Generation with Type Safety

```rust
pub struct VertexGenerator<T, M: Mark> {
    data_type: PhantomData<T>,
    mark_type: PhantomData<M>,
    attribute_processors: HashMap<String, Box<dyn AttributeProcessor<T>>>,
}

impl<T, M: Mark> VertexGenerator<T, M> {
    pub fn generate_vertices(&self, data: &[T]) -> Vec<M::Vertex> {
        data.iter().map(|datum| {
            // Apply all registered attribute processors
            let position = self.process_attribute::<M::PositionAttribute>("position", datum);
            let color = self.process_attribute::<M::ColorAttribute>("color", datum);
            let size = self.process_attribute::<M::SizeAttribute>("size", datum);

            // Create vertex using mark's vertex creation function
            M::create_vertex(position, color, size)
        }).collect()
    }

    fn process_attribute<A>(&self, name: &str, datum: &T) -> A
    where A: Clone + Default
    {
        self.attribute_processors
            .get(name)
            .map(|processor| processor.process(datum))
            .unwrap_or_default()
    }
}
```

### WGSL Generation with Type Information

```rust
impl ShaderPipeline {
    pub fn generate_vertex_shader<T: ShaderType, M: Mark>(&self) -> String {
        let mut shader = String::new();

        // Add data type definition
        shader.push_str(&format!("// Data type definition\n{}\n",
            T::wgsl_type_definition()));

        // Add vertex type definition
        shader.push_str(&format!("// Vertex type definition\n{}\n",
            M::Vertex::wgsl_type_definition()));

        // Add uniform definitions for each shader function
        for (i, func) in self.functions.iter().enumerate() {
            if func.has_uniforms() {
                shader.push_str(&format!(
                    "@group(0) @binding({}) var<uniform> {}_uniforms: {}Uniforms;\n",
                    i, func.function_name(), func.function_name()
                ));
            }
        }

        // Add all shader function definitions
        for func in &self.functions {
            shader.push_str(func.wgsl_function());
            shader.push_str("\n\n");
        }

        // Generate main vertex function that applies all transformations
        shader.push_str(&self.generate_main_vertex_function::<T, M>());

        shader
    }

    fn generate_main_vertex_function<T: ShaderType, M: Mark>(&self) -> String {
        format!(r#"
        @vertex
        fn vs_main(@location(0) data_index: u32) -> @builtin(position) vec4<f32> {{
            // Load data from storage buffer
            let data = data_buffer[data_index];

            // Apply position transformation
            let position = {}(data, {}_uniforms);

            // Apply other transformations...
            let color = {}(data, {}_uniforms);
            let size = {}(data, {}_uniforms);

            // Create vertex and return position
            let vertex = create_vertex(position, color, size);
            return vec4<f32>(vertex.position, 0.0, 1.0);
        }}
        "#,
        self.get_function_name("position"),
        self.get_function_name("position"),
        self.get_function_name("color"),
        self.get_function_name("color"),
        self.get_function_name("size"),
        self.get_function_name("size")
        )
    }
}
```

## Complete Example: Type-Safe Weather Visualization

```rust
// Define data structure with shader type derivation
#[derive(ShaderType, HasFields)]
pub struct WeatherData {
    pub longitude: f32,
    pub latitude: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub wind_speed: f32,
}

// Create type-safe shader functions
let position_transform = GeographicTransform::<WeatherData>::new(
    mercator_projection
);

let color_transform = FieldBasedTransform::<WeatherData, "temperature", f32>::new(
    temperature_to_color_scale
);

let size_transform = FieldBasedTransform::<WeatherData, "wind_speed", f32>::new(
    wind_speed_to_radius_scale
);

// Use with compile-time validation
let weather_chart = chart
    .select_all::<Circle>()  // M = Circle
    .data(weather_data)      // T = WeatherData
    .attr("position", position_transform)  // ✅ GeographicTransform<WeatherData> → [f32; 2]
    .attr("color", color_transform)        // ✅ FieldTransform<WeatherData, f32> → [f32; 4]
    .attr("size", size_transform);         // ✅ FieldTransform<WeatherData, f32> → f32

// This would fail at compile time:
// .attr("position", color_transform)     // ❌ Wrong output type
// .attr("nonexistent", size_transform)   // ❌ Circle doesn't have "nonexistent" attribute
```

## Benefits of This Type System Integration

### 1. Compile-Time Safety

- Invalid attribute mappings caught at compile time
- Type mismatches between shader functions and marks prevented
- Field access validation for data structures

### 2. IDE Support

- Full IntelliSense/autocomplete for available attributes
- Type information shown for shader function inputs/outputs
- Refactoring support when changing data structures

### 3. Performance

- No runtime type checking overhead
- Optimal vertex buffer layouts generated automatically
- WGSL shader code specialized for exact data types

### 4. Developer Experience

- Clear error messages when types don't match
- Natural Rust syntax for complex type relationships
- Gradual typing - start simple, add constraints as needed

This sophisticated type system integration ensures that Gup's shader function
system is both incredibly powerful and completely safe, leveraging Rust's type
system to catch errors at compile time rather than runtime.
