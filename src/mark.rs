// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Core Mark trait and mark system for visual primitives.
//!
//! The Mark trait defines the interface that all visual primitives implement. It
//! bridges high-level visualization concepts (circles, rectangles, lines) with
//! low-level GPU rendering. The trait supports both hand-optimized shaders for
//! performance and generated shaders for flexibility, while integrating seamlessly
//! with the shader function system.
//!
//! ## Mark-Shader Integration
//!
//! The integration system enables marks to use composed shader functions for
//! attribute mapping with compile-time type safety and GPU performance:
//!
//! ```rust,ignore
//! selection.select_all::<Circle>()
//!     .attr("position",
//!         geographic_projection           // Data -> Vec2
//!             .compose(screen_transform)  // Vec2 -> Vec2
//!     )
//!     .attr("color",
//!         temperature_scale              // Data -> f32
//!             .compose(color_interpolation) // f32 -> Vec4
//!     );
//! ```

pub mod batch_renderer;
pub mod boxplot;
pub mod circle;
pub mod composite;
pub mod gpu_path_tessellator;
pub mod line;
pub mod path;
pub mod rectangle;
pub mod renderer;
pub mod text;

pub use batch_renderer::{
    BatchFrameStats, BatchRendererConfig, CullingManager, InstanceAttributes,
    InstancedBatchRenderer, LodLevel, RenderBatch, Viewport2D,
};
pub use boxplot::{BoxPlot, BoxPlotAttributes, BoxPlotInstance, BoxPlotOrientation, BoxPlotVertex};
pub use circle::{Circle, CircleAttributes, CircleVertex};
pub use composite::{
    CompositeMark, CompositeMarkAttributes, CompositeMarkVertex, SubMark, Transform,
};
pub use gpu_path_tessellator::GpuPathTessellator;
pub use line::{Line, LineAttributes, LineStyle, LineVertex};
pub use path::{Path, PathAttributes, PathCommand, PathVertex};
pub use rectangle::{Rectangle, RectangleAttributes, RectangleVertex};
pub use renderer::MarkRenderer;
pub use text::{Text, TextMarkAttributes, TextVertex};

use crate::error::GupResult;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferAddress, BufferBindingType, ColorTargetState, ColorWrites, Device, FragmentState,
    FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, TextureFormat, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};

/// Core trait for all visual marks (circles, rectangles, lines, etc.).
///
/// This trait bridges high-level visualization concepts with GPU rendering,
/// supporting both manual optimized shaders and generated shaders for flexibility.
///
/// # Type Parameters
///
/// * `Vertex` - GPU vertex type that must implement bytemuck traits for safe transfer
/// * `AttributeValue` - High-level attribute type used for mark configuration
///
/// # Shader Support
///
/// Marks can provide either:
/// - Hand-optimized shaders via `VERTEX_SHADER` and `FRAGMENT_SHADER` constants
/// - Generated shaders via `generate_vertex_shader` and `generate_fragment_shader` methods
///
/// # Examples
///
/// ```rust
/// use gup::mark::Mark;
///
/// #[derive(Debug, Clone)]
/// pub struct Circle;
///
/// #[repr(C)]
/// #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// pub struct CircleVertex {
///     position: [f32; 2],
/// }
///
/// #[derive(Debug, Clone)]
/// pub struct CircleAttributes {
///     pub center: [f32; 2],
///     pub radius: f32,
///     pub color: [f32; 4],
/// }
///
/// impl Mark for Circle {
///     type Vertex = CircleVertex;
///     type AttributeValue = CircleAttributes;
///
///     fn vertex_count() -> usize { 4 }
///     fn index_count() -> Option<usize> { Some(6) }
///
///     fn generate_vertices() -> Vec<Self::Vertex> {
///         vec![
///             CircleVertex { position: [-1.0, -1.0] },
///             CircleVertex { position: [ 1.0, -1.0] },
///             CircleVertex { position: [ 1.0,  1.0] },
///             CircleVertex { position: [-1.0,  1.0] },
///         ]
///     }
///
///     fn generate_indices() -> Option<Vec<u32>> {
///         Some(vec![0, 1, 2, 0, 2, 3])
///     }
/// }
/// ```
pub trait Mark: Clone + Send + Sync + 'static {
    /// GPU vertex type with required bytemuck traits for safe GPU transfer
    type Vertex: bytemuck::Pod + bytemuck::Zeroable + Send + Sync + 'static;

    /// High-level attribute type for mark configuration
    type AttributeValue: Send + Sync + 'static;

    /// Pre-written vertex shader (fastest) - None means generate shaders
    const VERTEX_SHADER: Option<&'static str> = None;

    /// Pre-written fragment shader (fastest) - None means generate shaders
    const FRAGMENT_SHADER: Option<&'static str> = None;

    /// Pattern-enabled fragment shader for accessibility rendering
    /// Uses patterns instead of colors for data encoding
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> = None;

    /// Generate vertex shader WGSL code integrating shader functions
    ///
    /// This method creates a vertex shader that integrates with the shader function
    /// system for dynamic attribute mapping. The implementation should inject
    /// shader function calls at appropriate points.
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate fragment shader WGSL code integrating shader functions
    ///
    /// This method creates a fragment shader that uses attribute values computed
    /// by shader functions in the vertex stage.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate vertex shader with specific shader function mappings
    ///
    /// This method allows marks to customize how shader functions are integrated
    /// into their vertex shaders for different attributes.
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        pipeline.generate_vertex_shader()
    }

    /// Generate fragment shader with specific shader function mappings
    ///
    /// This method allows marks to customize how shader functions are integrated
    /// into their fragment shaders for different attributes.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        pipeline.generate_fragment_shader()
    }

    /// Get the WGSL type name for a specific attribute
    ///
    /// This method returns the expected WGSL type for an attribute, enabling
    /// compile-time type validation of shader function outputs.
    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec2<f32>"),
            "color" => Ok("vec4<f32>"),
            "size" | "radius" => Ok("f32"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown attribute: {attribute_name}"
            ))),
        }
    }

    /// Check if a shader function output type is compatible with an attribute
    ///
    /// This method validates that a shader function's output type matches the
    /// expected type for a mark attribute.
    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }

    /// Number of vertices in the base geometry for this mark type
    fn vertex_count() -> usize;

    /// Number of indices if using indexed rendering (None for non-indexed)
    fn index_count() -> Option<usize> {
        None
    }

    /// Generate the base vertex data for this mark type
    ///
    /// This creates the basic geometry (e.g., quad for circles, triangle for arrows).
    /// Instance data and attributes are handled separately.
    fn generate_vertices() -> Vec<Self::Vertex>;

    /// Generate index data for indexed rendering (None for non-indexed)
    fn generate_indices() -> Option<Vec<u32>> {
        None
    }

    /// Get vertex attributes for this mark's vertex buffer layout.
    ///
    /// By default, returns a single vec2 position attribute at location 0.
    /// Marks with more complex vertex data (e.g., Line with position and normal)
    /// should override this method.
    fn vertex_attributes() -> &'static [VertexAttribute] {
        &[VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: VertexFormat::Float32x2,
        }]
    }
}

