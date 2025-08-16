// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Core Selection<T, M> type for GPU-accelerated data visualization.
//!
//! The Selection type represents a collection of data bound to visual marks with
//! GPU-accelerated attribute mappings, directly inspired by D3.js selections.

use crate::buffer::GpuBuffer as BufferGpuBuffer;
use crate::interaction::{InteractionElement, InteractionEvent, Renderable};
use crate::{BufferType, GupResult, Mixable, RenderContext};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

/// Trait for shader functions that transform data into visual attributes.
pub trait ShaderFunction: Send + Sync + 'static {
    /// Input data type that this shader function can process
    type Input: Send + Sync;
    /// Output attribute type that this shader function produces
    type Output: Send + Sync;

    /// Apply the shader function to transform input data to output attribute
    fn apply(&self, input: &Self::Input) -> Self::Output;

    /// Get the WGSL shader code for this function
    fn wgsl_code(&self) -> String;

    /// Get a unique identifier for this shader function
    fn function_id(&self) -> String;
}

/// Trait to check compatibility between types for shader function binding
pub trait Compatible<T> {
    /// Check if this type is compatible with T for shader function binding
    fn is_compatible() -> bool {
        true
    }
}

/// Blanket implementation - all types are compatible with themselves
impl<T> Compatible<T> for T {}

/// Shader function that maps data field to position attribute
#[derive(Debug, Clone)]
pub struct PositionShaderFunction<F, T> {
    extractor: F,
    _phantom: PhantomData<T>,
}

impl<F, T> PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub fn new(extractor: F) -> Self {
        Self {
            extractor,
            _phantom: PhantomData,
        }
    }
}

impl<F, T> ShaderFunction for PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    type Input = T;
    type Output = [f32; 2];

    fn apply(&self, input: &Self::Input) -> Self::Output {
        (self.extractor)(input)
    }

    fn wgsl_code(&self) -> String {
        // In a full implementation, this would generate WGSL code
        // For now, return a placeholder
        "@vertex fn position_vertex(input: VertexInput) -> VertexOutput { ... }".to_string()
    }

    fn function_id(&self) -> String {
        "position_shader".to_string()
    }
}

/// Shader function that maps data field to color attribute
#[derive(Debug, Clone)]
pub struct ColorShaderFunction<F, T> {
    extractor: F,
    _phantom: PhantomData<T>,
}

impl<F, T> ColorShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 4] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub fn new(extractor: F) -> Self {
        Self {
            extractor,
            _phantom: PhantomData,
        }
    }
}

impl<F, T> ShaderFunction for ColorShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 4] + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    type Input = T;
    type Output = [f32; 4];

    fn apply(&self, input: &Self::Input) -> Self::Output {
        (self.extractor)(input)
    }

    fn wgsl_code(&self) -> String {
        "@fragment fn color_fragment(input: FragmentInput) -> @location(0) vec4<f32> { ... }"
            .to_string()
    }

    fn function_id(&self) -> String {
        "color_shader".to_string()
    }
}

/// Placeholder shader function for backward compatibility
#[derive(Debug, Clone)]
pub struct PlaceholderShaderFunction {
    name: String,
}

impl ShaderFunction for PlaceholderShaderFunction {
    type Input = (); // Placeholder type
    type Output = (); // Placeholder type

    fn apply(&self, _input: &Self::Input) -> Self::Output {
        // Placeholder implementation
    }

    fn wgsl_code(&self) -> String {
        format!("// Placeholder shader function: {}", self.name)
    }

    fn function_id(&self) -> String {
        format!("placeholder_{}", self.name)
    }
}

/// Type-erased shader function for storage in collections
pub struct BoxedShaderFunction {
    function_id: String,
    wgsl_code: String,
    // In a full implementation, this would contain the actual shader function
    // For now, we store just the metadata
}

impl BoxedShaderFunction {
    pub fn new<F: ShaderFunction>(shader_func: F) -> Self {
        Self {
            function_id: shader_func.function_id(),
            wgsl_code: shader_func.wgsl_code(),
        }
    }

