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

pub mod circle;
pub mod line;
pub mod rectangle;

pub use circle::{Circle, CircleAttributes, CircleVertex};
pub use line::{Line, LineAttributes, LineStyle, LineVertex};
pub use rectangle::{Rectangle, RectangleAttributes, RectangleVertex};

use crate::error::GupResult;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::any::TypeId;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::{Device, RenderPipeline};

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

    /// Generate vertex shader WGSL code for dynamic attribute mapping
    ///
    /// Default implementation delegates to the shader pipeline for generated shaders.
    /// Override this method for mark-specific optimizations.
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        pipeline.generate_vertex_shader()
    }

    /// Generate fragment shader WGSL code for dynamic attribute mapping
    ///
    /// Default implementation delegates to the shader pipeline for generated shaders.
    /// Override this method for mark-specific optimizations.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        pipeline.generate_fragment_shader()
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

    /// Create a render pipeline for this mark type
    fn create_render_pipeline(&self, device: &Device) -> GupResult<RenderPipeline>;

    /// Get the vertex count for this mark type
    fn vertex_count(&self) -> usize;

    /// Get the index count for this mark type (None for non-indexed)
    fn index_count(&self) -> Option<usize>;

    /// Generate vertices for this mark type
    fn generate_vertices_boxed(&self) -> Vec<u8>;

    /// Generate indices for this mark type (None for non-indexed)
    fn generate_indices_boxed(&self) -> Option<Vec<u32>>;
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

    fn create_render_pipeline(&self, _device: &Device) -> GupResult<RenderPipeline> {
        // TODO: Implement render pipeline creation
        // This would use either custom shaders or generate them via the pipeline system
        todo!("Render pipeline creation will be implemented in render integration task")
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
            crate::error::GupError::RenderError(format!(
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
}

impl Default for MarkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