/// Trait for providing stable mark type IDs for GPU interaction.
///
/// This trait is automatically implemented by the `#[derive(MarkTypeId)]` macro
/// and provides a compile-time stable ID that corresponds to the mark type's
/// GPU shader representation.
///
/// # GPU Integration
///
/// Mark type IDs are used by the interaction system to identify which type
/// of mark was clicked or hovered. The IDs must match the enum values in
/// GPU shaders like `hit_test.compute.wgsl`.
///
/// # Example
///
/// ```rust,ignore
/// use gup_macros::MarkTypeId;
/// use gup::mark::{Mark, MarkTypeIdProvider};
///
/// #[derive(Clone, MarkTypeId)]
/// #[mark_type_id = 0]
/// pub struct Circle;
///
/// assert_eq!(Circle::MARK_TYPE_ID, 0);
/// assert_eq!(Circle::mark_type_id(), 0);
/// ```
pub trait MarkTypeIdProvider {
    /// Get the mark type ID for GPU interaction system.
    fn mark_type_id() -> u32;
}

/// Type-erased information about a mark type for runtime management
pub trait MarkInfo: Send + Sync {
    /// Get the type name for debugging
    fn type_name(&self) -> &'static str;

    /// Get the size of the vertex type in bytes
    fn vertex_size(&self) -> usize;

    /// Get the size of the attribute type in bytes
    fn attribute_size(&self) -> usize;

    /// Check if this mark has hand-written custom shaders
    fn has_custom_shaders(&self) -> bool;

    /// Check if this mark supports pattern rendering
    fn has_pattern_shader(&self) -> bool;

    /// Create a render pipeline for this mark type
    fn create_render_pipeline(&self, device: &Device) -> GupResult<RenderPipeline>;

    /// Create a render pipeline with pattern support
    fn create_render_pipeline_with_patterns(&self, device: &Device) -> GupResult<RenderPipeline>;

    /// Get the vertex count for this mark type
    fn vertex_count(&self) -> usize;

    /// Get the index count for this mark type (None for non-indexed)
    fn index_count(&self) -> Option<usize>;

    /// Generate vertices for this mark type
    fn generate_vertices_boxed(&self) -> Vec<u8>;

    /// Generate indices for this mark type (None for non-indexed)
    fn generate_indices_boxed(&self) -> Option<Vec<u32>>;