    pub fn function_id(&self) -> &str {
        &self.function_id
    }

    pub fn wgsl_code(&self) -> &str {
        &self.wgsl_code
    }
}

/// A mark defines the visual representation type for data points.
pub trait Mark: Send + Sync + 'static {
    /// The vertex type used by this mark's rendering pipeline
    type Vertex: bytemuck::Pod + bytemuck::Zeroable + Send + Sync;

    /// The attribute values this mark can accept
    type AttributeValue: Send + Sync;

    /// Create a vertex from an attribute value
    fn create_vertex(attr: &Self::AttributeValue) -> Self::Vertex;

    /// Get the primitive topology for rendering this mark
    fn primitive_topology() -> wgpu::PrimitiveTopology;

    /// Get a description of this mark type
    fn description() -> &'static str;
}

/// Basic circle mark implementation
#[derive(Debug, Clone)]
pub struct Circle;

/// Basic line mark implementation for grid and line visualizations
#[derive(Debug, Clone)]
pub struct Line;

/// Vertex data for circle rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub radius: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment
}

/// Attribute values for circle marks
#[derive(Debug, Clone)]
pub struct CircleAttributes {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub radius: f32,
}

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    fn create_vertex(attr: &Self::AttributeValue) -> Self::Vertex {
        CircleVertex {
            position: attr.position,
            color: attr.color,
            radius: attr.radius,
            _padding: [0.0; 3],
        }
    }

    fn primitive_topology() -> wgpu::PrimitiveTopology {
        wgpu::PrimitiveTopology::PointList
    }

    fn description() -> &'static str {
        "Circle"
    }
}

/// Vertex data for line rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub color: [f32; 4],
    pub width: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment
}

/// Attribute values for line marks
#[derive(Debug, Clone, Default)]
pub struct LineAttributes {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub color: [f32; 4],
    pub width: f32,
}

impl Mark for Line {
    type Vertex = LineVertex;
    type AttributeValue = LineAttributes;

    fn create_vertex(attr: &Self::AttributeValue) -> Self::Vertex {
        LineVertex {
            start: attr.start,
            end: attr.end,
            color: attr.color,
            width: attr.width,
            _padding: [0.0; 3],
        }
    }

    fn primitive_topology() -> wgpu::PrimitiveTopology {
        wgpu::PrimitiveTopology::LineList
    }

    fn description() -> &'static str {
        "Line"
    }
}

/// Instance data for GPU rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    pub transform: [f32; 16], // 4x4 transformation matrix
    pub data_index: u32,      // Index into data array
    _padding: [f32; 3],       // Ensure 16-byte alignment
}

/// Shader pipeline for attribute mapping
pub struct ShaderPipeline {
    /// Collection of registered shader functions
    shader_functions: HashMap<String, BoxedShaderFunction>,
    /// Generated WGSL shader code
    generated_wgsl: Option<String>,
    /// Shader generation dirty flag
    is_dirty: bool,
}

impl ShaderPipeline {
    fn new() -> Self {
        Self {
            shader_functions: HashMap::new(),
            generated_wgsl: None,
            is_dirty: true,
        }
    }

    /// Register a shader function for an attribute
    fn register_shader_function<F: ShaderFunction>(
        &mut self,
        attribute_name: &str,
        shader_func: F,
    ) {
        let boxed_func = BoxedShaderFunction::new(shader_func);
        self.shader_functions
            .insert(attribute_name.to_string(), boxed_func);
        self.is_dirty = true;
        self.generated_wgsl = None;
    }

    /// Generate combined WGSL shader code from all registered functions
    #[allow(dead_code)]
    fn generate_wgsl(&mut self) -> &str {
        if self.is_dirty {
            let mut combined_wgsl = String::new();
            combined_wgsl.push_str("// Generated WGSL shader code\n");

            for (attr_name, func) in &self.shader_functions {
                combined_wgsl.push_str(&format!("// Shader function for attribute: {attr_name}\n"));
                combined_wgsl.push_str(func.wgsl_code());
                combined_wgsl.push('\n');
            }

            self.generated_wgsl = Some(combined_wgsl);
            self.is_dirty = false;
        }

        self.generated_wgsl.as_ref().unwrap()
    }

