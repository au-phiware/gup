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

//! Integration tests for the Mark trait and mark system.
//!
//! These tests validate the complete mark system including GPU compilation,
//! render pipeline creation, and integration with the shader function system.

use gup::mark::{Circle, Line, Mark, MarkRegistry, Rectangle};
use gup::shader_pipeline::ComposableShaderPipeline;
use gup::{GupContext, GupResult};
use std::sync::Arc;

async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

#[tokio::test]
async fn test_circle_mark_shader_compilation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test that hand-optimized shaders compile successfully
    if let Some(vertex_shader_source) = Circle::VERTEX_SHADER {
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_circle_vertex"),
            source: wgpu::ShaderSource::Wgsl(vertex_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(vertex_module);
    }

    if let Some(fragment_shader_source) = Circle::FRAGMENT_SHADER {
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_circle_fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(fragment_module);
    }

    Ok(())
}

#[tokio::test]
async fn test_rectangle_mark_shader_compilation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test that hand-optimized rectangle shaders compile successfully
    if let Some(vertex_shader_source) = Rectangle::VERTEX_SHADER {
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_rectangle_vertex"),
            source: wgpu::ShaderSource::Wgsl(vertex_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(vertex_module);
    }

    if let Some(fragment_shader_source) = Rectangle::FRAGMENT_SHADER {
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_rectangle_fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(fragment_module);
    }

    Ok(())
}

#[tokio::test]
async fn test_line_mark_shader_compilation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test that hand-optimized line shaders compile successfully
    if let Some(vertex_shader_source) = Line::VERTEX_SHADER {
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_line_vertex"),
            source: wgpu::ShaderSource::Wgsl(vertex_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(vertex_module);
    }

    if let Some(fragment_shader_source) = Line::FRAGMENT_SHADER {
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_line_fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_shader_source.into()),
        });
        // If this doesn't panic, the shader compiled successfully
        drop(fragment_module);
    }

    Ok(())
}

#[tokio::test]
async fn test_circle_mark_generated_shaders() -> GupResult<()> {
    let _context = create_test_context().await?;
    let pipeline = ComposableShaderPipeline::new();

    // Test generated shader code creation
    let vertex_shader = Circle::generate_vertex_shader(&pipeline);
    let fragment_shader = Circle::generate_fragment_shader(&pipeline);

    // Verify shaders contain expected content
    assert!(vertex_shader.contains("vs_main"));
    assert!(vertex_shader.contains("CircleInstance"));
    assert!(fragment_shader.contains("fs_main"));
    assert!(fragment_shader.contains("distance_from_center"));

    Ok(())
}

#[tokio::test]
async fn test_rectangle_mark_generated_shaders() -> GupResult<()> {
    let _context = create_test_context().await?;
    let pipeline = ComposableShaderPipeline::new();

    // Test generated shader code creation
    let vertex_shader = Rectangle::generate_vertex_shader(&pipeline);
    let fragment_shader = Rectangle::generate_fragment_shader(&pipeline);

    // Verify shaders contain expected content
    assert!(vertex_shader.contains("vs_main"));
    assert!(vertex_shader.contains("RectangleInstance"));
    assert!(fragment_shader.contains("fs_main"));
    assert!(fragment_shader.contains("distance_to_edge"));

    Ok(())
}

#[tokio::test]
async fn test_line_mark_generated_shaders() -> GupResult<()> {
    let _context = create_test_context().await?;
    let pipeline = ComposableShaderPipeline::new();

    // Test generated shader code creation
    let vertex_shader = Line::generate_vertex_shader(&pipeline);
    let fragment_shader = Line::generate_fragment_shader(&pipeline);

    // Verify shaders contain expected content
    assert!(vertex_shader.contains("vs_main"));
    assert!(vertex_shader.contains("LineInstance"));
    assert!(fragment_shader.contains("fs_main"));
    assert!(fragment_shader.contains("style"));

    Ok(())
}

#[tokio::test]
async fn test_mark_registry_integration() -> GupResult<()> {
    let context = create_test_context().await?;
    let _device = &context.device;
    let mut registry = MarkRegistry::new();

    // Register all mark types
    registry.register::<Circle>();
    registry.register::<Rectangle>();
    registry.register::<Line>();
    assert!(registry.is_registered::<Circle>());
    assert!(registry.is_registered::<Rectangle>());
    assert!(registry.is_registered::<Line>());

    // Verify mark info is available for all marks
    let circle_info = registry.get_mark_info::<Circle>().unwrap();
    assert_eq!(circle_info.vertex_count(), 4);
    assert_eq!(circle_info.index_count(), Some(6));
    assert!(circle_info.has_custom_shaders());

    let rectangle_info = registry.get_mark_info::<Rectangle>().unwrap();
    assert_eq!(rectangle_info.vertex_count(), 4);
    assert_eq!(rectangle_info.index_count(), Some(6));
    assert!(rectangle_info.has_custom_shaders());

    let line_info = registry.get_mark_info::<Line>().unwrap();
    assert_eq!(line_info.vertex_count(), 4);
    assert_eq!(line_info.index_count(), Some(6));
    assert!(line_info.has_custom_shaders());

    // Test vertex generation for Circle
    let vertices_bytes = circle_info.generate_vertices_boxed();
    assert!(!vertices_bytes.is_empty());

    let indices = circle_info.generate_indices_boxed().unwrap();
    assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);

    // Note: Pipeline creation is not yet implemented, so we skip that test
    // let pipeline = registry.get_pipeline::<Circle>(device)?;

    Ok(())
}