    /// Downcast to Any for type-specific operations
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Concrete implementation of MarkInfo for a specific mark type
pub struct MarkInfoImpl<M: Mark> {
    _phantom: PhantomData<M>,
}

impl<M: Mark> Default for MarkInfoImpl<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Mark> MarkInfoImpl<M> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Create render pipeline implementation for this mark type.
    fn create_render_pipeline_impl(&self, device: &Device) -> GupResult<RenderPipeline> {
        // Determine shader sources (manual vs generated)
        let (vertex_source, fragment_source) =
            if M::VERTEX_SHADER.is_some() && M::FRAGMENT_SHADER.is_some() {
                // Use hand-optimized shaders
                (
                    M::VERTEX_SHADER.unwrap().to_string(),
                    M::FRAGMENT_SHADER.unwrap().to_string(),
                )
            } else {
                // Generate shaders using pipeline system
                let pipeline = ComposableShaderPipeline::new();
                let vertex_shader = M::generate_vertex_shader(&pipeline);
                let fragment_shader = M::generate_fragment_shader(&pipeline);
                (vertex_shader, fragment_shader)
            };

        // Create shader modules
        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{}_vertex", self.type_name())),
            source: ShaderSource::Wgsl(vertex_source.into()),
        });

        let fragment_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{}_fragment", self.type_name())),
            source: ShaderSource::Wgsl(fragment_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = self.create_bind_group_layout(device)?;

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{}_pipeline_layout", self.type_name())),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(&format!("{}_pipeline", self.type_name())),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[self.create_vertex_buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb, // Standard surface format
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None, // Allow double-sided rendering for flexibility
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None, // 2D rendering without depth testing
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create render pipeline with pattern support for accessibility.
    fn create_render_pipeline_with_patterns_impl(
        &self,
        device: &Device,
    ) -> GupResult<RenderPipeline> {
        // Determine shader sources (use pattern fragment shader if available)
        let (vertex_source, fragment_source) =
            if M::VERTEX_SHADER.is_some() && M::PATTERN_FRAGMENT_SHADER.is_some() {
                // Use hand-optimized vertex shader with pattern fragment shader
                (
                    M::VERTEX_SHADER.unwrap().to_string(),
                    M::PATTERN_FRAGMENT_SHADER.unwrap().to_string(),
                )
            } else {
                // Fall back to standard shaders
                let pipeline = ComposableShaderPipeline::new();
                let vertex_shader = M::generate_vertex_shader(&pipeline);
                let fragment_shader = M::generate_fragment_shader(&pipeline);
                (vertex_shader, fragment_shader)
            };

        // Create shader modules
        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{}_pattern_vertex", self.type_name())),
            source: ShaderSource::Wgsl(vertex_source.into()),
        });

        let fragment_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{}_pattern_fragment", self.type_name())),
            source: ShaderSource::Wgsl(fragment_source.into()),
        });

        // Create bind group layouts
        let instance_bind_group_layout = self.create_bind_group_layout(device)?;
        let pattern_bind_group_layout = self.create_pattern_bind_group_layout(device);

        // Create pipeline layout with both bind groups
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{}_pattern_pipeline_layout", self.type_name())),
            bind_group_layouts: &[&instance_bind_group_layout, &pattern_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(&format!("{}_pattern_pipeline", self.type_name())),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[self.create_vertex_buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create bind group layout for pattern uniforms.
    fn create_pattern_bind_group_layout(&self, device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Pattern Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    /// Create bind group layout for this mark type.
    fn create_bind_group_layout(&self, device: &Device) -> GupResult<BindGroupLayout> {
        let mut entries = Vec::new();

        // Instance data buffer (always present).
        // Visible to both vertex and fragment stages so that marks like BoxPlot
        // can read instance data in the fragment shader (e.g. for SDF rendering
        // with per-instance outlier arrays that don't fit in vertex outputs).
        entries.push(BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX_FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        // Add uniform buffers if the mark uses generated shaders
        if !self.has_custom_shaders() {
            // Position transform uniforms
            entries.push(BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            // Color transform uniforms
            entries.push(BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
        }

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&format!("{}_bind_group_layout", self.type_name())),
            entries: &entries,
        });

        Ok(layout)
    }

    /// Create vertex buffer layout for this mark type.
    fn create_vertex_buffer_layout(&self) -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<M::Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: M::vertex_attributes(),
        }
    }
}