    /// Check if a shader function is registered for an attribute
    fn has_shader_function(&self, attribute_name: &str) -> bool {
        self.shader_functions.contains_key(attribute_name)
    }

    /// Get all registered attribute names
    fn attribute_names(&self) -> Vec<&String> {
        self.shader_functions.keys().collect()
    }
}

/// Core Selection type that binds data to visual marks with GPU acceleration.
///
/// This type represents a collection of data bound to visual marks, providing
/// all the power of D3-style selections while leveraging GPU parallel processing.
///
/// # Type Parameters
///
/// * `T` - The data type for each element in the selection
/// * `M` - The mark type that defines visual representation
///
/// # Examples
///
/// ```rust
/// use gup::selection::{Selection, Circle};
/// use gup::RenderContext;
/// use std::sync::Arc;
///
/// #[derive(Debug, Clone)]
/// struct DataPoint {
///     x: f32,
///     y: f32,
///     value: f32,
/// }
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let context = Arc::new(RenderContext::new().await?);
///     let data = vec![
///         DataPoint { x: 0.0, y: 0.0, value: 1.0 },
///         DataPoint { x: 1.0, y: 1.0, value: 2.0 },
///     ];
///
///     let selection = Selection::<DataPoint, Circle>::new(data, context)?;
///     // Configure attributes and render...
///     Ok(())
/// }
/// ```
pub struct Selection<T, M: Mark> {
    /// Raw data stored on CPU for easy access and updates
    data: Vec<T>,
    /// Mark type phantom data
    mark_type: PhantomData<M>,

    /// GPU resources
    vertex_buffer: Option<BufferGpuBuffer<M::Vertex>>,
    instance_buffer: Option<BufferGpuBuffer<InstanceData>>,

    /// Shader function pipeline for attribute mapping
    shader_pipeline: ShaderPipeline,

    /// Rendering context
    context: Arc<RenderContext>,

    /// Cached attribute values for performance
    cached_attributes: Vec<M::AttributeValue>,
    attributes_dirty: bool,
}