#[tokio::test]
async fn test_circle_vertex_buffer_gpu_compatibility() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Generate vertices
    let vertices = Circle::generate_vertices();
    let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);

    // Create a GPU buffer and upload the vertex data
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_circle_vertices"),
        size: vertex_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context.queue.write_buffer(&buffer, 0, vertex_data);

    // If we reach here without panicking, the vertex data is GPU-compatible
    Ok(())
}

#[tokio::test]
async fn test_rectangle_vertex_buffer_gpu_compatibility() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Generate vertices
    let vertices = Rectangle::generate_vertices();
    let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);

    // Create a GPU buffer and upload the vertex data
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_rectangle_vertices"),
        size: vertex_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context.queue.write_buffer(&buffer, 0, vertex_data);

    // If we reach here without panicking, the vertex data is GPU-compatible
    Ok(())
}

#[tokio::test]
async fn test_line_vertex_buffer_gpu_compatibility() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Generate vertices
    let vertices = Line::generate_vertices();
    let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);

    // Create a GPU buffer and upload the vertex data
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_line_vertices"),
        size: vertex_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context.queue.write_buffer(&buffer, 0, vertex_data);

    // If we reach here without panicking, the vertex data is GPU-compatible
    Ok(())
}

#[tokio::test]
async fn test_mark_performance_targets() -> GupResult<()> {
    let _context = create_test_context().await?;

    // Test vertex generation performance for all marks (should be very fast)
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _circle_vertices = Circle::generate_vertices();
        let _rectangle_vertices = Rectangle::generate_vertices();
        let _line_vertices = Line::generate_vertices();
    }
    let duration = start.elapsed();

    // Should generate 1000 vertex sets for all marks in < 1ms
    assert!(
        duration.as_millis() < 1,
        "Vertex generation took {duration:?}"
    );

    // Test mark registry operations performance
    let mut registry = MarkRegistry::new();

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        registry.register::<Circle>();
        registry.register::<Rectangle>();
        registry.register::<Line>();
        assert!(registry.is_registered::<Circle>());
        assert!(registry.is_registered::<Rectangle>());
        assert!(registry.is_registered::<Line>());
    }
    let duration = start.elapsed();

    // Registry operations should be very fast
    assert!(
        duration.as_millis() < 10,
        "Registry operations took {duration:?}"
    );

    Ok(())
}

#[tokio::test]
async fn test_custom_mark_implementation() -> GupResult<()> {
    use gup::mark::Mark;

    // Define a custom triangle mark for testing
    #[derive(Debug, Clone)]
    struct Triangle;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct TriangleVertex {
        position: [f32; 2],
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TriangleAttributes {
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    }

    impl Mark for Triangle {
        type Vertex = TriangleVertex;
        type AttributeValue = TriangleAttributes;

        fn vertex_count() -> usize {
            3
        }

        fn generate_vertices() -> Vec<Self::Vertex> {
            vec![
                TriangleVertex {
                    position: [0.0, 1.0],
                }, // Top
                TriangleVertex {
                    position: [-1.0, -1.0],
                }, // Bottom-left
                TriangleVertex {
                    position: [1.0, -1.0],
                }, // Bottom-right
            ]
        }
    }

    // Test the custom mark
    let vertices = Triangle::generate_vertices();
    assert_eq!(vertices.len(), 3);
    assert_eq!(Triangle::vertex_count(), 3);
    assert_eq!(Triangle::index_count(), None);

    // Test with registry
    let mut registry = MarkRegistry::new();
    registry.register::<Triangle>();
    assert!(registry.is_registered::<Triangle>());

    let mark_info = registry.get_mark_info::<Triangle>().unwrap();
    assert_eq!(mark_info.vertex_count(), 3);
    assert!(!mark_info.has_custom_shaders()); // No custom shaders defined

    Ok(())
}

#[tokio::test]
async fn test_mark_system_memory_usage() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test memory efficiency of mark system
    let vertices = Circle::generate_vertices();
    let vertex_size = std::mem::size_of_val(&vertices[0]);
    let total_vertex_memory = vertices.len() * vertex_size;

    // Circle vertices should be compact (2 floats per vertex)
    assert_eq!(vertex_size, 8); // 2 * 4 bytes
    assert_eq!(total_vertex_memory, 32); // 4 vertices * 8 bytes

    // Test index memory usage
    let indices = Circle::generate_indices().unwrap();
    let index_memory = indices.len() * std::mem::size_of::<u32>();
    assert_eq!(index_memory, 24); // 6 indices * 4 bytes

    // Test GPU buffer creation doesn't fail
    let vertex_data: &[u8] = bytemuck::cast_slice(&vertices);
    let _vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("memory_test_vertices"),
        size: vertex_data.len() as u64,
        usage: wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    });

    let index_data: &[u8] = bytemuck::cast_slice(&indices);
    let _index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("memory_test_indices"),
        size: index_data.len() as u64,
        usage: wgpu::BufferUsages::INDEX,
        mapped_at_creation: false,
    });

    Ok(())
}