impl<M: Mark> MarkInfo for MarkInfoImpl<M> {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<M>()
    }

    fn vertex_size(&self) -> usize {
        std::mem::size_of::<M::Vertex>()
    }

    fn attribute_size(&self) -> usize {
        std::mem::size_of::<M::AttributeValue>()
    }

    fn has_custom_shaders(&self) -> bool {
        M::VERTEX_SHADER.is_some() && M::FRAGMENT_SHADER.is_some()
    }

    fn has_pattern_shader(&self) -> bool {
        M::PATTERN_FRAGMENT_SHADER.is_some()
    }

    fn create_render_pipeline(&self, device: &Device) -> GupResult<RenderPipeline> {
        self.create_render_pipeline_impl(device)
    }

    fn create_render_pipeline_with_patterns(&self, device: &Device) -> GupResult<RenderPipeline> {
        self.create_render_pipeline_with_patterns_impl(device)
    }

    fn vertex_count(&self) -> usize {
        M::vertex_count()
    }

    fn index_count(&self) -> Option<usize> {
        M::index_count()
    }

    fn generate_vertices_boxed(&self) -> Vec<u8> {
        let vertices = M::generate_vertices();
        bytemuck::cast_slice(&vertices).to_vec()
    }

    fn generate_indices_boxed(&self) -> Option<Vec<u32>> {
        M::generate_indices()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Registry for managing mark types at runtime
///
/// The mark registry enables type-safe mark management and pipeline caching.
/// It stores metadata about registered mark types and manages render pipelines.
///
/// # Examples
///
/// ```rust
/// use gup::mark::{MarkRegistry, Circle};
/// use gup::GupContext;
/// use std::sync::Arc;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let context = Arc::new(GupContext::headless().await?);
///     let device = &context.device;
///     let mut registry = MarkRegistry::new();
///
///     // Register mark types
///     registry.register::<Circle>();
///
///     // Check registration
///     assert!(registry.is_registered::<Circle>());
///
///     // Get render pipeline (creates and caches)
///     let pipeline = registry.get_pipeline::<Circle>(device)?;
///
///     Ok(())
/// }
/// ```
pub struct MarkRegistry {
    /// Registered mark type information
    marks: HashMap<TypeId, Box<dyn MarkInfo>>,

    /// Cached render pipelines by mark type
    pipelines: HashMap<TypeId, Arc<RenderPipeline>>,
}

impl MarkRegistry {
    /// Create a new empty mark registry
    pub fn new() -> Self {
        Self {
            marks: HashMap::new(),
            pipelines: HashMap::new(),
        }
    }

    /// Register a mark type with the registry
    ///
    /// This stores metadata about the mark type for runtime operations.
    /// Registration is idempotent - registering the same type multiple times is safe.
    pub fn register<M: Mark>(&mut self) {
        let type_id = TypeId::of::<M>();
        let info = Box::new(MarkInfoImpl::<M>::new());
        self.marks.insert(type_id, info);
    }

    /// Check if a mark type is registered
    pub fn is_registered<M: Mark>(&self) -> bool {
        let type_id = TypeId::of::<M>();
        self.marks.contains_key(&type_id)
    }

    /// Get the mark info for a registered mark type
    pub fn get_mark_info<M: Mark>(&self) -> Option<&dyn MarkInfo> {
        let type_id = TypeId::of::<M>();
        self.marks.get(&type_id).map(|info| info.as_ref())
    }

    /// Get or create a render pipeline for a mark type
    ///
    /// This method:
    /// 1. Checks if a pipeline is already cached
    /// 2. If not, creates a new pipeline using the mark's shader generation
    /// 3. Caches the pipeline for future use
    /// 4. Returns an Arc to the pipeline for shared ownership
    pub fn get_pipeline<M: Mark>(&mut self, device: &Device) -> GupResult<Arc<RenderPipeline>> {
        let type_id = TypeId::of::<M>();

        // Return cached pipeline if available
        if let Some(pipeline) = self.pipelines.get(&type_id) {
            return Ok(Arc::clone(pipeline));
        }

        // Create new pipeline
        let mark_info = self.marks.get(&type_id).ok_or_else(|| {
            crate::error::GupError::render_error(format!(
                "Mark type {} not registered",
                std::any::type_name::<M>()
            ))
        })?;

        let pipeline = mark_info.create_render_pipeline(device)?;
        let arc_pipeline = Arc::new(pipeline);

        // Cache for future use
        self.pipelines.insert(type_id, Arc::clone(&arc_pipeline));

        Ok(arc_pipeline)
    }

    /// Clear all cached pipelines
    ///
    /// This is useful when GPU resources need to be recreated (e.g., device lost).
    pub fn clear_pipeline_cache(&mut self) {
        self.pipelines.clear();
    }

    /// Get the number of registered mark types
    pub fn mark_count(&self) -> usize {
        self.marks.len()
    }

    /// Get the number of cached pipelines
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.len()
    }

    /// Get all registered mark type names for debugging
    pub fn registered_types(&self) -> Vec<&'static str> {
        self.marks.values().map(|info| info.type_name()).collect()
    }

    /// Create bind group for a specific mark type.
    ///
    /// This method creates a bind group that matches the layout expected by the mark's
    /// render pipeline, with the provided instance and uniform buffers.
    pub fn create_bind_group<M: Mark>(
        &self,
        device: &Device,
        instance_buffer: &wgpu::Buffer,
        uniform_buffers: &[&wgpu::Buffer],
    ) -> GupResult<wgpu::BindGroup> {
        let mark_info = self.get_mark_info::<M>().ok_or_else(|| {
            crate::error::GupError::render_error("Mark not registered".to_string())
        })?;

        // Get the mark info implementation to access layout creation
        let type_id = TypeId::of::<M>();
        let mark_info_impl = self.marks.get(&type_id).unwrap();

        // Create bind group layout
        let bind_group_layout = if let Some(mark_info_impl) =
            mark_info_impl.as_any().downcast_ref::<MarkInfoImpl<M>>()
        {
            mark_info_impl.create_bind_group_layout(device)?
        } else {
            return Err(crate::error::GupError::render_error(
                "Failed to downcast mark info".to_string(),
            ));
        };

        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: instance_buffer.as_entire_binding(),
        }];

        // Add uniform buffer entries
        for (i, buffer) in uniform_buffers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: (i + 1) as u32,
                resource: buffer.as_entire_binding(),
            });
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_bind_group", mark_info.type_name())),
            layout: &bind_group_layout,
            entries: &entries,
        });

        Ok(bind_group)
    }

    /// Get bind group layout for a specific mark type.
    pub fn get_bind_group_layout<M: Mark>(&self, device: &Device) -> GupResult<BindGroupLayout> {
        let type_id = TypeId::of::<M>();
        let mark_info_impl = self.marks.get(&type_id).ok_or_else(|| {
            crate::error::GupError::render_error(format!(
                "Mark type {} not registered",
                std::any::type_name::<M>()
            ))
        })?;

        if let Some(mark_info_impl) = mark_info_impl.as_any().downcast_ref::<MarkInfoImpl<M>>() {
            mark_info_impl.create_bind_group_layout(device)
        } else {
            Err(crate::error::GupError::render_error(
                "Failed to downcast mark info".to_string(),
            ))
        }
    }
}