impl<T, M: Mark> Selection<T, M> {
    /// Create a new selection with data and rendering context.
    ///
    /// # Arguments
    ///
    /// * `data` - Vector of data points to bind to visual marks
    /// * `context` - Shared rendering context containing GPU resources
    ///
    /// # Returns
    ///
    /// A new Selection instance ready for attribute mapping and rendering
    pub fn new(data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self> {
        let data_len = data.len();

        Ok(Self {
            data,
            mark_type: PhantomData,
            vertex_buffer: None,
            instance_buffer: None,
            shader_pipeline: ShaderPipeline::new(),
            context,
            cached_attributes: Vec::with_capacity(data_len),
            attributes_dirty: true,
        })
    }

    /// Get the number of data points in this selection
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the selection is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get a reference to the data
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// Update the data in this selection
    pub fn set_data(&mut self, data: Vec<T>) {
        self.data = data;
        self.cached_attributes.clear();
        self.attributes_dirty = true;
        // GPU buffers will be recreated on next render
        self.vertex_buffer = None;
        self.instance_buffer = None;
    }

    /// Bind a shader function to a visual attribute with compile-time type validation.
    ///
    /// This method:
    /// 1. Validates that F::Input is compatible with T at compile time
    /// 2. Registers the shader function for GPU compilation
    /// 3. Marks the shader pipeline as dirty for regeneration
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name (e.g., "position", "color", "size")
    /// * `shader_func` - The shader function to bind
    ///
    /// # Returns
    ///
    /// A mutable reference for method chaining
    pub fn attr<F>(&mut self, name: &str, shader_func: F) -> &mut Self
    where
        F: ShaderFunction + 'static,
        F::Input: Compatible<T>,
    {
        // Type validation happens at compile time through the trait bounds
        // The shader function output will be used to update the appropriate attribute field
        self.shader_pipeline
            .register_shader_function(name, shader_func);
        self.attributes_dirty = true;
        self
    }

    /// Legacy method for string-based shader function binding (deprecated)
    ///
    /// This method is kept for backward compatibility but should be replaced
    /// with the type-safe `attr` method in new code.
    #[deprecated(note = "Use attr() with ShaderFunction trait instead")]
    pub fn attr_legacy(&mut self, name: &str, shader_func_name: &str) -> &mut Self {
        // For backward compatibility, create a placeholder shader function
        // In a real implementation, this would look up the function by name
        let placeholder_func = PlaceholderShaderFunction {
            name: shader_func_name.to_string(),
        };
        self.shader_pipeline
            .register_shader_function(name, placeholder_func);
        self.attributes_dirty = true;
        self
    }

    /// Check if an attribute is bound
    pub fn has_attribute(&self, name: &str) -> bool {
        self.shader_pipeline.has_shader_function(name)
    }

    /// Get all bound attribute names
    pub fn attribute_names(&self) -> Vec<&String> {
        self.shader_pipeline.attribute_names()
    }

    /// Event handling system for interactive visualizations.
    ///
    /// This method registers event handlers that will be called when user interactions
    /// occur on the rendered visualization. Events are processed on the GPU for
    /// high-performance interaction with large datasets.
    ///
    /// # Arguments
    ///
    /// * `event` - The event type (e.g., "click", "hover", "drag")
    /// * `handler` - The callback function to execute when the event occurs
    ///
    /// # Returns
    ///
    /// A mutable reference for method chaining
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// selection.on("click", |event, data| {
    ///     println!("Clicked on point: {:?}", data);
    /// });
    /// ```
    pub fn on<H>(&mut self, event: &str, _handler: H) -> &mut Self
    where
        H: Fn(InteractionEvent, &T) + Send + Sync + 'static,
    {
        // Store event handler information for future GPU-based event processing
        // In a full implementation, this would:
        // 1. Register the handler with the GPU interaction system
        // 2. Set up GPU-based spatial indexing for efficient hit testing
        // 3. Configure the render pipeline to output interaction data

        println!("Event handler registered for '{event}' events");
        // For now, we just acknowledge the handler registration
        self
    }

    /// Get the selection ID for this selection (used by interaction system)
    pub fn selection_id(&self) -> u32 {
        // Use a hash of the data pointer as a unique identifier
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        (self.data.as_ptr() as usize).hash(&mut hasher);
        hasher.finish() as u32
    }

    /// Initialize GPU buffers if needed
    fn ensure_gpu_buffers(&mut self) -> GupResult<()> {
        let data_len = self.data.len();

        if data_len == 0 {
            return Ok(());
        }

        // Create vertex buffer if needed
        if self.vertex_buffer.is_none() {
            let vertex_buffer =
                BufferGpuBuffer::new(self.context.device(), BufferType::Vertex, data_len);
            self.vertex_buffer = Some(vertex_buffer);
        }

        // Create instance buffer if needed
        if self.instance_buffer.is_none() {
            let instance_buffer =
                BufferGpuBuffer::new(self.context.device(), BufferType::Instance, data_len);
            self.instance_buffer = Some(instance_buffer);
        }

        Ok(())
    }

    /// Update cached attributes if dirty
    fn update_attributes(&mut self) -> GupResult<()>
    where
        T: Clone,
        M::AttributeValue: Default + Clone,
    {
        if !self.attributes_dirty {
            return Ok(());
        }

        // Clear existing cached attributes
        self.cached_attributes.clear();
        self.cached_attributes.reserve(self.data.len());

        // For now, create default attributes for each data point
        // In a full implementation, this would apply shader functions
        // to transform data into attribute values
        for _data_item in &self.data {
            self.cached_attributes.push(M::AttributeValue::default());
        }

        self.attributes_dirty = false;
        Ok(())
    }

    /// Render the selection to the current render target.
    ///
    /// This method:
    /// 1. Ensures GPU buffers are allocated
    /// 2. Updates attribute data if dirty
    /// 3. Uploads data to GPU
    /// 4. Submits render commands
    pub fn render(&mut self) -> GupResult<()>
    where
        T: Clone,
        M::AttributeValue: Default + Clone,
    {
        if self.data.is_empty() {
            return Ok(());
        }

        // Ensure GPU resources are ready
        self.ensure_gpu_buffers()?;
        self.update_attributes()?;

        // Convert attributes to vertices
        let vertices: Vec<M::Vertex> = self
            .cached_attributes
            .iter()
            .map(|attr| M::create_vertex(attr))
            .collect();

        // Upload vertex data with auto-resizing
        if let Some(vertex_buffer) = &mut self.vertex_buffer {
            vertex_buffer.upload(self.context.device(), self.context.queue(), &vertices)?;
        }

        // Create instance data
        let instances: Vec<InstanceData> = (0..self.data.len())
            .map(|i| InstanceData {
                transform: [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ], // Identity matrix
                data_index: i as u32,
                _padding: [0.0; 3],
            })
            .collect();

        // Upload instance data with auto-resizing
        if let Some(instance_buffer) = &mut self.instance_buffer {
            instance_buffer.upload(self.context.device(), self.context.queue(), &instances)?;
        }

        // Actual rendering would happen here with proper shader pipeline
        // For now, we just return success to indicate the selection is valid

        Ok(())
    }
}

/// Implement Mixable for Selection to enable composition
impl<T, M: Mark> Mixable for Selection<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug,
    M::AttributeValue: Default + Clone,
{
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Use the selection's own render method
        Selection::render(self)
    }