#[tokio::test]
async fn test_mark_trait_extensibility() -> GupResult<()> {
    use gup::mark::Mark;

    // Test that marks can have different vertex structures
    #[derive(Debug, Clone)]
    struct ComplexMark;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct ComplexVertex {
        position: [f32; 3], // 3D position
        normal: [f32; 3],   // Normal vector
        uv: [f32; 2],       // Texture coordinates
        color: [f32; 4],    // Vertex color
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct ComplexAttributes {
        transform: [f32; 16], // 4x4 matrix
        material_id: u32,
        lighting_factor: f32,
    }

    impl Mark for ComplexMark {
        type Vertex = ComplexVertex;
        type AttributeValue = ComplexAttributes;

        fn vertex_count() -> usize {
            8 // Cube vertices
        }

        fn index_count() -> Option<usize> {
            Some(36) // 12 triangles * 3 indices
        }

        fn generate_vertices() -> Vec<Self::Vertex> {
            // Generate a simple cube
            vec![
                ComplexVertex {
                    position: [-1.0, -1.0, -1.0],
                    normal: [0.0, 0.0, -1.0],
                    uv: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [1.0, -1.0, -1.0],
                    normal: [0.0, 0.0, -1.0],
                    uv: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [1.0, 1.0, -1.0],
                    normal: [0.0, 0.0, -1.0],
                    uv: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [-1.0, 1.0, -1.0],
                    normal: [0.0, 0.0, -1.0],
                    uv: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [-1.0, -1.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [1.0, -1.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [1.0, 1.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [1.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                ComplexVertex {
                    position: [-1.0, 1.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 1.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                },
            ]
        }

        fn generate_indices() -> Option<Vec<u32>> {
            Some(vec![
                // Front face
                0, 1, 2, 0, 2, 3, // Back face
                4, 6, 5, 4, 7, 6, // Left face
                4, 0, 3, 4, 3, 7, // Right face
                1, 5, 6, 1, 6, 2, // Top face
                3, 2, 6, 3, 6, 7, // Bottom face
                4, 1, 0, 4, 5, 1,
            ])
        }
    }

    // Test the complex mark
    let vertices = ComplexMark::generate_vertices();
    assert_eq!(vertices.len(), 8);
    assert_eq!(ComplexMark::vertex_count(), 8);

    let indices = ComplexMark::generate_indices().unwrap();
    assert_eq!(indices.len(), 36);
    assert_eq!(ComplexMark::index_count(), Some(36));

    // Verify GPU compatibility
    let _vertex_bytes: &[u8] = bytemuck::cast_slice(&vertices);
    let _index_bytes: &[u8] = bytemuck::cast_slice(&indices);

    Ok(())
}

#[test]
fn test_mark_type_id_constants() {
    // Validate that mark type IDs match the expected GPU shader values
    // These must stay stable and match the hit_test.compute.wgsl shader

    // Test that the constants are accessible
    assert_eq!(
        Circle::MARK_TYPE_ID,
        0,
        "Circle must have ID 0 to match GPU shader"
    );
    assert_eq!(
        Rectangle::MARK_TYPE_ID,
        1,
        "Rectangle must have ID 1 to match GPU shader"
    );
    assert_eq!(
        Line::MARK_TYPE_ID,
        2,
        "Line must have ID 2 to match GPU shader"
    );

    // Test that IDs are in valid range (0-255 for u8 compatibility)
    #[allow(clippy::assertions_on_constants, clippy::absurd_extreme_comparisons)]
    {
        assert!(Circle::MARK_TYPE_ID <= 255, "Mark type IDs must fit in u8");
        assert!(
            Rectangle::MARK_TYPE_ID <= 255,
            "Mark type IDs must fit in u8"
        );
        assert!(Line::MARK_TYPE_ID <= 255, "Mark type IDs must fit in u8");
    }

    // Test that IDs are unique
    let mut ids = vec![
        Circle::MARK_TYPE_ID,
        Rectangle::MARK_TYPE_ID,
        Line::MARK_TYPE_ID,
    ];
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3, "Mark type IDs must be unique");
}