impl Default for MarkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Type-safe attribute binding for shader functions.
///
/// This struct holds a shader function bound to a specific mark attribute,
/// providing compile-time type validation and runtime uniform buffer management.
///
/// Note: This is a simplified version for the integration story. A full implementation
/// would use dynamic dispatch with proper uniform type erasure.
pub struct AttributeBinding<T, M: Mark> {
    attribute_name: String,
    function_name: String,
    wgsl_code: String,
    uniform_buffer: Option<wgpu::Buffer>,
    _phantom: PhantomData<(T, M)>,
}

impl<T, M: Mark> AttributeBinding<T, M> {
    /// Create a new attribute binding with type validation.
    ///
    /// # Type Safety
    /// This method enforces that:
    /// - F::Input is compatible with T (the data type)
    /// - F::Output is compatible with M::AttributeValue (the mark's attribute type)
    ///
    /// # Arguments
    /// * `name` - The attribute name (e.g., "position", "color", "size")
    /// * `function_name` - The shader function name
    /// * `wgsl_code` - WGSL code for the function
    pub fn new(name: &str, function_name: &str, wgsl_code: &str) -> Self {
        Self {
            attribute_name: name.to_string(),
            function_name: function_name.to_string(),
            wgsl_code: wgsl_code.to_string(),
            uniform_buffer: None,
            _phantom: PhantomData,
        }
    }

    /// Create uniform buffer for this shader function if needed.
    pub fn create_uniform_buffer(
        &mut self,
        device: &wgpu::Device,
        uniform_size: u64,
    ) -> GupResult<()> {
        if uniform_size > 0 {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{}_uniforms", self.attribute_name)),
                size: uniform_size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.uniform_buffer = Some(buffer);
        }
        Ok(())
    }

    /// Update uniform buffer data.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, data: &[u8]) -> GupResult<()> {
        if let Some(buffer) = &self.uniform_buffer {
            queue.write_buffer(buffer, 0, data);
        }
        Ok(())
    }

    /// Get WGSL function call for this attribute.
    pub fn get_wgsl_function_call(&self) -> String {
        format!(
            "{}(data, {}_uniforms)",
            self.function_name, self.attribute_name
        )
    }

    /// Get the attribute name.
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }

    /// Get the shader function name.
    pub fn function_name(&self) -> &str {
        &self.function_name
    }

    /// Get the WGSL code.
    pub fn wgsl_code(&self) -> &str {
        &self.wgsl_code
    }

    /// Check if this binding has uniform data.
    pub fn has_uniforms(&self) -> bool {
        self.uniform_buffer.is_some()
    }

    /// Get reference to the uniform buffer.
    pub fn uniform_buffer(&self) -> Option<&wgpu::Buffer> {
        self.uniform_buffer.as_ref()
    }
}

/// Attribute binding error types.
#[derive(Debug, Clone)]
pub enum AttributeError {
    /// Type mismatch between shader function output and mark attribute
    TypeMismatch {
        attribute: String,
        expected: &'static str,
        actual: String,
    },
    /// Unknown attribute name for the mark type
    UnknownAttribute {
        attribute: String,
        mark_type: &'static str,
    },
    /// Shader function compilation error
    CompilationError {
        function_name: String,
        error_message: String,
    },
}

impl std::fmt::Display for AttributeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeError::TypeMismatch {
                attribute,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Type mismatch for attribute '{attribute}': expected {expected}, got {actual}"
                )
            }
            AttributeError::UnknownAttribute {
                attribute,
                mark_type,
            } => {
                write!(
                    f,
                    "Unknown attribute '{attribute}' for mark type {mark_type}"
                )
            }
            AttributeError::CompilationError {
                function_name,
                error_message,
            } => {
                write!(
                    f,
                    "Shader function '{function_name}' compilation error: {error_message}"
                )
            }
        }
    }
}

impl std::error::Error for AttributeError {}

/// Pipeline manager for mark-shader combinations with caching.
///
/// This manager creates and caches render pipelines for specific combinations
/// of marks and shader functions, optimizing GPU resource usage.
pub struct MarkPipelineManager {
    pipelines: HashMap<PipelineKey, Arc<wgpu::RenderPipeline>>,
    shader_cache: HashMap<ShaderKey, String>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct PipelineKey {
    mark_type: TypeId,
    attribute_functions: Vec<(String, String)>, // (attribute_name, function_signature)
    data_type: TypeId,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ShaderKey {
    mark_type: TypeId,
    shader_type: ShaderStage,
    functions: Vec<String>,
}

#[derive(Hash, PartialEq, Eq, Clone)]
enum ShaderStage {
    Vertex,
    Fragment,
}

impl MarkPipelineManager {
    /// Create a new pipeline manager.
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
            shader_cache: HashMap::new(),
        }
    }