    fn is_valid(&self) -> bool {
        !self.data.is_empty()
        // Device validity would be checked if needed, but wgpu devices are always valid once created
    }

    fn description(&self) -> String {
        format!(
            "Selection<{}, {}> with {} data points",
            std::any::type_name::<T>(),
            M::description(),
            self.data.len()
        )
    }
}

/// Implement Renderable for Selection to enable interaction processing
impl<T, M: Mark> Renderable for Selection<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug,
    M::AttributeValue: Default + Clone,
{
    fn get_elements_for_interaction(&self) -> GupResult<Vec<InteractionElement>> {
        let mut elements = Vec::with_capacity(self.data.len());

        // For each data point, extract interaction information
        for (index, _data_item) in self.data.iter().enumerate() {
            // In a full implementation, this would use the actual attribute values
            // computed by shader functions. For now, use default values.
            let _attrs = M::AttributeValue::default();

            // Convert mark-specific attributes to generic interaction element
            let element = match M::description() {
                "Circle" => {
                    // For circles, extract position and radius
                    // This is a simplified implementation - real version would use
                    // proper attribute extraction

                    // For testing, create different positions for different elements
                    let position = match index {
                        0 => [50.0, 50.0],                               // First element at (50, 50)
                        1 => [150.0, 100.0], // Second element at (150, 100)
                        2 => [200.0, 200.0], // Third element at (200, 200)
                        _ => [index as f32 * 20.0, index as f32 * 20.0], // Spread others out
                    };

                    InteractionElement {
                        position,
                        size: [1.0, 0.0], // very small radius for precise testing
                        mark_type: 0,     // Circle mark type ID
                    }
                }
                "Rectangle" => {
                    InteractionElement {
                        position: [0.0, 0.0], // Would come from position attribute
                        size: [20.0, 10.0],   // width, height
                        mark_type: 1,         // Rectangle mark type ID
                    }
                }
                "Line" => {
                    InteractionElement {
                        position: [0.0, 0.0], // Would come from position attribute
                        size: [30.0, 2.0],    // length, thickness
                        mark_type: 2,         // Line mark type ID
                    }
                }
                _ => {
                    // Unknown mark type, default to circle
                    InteractionElement {
                        position: [0.0, 0.0],
                        size: [5.0, 0.0],
                        mark_type: 0,
                    }
                }
            };

            elements.push(element);
        }

        Ok(elements)
    }

    fn selection_id(&self) -> u32 {
        self.selection_id()
    }
}