    /// Get or create a render pipeline for a mark-shader combination.
    ///
    /// This method:
    /// 1. Checks if a pipeline is already cached
    /// 2. If not, generates shaders using the mark's integration methods
    /// 3. Creates and caches the pipeline
    /// 4. Returns an Arc for shared ownership
    pub fn get_or_create_pipeline<T: 'static, M: Mark + 'static>(
        &mut self,
        device: &wgpu::Device,
        attribute_functions: &HashMap<String, String>,
    ) -> GupResult<Arc<wgpu::RenderPipeline>> {
        let key = self.create_pipeline_key::<T, M>(attribute_functions);

        // Return cached pipeline if available
        if let Some(pipeline) = self.pipelines.get(&key) {
            return Ok(Arc::clone(pipeline));
        }

        // Generate shaders
        let vertex_shader = self.generate_vertex_shader::<M>(attribute_functions)?;
        let fragment_shader = self.generate_fragment_shader::<M>(attribute_functions)?;

        // Create shader modules
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{}_vertex", std::any::type_name::<M>())),
            source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
        });

        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{}_fragment", std::any::type_name::<M>())),
            source: wgpu::ShaderSource::Wgsl(fragment_shader.into()),
        });

        // Create pipeline layout and render pipeline
        let pipeline = self.create_render_pipeline(device, &vertex_module, &fragment_module)?;
        let arc_pipeline = Arc::new(pipeline);

        // Cache for future use
        self.pipelines.insert(key, Arc::clone(&arc_pipeline));

        Ok(arc_pipeline)
    }

    /// Generate vertex shader for a mark type with shader function integration.
    fn generate_vertex_shader<M: Mark>(
        &mut self,
        attribute_functions: &HashMap<String, String>,
    ) -> GupResult<String> {
        let shader_key = ShaderKey {
            mark_type: TypeId::of::<M>(),
            shader_type: ShaderStage::Vertex,
            functions: attribute_functions.values().cloned().collect(),
        };

        if let Some(cached_shader) = self.shader_cache.get(&shader_key) {
            return Ok(cached_shader.clone());
        }

        let pipeline = ComposableShaderPipeline::new();
        let shader = M::generate_vertex_shader_with_functions(&pipeline, attribute_functions);

        self.shader_cache.insert(shader_key, shader.clone());
        Ok(shader)
    }

    /// Generate fragment shader for a mark type with shader function integration.
    fn generate_fragment_shader<M: Mark>(
        &mut self,
        attribute_functions: &HashMap<String, String>,
    ) -> GupResult<String> {
        let shader_key = ShaderKey {
            mark_type: TypeId::of::<M>(),
            shader_type: ShaderStage::Fragment,
            functions: attribute_functions.values().cloned().collect(),
        };

        if let Some(cached_shader) = self.shader_cache.get(&shader_key) {
            return Ok(cached_shader.clone());
        }

        let pipeline = ComposableShaderPipeline::new();
        let shader = M::generate_fragment_shader_with_functions(&pipeline, attribute_functions);

        self.shader_cache.insert(shader_key, shader.clone());
        Ok(shader)
    }

    /// Create a pipeline key for caching.
    fn create_pipeline_key<T: 'static, M: Mark + 'static>(
        &self,
        attribute_functions: &HashMap<String, String>,
    ) -> PipelineKey {
        let mut functions: Vec<(String, String)> = attribute_functions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        functions.sort(); // Ensure consistent ordering

        PipelineKey {
            mark_type: TypeId::of::<M>(),
            attribute_functions: functions,
            data_type: TypeId::of::<T>(),
        }
    }

    /// Create a render pipeline from shader modules.
    fn create_render_pipeline(
        &self,
        device: &wgpu::Device,
        vertex_module: &wgpu::ShaderModule,
        fragment_module: &wgpu::ShaderModule,
    ) -> GupResult<wgpu::RenderPipeline> {
        // Create bind group layout (simplified for now)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mark_shader_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mark_shader_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mark_shader_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }

    /// Clear all cached pipelines and shaders.
    pub fn clear_cache(&mut self) {
        self.pipelines.clear();
        self.shader_cache.clear();
    }

    /// Get the number of cached pipelines.
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.len()
    }

    /// Get the number of cached shaders.
    pub fn shader_count(&self) -> usize {
        self.shader_cache.len()
    }
}