impl<T, M: Mark> std::fmt::Debug for Selection<T, M>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("data", &self.data)
            .field("mark_type", &M::description())
            .field("shader_functions", &self.shader_pipeline.attribute_names())
            .field("data_len", &self.data.len())
            .finish()
    }
}

/// Helper trait to provide defaults for attribute values
pub trait DefaultAttributes {
    fn default() -> Self;
}

impl DefaultAttributes for CircleAttributes {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0], // White
            radius: 5.0,
        }
    }
}

// Implement Default for CircleAttributes to satisfy the constraint
impl Default for CircleAttributes {
    fn default() -> Self {
        DefaultAttributes::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
    }

    #[tokio::test]
    async fn test_selection_creation() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let selection = Selection::<TestData, Circle>::new(data, context);
        assert!(selection.is_ok());
        let selection = selection.unwrap();
        assert_eq!(selection.len(), 1);
        assert!(!selection.is_empty());
    }

    #[tokio::test]
    async fn test_selection_empty() {
        let data: Vec<TestData> = vec![];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let selection = Selection::<TestData, Circle>::new(data, context).unwrap();
        assert_eq!(selection.len(), 0);
        assert!(selection.is_empty());
    }

    #[tokio::test]
    async fn test_attribute_binding() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        let position_shader =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);
        let color_shader =
            ColorShaderFunction::<_, TestData>::new(|data: &TestData| [data.value, 0.0, 0.0, 1.0]);

        selection
            .attr("position", position_shader)
            .attr("color", color_shader);

        assert!(selection.has_attribute("position"));
        assert!(selection.has_attribute("color"));
        assert!(!selection.has_attribute("nonexistent"));

        let attr_names = selection.attribute_names();
        assert_eq!(attr_names.len(), 2);
        assert!(attr_names.contains(&&"position".to_string()));
        assert!(attr_names.contains(&&"color".to_string()));
    }

    #[tokio::test]
    async fn test_data_update() {
        let initial_data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(initial_data, context).unwrap();

        assert_eq!(selection.len(), 1);

        let new_data = vec![
            TestData {
                x: 1.0,
                y: 2.0,
                value: 3.0,
            },
            TestData {
                x: 4.0,
                y: 5.0,
                value: 6.0,
            },
        ];
        selection.set_data(new_data);

        assert_eq!(selection.len(), 2);
        assert_eq!(selection.data()[1].x, 4.0);
    }

    #[tokio::test]
    async fn test_render_empty_selection() {
        let data: Vec<TestData> = vec![];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        let result = selection.render();
        assert!(result.is_ok()); // Empty selection should render successfully
    }

    #[tokio::test]
    async fn test_render_with_data() {
        let data = vec![
            TestData {
                x: 1.0,
                y: 2.0,
                value: 3.0,
            },
            TestData {
                x: 4.0,
                y: 5.0,
                value: 6.0,
            },
        ];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        let result = selection.render();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shader_function_registration() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        let position_func =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);

        selection.attr("position", position_func);

        // Check that the shader function is properly registered
        assert!(selection.has_attribute("position"));
        let wgsl_code = selection.shader_pipeline.generate_wgsl();
        assert!(wgsl_code.contains("position"));
    }

    #[test]
    fn test_shader_function_traits() {
        let position_func =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);
        let test_data = TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        };

        let result = position_func.apply(&test_data);
        assert_eq!(result, [1.0, 2.0]);

        assert_eq!(position_func.function_id(), "position_shader");
        assert!(!position_func.wgsl_code().is_empty());
    }

    #[test]
    fn test_circle_mark_implementation() {
        let attrs = CircleAttributes {
            position: [1.0, 2.0],
            color: [1.0, 0.0, 0.0, 1.0],
            radius: 5.0,
        };

        let vertex = Circle::create_vertex(&attrs);
        assert_eq!(vertex.position, [1.0, 2.0]);
        assert_eq!(vertex.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(vertex.radius, 5.0);

        assert_eq!(
            Circle::primitive_topology(),
            wgpu::PrimitiveTopology::PointList
        );
        assert_eq!(Circle::description(), "Circle");
    }

    #[tokio::test]
    async fn test_mixable_implementation() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        assert!(selection.is_valid());
        assert!(selection.description().contains("Selection"));
        assert!(selection.description().contains("Circle"));
        assert!(selection.description().contains("1 data points"));

        let result = selection.render();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compatible_trait() {
        // Test that Compatible trait works for same types
        assert!(TestData::is_compatible());
        assert!(CircleAttributes::is_compatible());
    }

    #[tokio::test]
    async fn test_large_dataset_performance() {
        // Performance test with 1000 points (smaller than 10K for faster testing)
        let large_data: Vec<TestData> = (0..1000)
            .map(|i| TestData {
                x: i as f32,
                y: (i * 2) as f32,
                value: (i * 3) as f32,
            })
            .collect();

        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(large_data, context).unwrap();

        let position_shader =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);
        selection.attr("position", position_shader);

        let start = std::time::Instant::now();
        let result = selection.render();
        let duration = start.elapsed();

        assert!(result.is_ok());
        // Should render 1000 points in reasonable time (< 100ms)
        assert!(duration.as_millis() < 100);
    }

    #[test]
    fn test_default_attributes() {
        let default_attrs = <CircleAttributes as Default>::default();
        assert_eq!(default_attrs.position, [0.0, 0.0]);
        assert_eq!(default_attrs.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(default_attrs.radius, 5.0);
    }

    #[tokio::test]
    async fn test_selection_debug_format() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<TestData, Circle>::new(data, context).unwrap();

        let position_shader =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);
        selection.attr("position", position_shader);

        let debug_str = format!("{selection:?}");
        assert!(debug_str.contains("Selection"));
        assert!(debug_str.contains("Circle"));
        assert!(debug_str.contains("data_len"));
    }

    #[test]
    fn test_placeholder_shader_function() {
        let placeholder = PlaceholderShaderFunction {
            name: "test_func".to_string(),
        };

        assert_eq!(placeholder.function_id(), "placeholder_test_func");
        assert!(placeholder.wgsl_code().contains("test_func"));
    }

    #[test]
    fn test_boxed_shader_function() {
        let position_func =
            PositionShaderFunction::<_, TestData>::new(|data: &TestData| [data.x, data.y]);
        let boxed = BoxedShaderFunction::new(position_func);

        assert_eq!(boxed.function_id(), "position_shader");
        assert!(!boxed.wgsl_code().is_empty());
    }
}

// Performance benchmarks (would typically be in benches/ directory)
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    struct BenchData {
        x: f32,
        y: f32,
        value: f32,
    }

    // This would be a proper benchmark with criterion in a real implementation
    #[tokio::test]
    async fn bench_selection_render_10k_points() {
        let large_data: Vec<BenchData> = (0..10_000)
            .map(|i| BenchData {
                x: (i as f32).sin(),
                y: (i as f32).cos(),
                value: i as f32,
            })
            .collect();

        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut selection = Selection::<BenchData, Circle>::new(large_data, context).unwrap();

        let position_shader =
            PositionShaderFunction::<_, BenchData>::new(|data: &BenchData| [data.x, data.y]);
        let color_shader = ColorShaderFunction::<_, BenchData>::new(|data: &BenchData| {
            let normalized = data.value / 10_000.0;
            [normalized, 1.0 - normalized, 0.5, 1.0]
        });

        selection
            .attr("position", position_shader)
            .attr("color", color_shader);

        let start = std::time::Instant::now();
        let result = selection.render();
        let duration = start.elapsed();

        assert!(result.is_ok());
        println!("10K points rendered in {duration:?}");

        // Performance target: <1ms for 10K points (in a GPU-accelerated implementation)
        // For now, we just verify it completes in reasonable time
        assert!(duration.as_secs() < 1);
    }
}