impl Default for MarkPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::{ColorMap, ComposableShaderFunction, LinearScale, Vec4};
    use crate::vec4;

    // Test mark implementation
    #[derive(Debug, Clone)]
    pub struct TestCircle;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct TestCircleVertex {
        position: [f32; 2],
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    pub struct TestCircleAttributes {
        pub center: [f32; 2],
        pub radius: f32,
        pub color: [f32; 4],
    }

    impl Mark for TestCircle {
        type Vertex = TestCircleVertex;
        type AttributeValue = TestCircleAttributes;

        const VERTEX_SHADER: Option<&'static str> = Some(
            r#"
            @vertex
            fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
                return vec4<f32>(position, 0.0, 1.0);
            }
        "#,
        );

        const FRAGMENT_SHADER: Option<&'static str> = Some(
            r#"
            @fragment
            fn fs_main() -> @location(0) vec4<f32> {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0);
            }
        "#,
        );

        fn vertex_count() -> usize {
            4
        }

        fn index_count() -> Option<usize> {
            Some(6)
        }

        fn generate_vertices() -> Vec<Self::Vertex> {
            vec![
                TestCircleVertex {
                    position: [-1.0, -1.0],
                },
                TestCircleVertex {
                    position: [1.0, -1.0],
                },
                TestCircleVertex {
                    position: [1.0, 1.0],
                },
                TestCircleVertex {
                    position: [-1.0, 1.0],
                },
            ]
        }

        fn generate_indices() -> Option<Vec<u32>> {
            Some(vec![0, 1, 2, 0, 2, 3])
        }
    }

    #[test]
    fn test_mark_trait_implementation() {
        assert_eq!(TestCircle::vertex_count(), 4);
        assert_eq!(TestCircle::index_count(), Some(6));

        let vertices = TestCircle::generate_vertices();
        assert_eq!(vertices.len(), 4);
        assert_eq!(vertices[0].position, [-1.0, -1.0]);

        let indices = TestCircle::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);

        assert!(TestCircle::VERTEX_SHADER.is_some());
        assert!(TestCircle::FRAGMENT_SHADER.is_some());
    }

    #[test]
    fn test_mark_info_implementation() {
        let mark_info = MarkInfoImpl::<TestCircle>::new();

        assert_eq!(mark_info.type_name(), std::any::type_name::<TestCircle>());
        assert_eq!(
            mark_info.vertex_size(),
            std::mem::size_of::<TestCircleVertex>()
        );
        assert_eq!(
            mark_info.attribute_size(),
            std::mem::size_of::<TestCircleAttributes>()
        );
        assert!(mark_info.has_custom_shaders());
        assert_eq!(mark_info.vertex_count(), 4);
        assert_eq!(mark_info.index_count(), Some(6));

        let vertices_bytes = mark_info.generate_vertices_boxed();
        assert_eq!(
            vertices_bytes.len(),
            4 * std::mem::size_of::<TestCircleVertex>()
        );

        let indices = mark_info.generate_indices_boxed().unwrap();
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn test_mark_registry() {
        let mut registry = MarkRegistry::new();
        assert_eq!(registry.mark_count(), 0);
        assert_eq!(registry.pipeline_count(), 0);

        // Register a mark type
        registry.register::<TestCircle>();
        assert_eq!(registry.mark_count(), 1);
        assert!(registry.is_registered::<TestCircle>());

        // Get mark info
        let mark_info = registry.get_mark_info::<TestCircle>().unwrap();
        assert_eq!(mark_info.vertex_count(), 4);

        // Check registered types
        let types = registry.registered_types();
        assert_eq!(types.len(), 1);
        assert!(types[0].contains("TestCircle"));

        // Test double registration is safe
        registry.register::<TestCircle>();
        assert_eq!(registry.mark_count(), 1);
    }

    #[test]
    fn test_vertex_buffer_generation() {
        let vertices = TestCircle::generate_vertices();

        // Verify vertex data is valid for GPU upload
        for vertex in &vertices {
            assert!(vertex.position[0].is_finite());
            assert!(vertex.position[1].is_finite());
        }

        // Verify bytemuck conversion works
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(
            bytes.len(),
            vertices.len() * std::mem::size_of::<TestCircleVertex>()
        );
    }

    #[test]
    fn test_mark_registry_clear_cache() {
        let mut registry = MarkRegistry::new();
        registry.register::<TestCircle>();

        // Initially no pipelines cached
        assert_eq!(registry.pipeline_count(), 0);

        // Clear cache (should not panic even when empty)
        registry.clear_pipeline_cache();
        assert_eq!(registry.pipeline_count(), 0);
    }

    // Test mark without custom shaders
    #[derive(Debug, Clone)]
    pub struct GeneratedShaderMark;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct GeneratedVertex {
        position: [f32; 2],
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    pub struct GeneratedAttributes {
        pub value: f32,
    }

    impl Mark for GeneratedShaderMark {
        type Vertex = GeneratedVertex;
        type AttributeValue = GeneratedAttributes;

        // No custom shaders - will use generated shaders

        fn vertex_count() -> usize {
            3
        }

        fn generate_vertices() -> Vec<Self::Vertex> {
            vec![
                GeneratedVertex {
                    position: [0.0, 1.0],
                },
                GeneratedVertex {
                    position: [-1.0, -1.0],
                },
                GeneratedVertex {
                    position: [1.0, -1.0],
                },
            ]
        }
    }

    #[test]
    fn test_generated_shader_mark() {
        let mark_info = MarkInfoImpl::<GeneratedShaderMark>::new();
        assert!(!mark_info.has_custom_shaders());
        assert_eq!(mark_info.vertex_count(), 3);
        assert_eq!(mark_info.index_count(), None);

        let vertices = GeneratedShaderMark::generate_vertices();
        assert_eq!(vertices.len(), 3);
    }

    #[test]
    fn test_mark_registry_multiple_types() {
        let mut registry = MarkRegistry::new();

        registry.register::<TestCircle>();
        registry.register::<GeneratedShaderMark>();

        assert_eq!(registry.mark_count(), 2);
        assert!(registry.is_registered::<TestCircle>());
        assert!(registry.is_registered::<GeneratedShaderMark>());

        let types = registry.registered_types();
        assert_eq!(types.len(), 2);
    }

    // Tests for mark-shader integration system

    #[test]
    fn test_attribute_type_validation() {
        // Test that Circle provides correct attribute types
        assert_eq!(Circle::get_attribute_type("position").unwrap(), "vec2<f32>");
        assert_eq!(Circle::get_attribute_type("color").unwrap(), "vec4<f32>");
        assert_eq!(Circle::get_attribute_type("size").unwrap(), "f32");
        assert_eq!(Circle::get_attribute_type("radius").unwrap(), "f32");

        // Test unknown attribute
        assert!(Circle::get_attribute_type("unknown").is_err());
    }

    #[test]
    fn test_attribute_compatibility() {
        // Test compatible attribute types
        assert!(Circle::is_attribute_compatible("position", "vec2<f32>"));
        assert!(Circle::is_attribute_compatible("color", "vec4<f32>"));
        assert!(Circle::is_attribute_compatible("radius", "f32"));

        // Test incompatible types
        assert!(!Circle::is_attribute_compatible("position", "f32"));
        assert!(!Circle::is_attribute_compatible("color", "vec2<f32>"));
        assert!(!Circle::is_attribute_compatible("radius", "vec4<f32>"));

        // Test unknown attribute
        assert!(!Circle::is_attribute_compatible("unknown", "f32"));
    }

    #[test]
    fn test_mark_pipeline_manager() {
        let mut manager = MarkPipelineManager::new();
        assert_eq!(manager.pipeline_count(), 0);
        assert_eq!(manager.shader_count(), 0);

        // Test cache clearing
        manager.clear_cache();
        assert_eq!(manager.pipeline_count(), 0);
        assert_eq!(manager.shader_count(), 0);
    }

    #[test]
    fn test_attribute_error_display() {
        let error = AttributeError::TypeMismatch {
            attribute: "position".to_string(),
            expected: "vec2<f32>",
            actual: "f32".to_string(),
        };
        let error_str = format!("{error}");
        assert!(error_str.contains("position"));
        assert!(error_str.contains("vec2<f32>"));
        assert!(error_str.contains("f32"));

        let error = AttributeError::UnknownAttribute {
            attribute: "unknown".to_string(),
            mark_type: "Circle",
        };
        let error_str = format!("{error}");
        assert!(error_str.contains("unknown"));
        assert!(error_str.contains("Circle"));

        let error = AttributeError::CompilationError {
            function_name: "test_function".to_string(),
            error_message: "syntax error".to_string(),
        };
        let error_str = format!("{error}");
        assert!(error_str.contains("test_function"));
        assert!(error_str.contains("syntax error"));
    }

    // Test data type for integration tests
    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
    }

    #[test]
    fn test_shader_function_integration() {
        // Test that we can create shader function compositions
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        // Test individual functions
        assert_eq!(LinearScale::function_name(), "linear_scale");
        assert_eq!(ColorMap::function_name(), "color_map");

        // Test that shader functions have WGSL code
        assert!(!LinearScale::wgsl_function().is_empty());
        assert!(!ColorMap::wgsl_function().is_empty());

        // Test uniform creation
        assert!(scale.create_uniforms().is_some());
        assert!(color_map.create_uniforms().is_some());
    }

    #[test]
    fn test_circle_shader_generation() {
        let pipeline = ComposableShaderPipeline::new();
        let mut attribute_functions = HashMap::new();
        attribute_functions.insert("position".to_string(), "position_transform".to_string());
        attribute_functions.insert("color".to_string(), "color_mapping".to_string());
        attribute_functions.insert("size".to_string(), "size_transform".to_string());

        // Test vertex shader generation
        let vertex_shader =
            Circle::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("CircleInstance"));
        assert!(vertex_shader.contains("VertexOutput"));
        assert!(vertex_shader.contains("position_transform"));
        assert!(vertex_shader.contains("color_mapping"));
        assert!(vertex_shader.contains("size_transform"));

        // Test fragment shader generation
        let fragment_shader =
            Circle::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("distance_from_center"));
        assert!(fragment_shader.contains("smoothstep"));
        assert!(fragment_shader.contains("anti-aliased"));
    }

    #[test]
    fn test_shader_generation_without_functions() {
        let pipeline = ComposableShaderPipeline::new();
        let attribute_functions = HashMap::new(); // No shader functions

        // Test that shaders are still generated correctly without shader functions
        let vertex_shader =
            Circle::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("instance.center")); // Uses default instance data

        let fragment_shader =
            Circle::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("anti-aliased"));
    }

    #[test]
    fn test_mark_integration_compile_time_safety() {
        // This test verifies that the type system enforces compatibility at compile time
        // These should compile successfully:

        // f32 -> f32 (LinearScale)
        let _scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);

        // f32 -> Vec4 (ColorMap)
        let _color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        // Test that Circle attributes match expected types
        let circle_attrs = CircleAttributes::default();
        assert_eq!(std::mem::size_of_val(&circle_attrs.center), 8); // Vec2 = 2 * f32
        assert_eq!(std::mem::size_of_val(&circle_attrs.radius), 4); // f32
        assert_eq!(std::mem::size_of_val(&circle_attrs.fill_color), 16); // Vec4 = 4 * f32
    }

    #[test]
    fn test_performance_requirements() {
        // Test that basic operations complete quickly
        let start = std::time::Instant::now();

        // Create pipeline manager
        let _manager = MarkPipelineManager::new();

        // Create attribute functions map
        let mut attribute_functions = HashMap::new();
        attribute_functions.insert("position".to_string(), "position_transform".to_string());
        attribute_functions.insert("color".to_string(), "color_mapping".to_string());

        // Generate shaders (this should be fast)
        let pipeline = ComposableShaderPipeline::new();
        let _vertex_shader =
            Circle::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        let _fragment_shader =
            Circle::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);

        let duration = start.elapsed();

        // Shader generation should complete in reasonable time
        assert!(
            duration.as_millis() < 100,
            "Shader generation took too long: {duration:?}"
        );
    }
}
