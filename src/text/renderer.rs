// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU text renderer using SDF fonts.

use super::*;
use crate::error::{GupError, GupResult};
// RenderContext import removed as we now use RenderFrame
use bytemuck::{Pod, Zeroable};
use std::mem;
use wgpu::util::DeviceExt;
use wgpu::*;

/// GPU text renderer for SDF fonts.
pub struct TextRenderer {
    /// Render pipeline for SDF text
    render_pipeline: RenderPipeline,
    /// Bind group layout for text resources
    bind_group_layout: BindGroupLayout,
    /// Vertex buffer for glyph quads
    vertex_buffer: Buffer,
    /// Index buffer for glyph quads
    index_buffer: Buffer,
    /// Uniform buffer for projection matrix
    uniform_buffer: Buffer,
    /// Current vertex buffer capacity
    vertex_capacity: usize,
    /// Queued text vertices awaiting rendering
    render_queue: Vec<TextVertex>,
    /// Sampler for font texture
    sampler: Sampler,
}

/// Vertex data for text rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TextVertex {
    /// Position in screen space
    position: [f32; 2],
    /// Texture coordinates
    tex_coords: [f32; 2],
    /// Text color
    color: [f32; 4],
    /// SDF parameters (scale, edge, outline)
    sdf_params: [f32; 4],
}

/// Uniform data for text rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TextUniforms {
    /// Projection matrix
    projection: [[f32; 4]; 4],
    /// Screen dimensions
    screen_size: [f32; 2],
    /// Padding
    _padding: [f32; 2],
}

/// Configuration for text rendering
pub struct TextRenderConfig<'a> {
    pub text: &'a str,
    pub position: Vec2,
    pub style: &'a TextStyle,
    pub font_atlas: &'a mut FontAtlas,
    pub layout_engine: &'a mut TextLayoutEngine,
    pub screen_width: f32,
    pub screen_height: f32,
    /// Optional viewport bounds for clipping detection.
    /// When `None`, no clipping is applied (backward compatible).
    pub viewport_bounds: Option<&'a ViewportBounds>,
    /// Clipping strategy configuration. Only used when `viewport_bounds` is `Some`.
    pub clipping_config: Option<&'a ClippingStrategyConfig>,
}

impl TextRenderer {
    /// Create a new text renderer.
    pub fn new(device: &Device) -> GupResult<Self> {
        // Create shader module
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                // Binding 0: Uniform buffer (projection matrix + screen size)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Binding 1: Font atlas texture (SDF data)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 2: Texture sampler (linear filtering)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[VertexBufferLayout {
                    array_stride: mem::size_of::<TextVertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            offset: mem::size_of::<[f32; 2]>() as BufferAddress,
                            shader_location: 1,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            offset: mem::size_of::<[f32; 4]>() as BufferAddress,
                            shader_location: 2,
                            format: VertexFormat::Float32x4,
                        },
                        VertexAttribute {
                            offset: (mem::size_of::<[f32; 2]>()
                                + mem::size_of::<[f32; 2]>()
                                + mem::size_of::<[f32; 4]>())
                                as BufferAddress,
                            shader_location: 3,
                            format: VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb, // Assumed default format
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
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
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create initial buffers
        let vertex_capacity = 1024; // Start with space for 1024 vertices
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: (vertex_capacity * mem::size_of::<TextVertex>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create index buffer for quads (6 indices per quad)
        let indices: Vec<u16> = (0..vertex_capacity as u16)
            .step_by(4)
            .flat_map(|i| [i, i + 1, i + 2, i, i + 2, i + 3])
            .collect();

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Text Uniform Buffer"),
            size: mem::size_of::<TextUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Text Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            render_pipeline,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            vertex_capacity,
            render_queue: Vec::new(),
            sampler,
        })
    }

    /// Reset buffer state for a new frame. Call this before rendering any text in a frame.
    pub fn begin_frame(&mut self) {
        self.render_queue.clear();
    }

    /// Render all queued text with a single draw call.
    pub fn render_queued_text<'a>(
        &mut self,
        render_pass: &mut RenderPass<'a>,
        device: &Device,
        queue: &Queue,
        font_atlas: &FontAtlas,
        screen_width: f32,
        screen_height: f32,
    ) -> GupResult<()> {
        if self.render_queue.is_empty() {
            return Ok(());
        }

        // Ensure vertex buffer capacity and upload all vertices
        let required_capacity = self.render_queue.len();
        if required_capacity > self.vertex_capacity {
            let new_capacity = required_capacity.max(self.vertex_capacity * 2);
            let capped_capacity = new_capacity.min(65535);
            if required_capacity > capped_capacity {
                return Err(GupError::render_error("Too many text vertices"));
            }
            self.resize_buffers(device, capped_capacity)?;
        }

        // Upload ALL queued vertices at once
        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.render_queue),
        );

        // Update uniform buffer
        let projection = self.create_projection_matrix(screen_width, screen_height);
        let uniforms = TextUniforms {
            projection,
            screen_size: [screen_width, screen_height],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create bind group
        let font_texture_view = font_atlas
            .texture()
            .create_view(&TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&font_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // Set up render state
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // SINGLE draw call for ALL queued text!
        let total_indices = (self.render_queue.len() / 4) * 6; // 6 indices per quad (4 vertices per quad)
        render_pass.draw_indexed(0..total_indices as u32, 0, 0..1);

        Ok(())
    }

    /// Queue text for batched rendering. This handles glyph loading, layout, and accumulation.
    /// Must be called before creating the render pass.
    pub fn queue_text(
        &mut self,
        frame: &crate::RenderFrame,
        config: &mut TextRenderConfig,
    ) -> GupResult<TextBounds> {
        // Ensure all glyphs are available in the atlas
        for ch in config.text.chars() {
            config.font_atlas.ensure_glyph(
                frame.device(),
                frame.queue(),
                ch,
                config.style.font_size,
            )?;
        }

        // Layout the text — with or without clipping
        let layout_result = if let Some(viewport_bounds) = config.viewport_bounds {
            let clipping_config = config.clipping_config.cloned().unwrap_or_default();
            config.layout_engine.layout_text_with_clipping(
                config.text,
                config.position,
                config.style,
                config.font_atlas,
                None,
                viewport_bounds,
                &clipping_config,
            )?
        } else {
            config.layout_engine.layout_text(
                config.text,
                config.position,
                config.style,
                config.font_atlas,
                None,
            )?
        };

        // Add to internal batches for later rendering
        self.add_glyph_batch(&layout_result.glyphs)?;

        Ok(layout_result.bounds)
    }

    /// Add a glyph batch to the internal rendering queue.
    /// This is a low-level method for building batched text rendering.
    fn add_glyph_batch(&mut self, glyphs: &GlyphBatch) -> GupResult<()> {
        if glyphs.is_empty() {
            return Ok(());
        }

        let vertices = self.create_vertices(glyphs);
        if !vertices.is_empty() {
            self.render_queue.extend_from_slice(&vertices);
        }

        Ok(())
    }

    /// Convenience method for immediate text rendering within an existing render pass.
    /// This handles glyph preparation, layout, and rendering in one call.
    /// For better performance with multiple texts, use queue_text() + render_queued_text().
    pub fn render_text<'a>(
        &mut self,
        render_pass: &mut RenderPass<'a>,
        device: &Device,
        queue: &Queue,
        config: TextRenderConfig,
    ) -> GupResult<TextBounds> {
        // Prepare the text (this may upload to GPU atlas)
        for ch in config.text.chars() {
            config
                .font_atlas
                .ensure_glyph(device, queue, ch, config.style.font_size)?;
        }

        // Layout the text — with or without clipping
        let layout_result = if let Some(viewport_bounds) = config.viewport_bounds {
            let clipping_config = config.clipping_config.cloned().unwrap_or_default();
            config.layout_engine.layout_text_with_clipping(
                config.text,
                config.position,
                config.style,
                config.font_atlas,
                None,
                viewport_bounds,
                &clipping_config,
            )?
        } else {
            config.layout_engine.layout_text(
                config.text,
                config.position,
                config.style,
                config.font_atlas,
                None,
            )?
        };

        if layout_result.glyphs.is_empty() {
            return Ok(layout_result.bounds);
        }

        // Create vertices for immediate rendering
        let vertices = self.create_vertices(&layout_result.glyphs);
        if vertices.is_empty() {
            return Ok(layout_result.bounds);
        }

        // Ensure vertex buffer capacity
        if vertices.len() > self.vertex_capacity {
            let new_capacity = vertices.len().max(self.vertex_capacity * 2);
            let capped_capacity = new_capacity.min(65535);
            if vertices.len() > capped_capacity {
                return Err(GupError::render_error("Too many text vertices"));
            }
            self.resize_buffers(device, capped_capacity)?;
        }

        // Upload vertices for immediate rendering
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // Update uniform buffer
        let projection = self.create_projection_matrix(config.screen_width, config.screen_height);
        let uniforms = TextUniforms {
            projection,
            screen_size: [config.screen_width, config.screen_height],
            _padding: [0.0, 0.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create bind group
        let font_texture_view = config
            .font_atlas
            .texture()
            .create_view(&TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&font_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // Set up render state and draw
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Single draw call for this text
        let total_indices = (vertices.len() / 4) * 6; // 6 indices per quad (4 vertices per quad)
        render_pass.draw_indexed(0..total_indices as u32, 0, 0..1);

        Ok(layout_result.bounds)
    }

    /// Create vertices for glyph batch.
    fn create_vertices(&self, glyphs: &GlyphBatch) -> Vec<TextVertex> {
        let mut vertices = Vec::with_capacity(glyphs.len() * 4);

        for glyph in glyphs {
            let left = glyph.position.x;
            let right = glyph.position.x + glyph.glyph.size.x;
            let top = glyph.position.y;
            let bottom = glyph.position.y + glyph.glyph.size.y;

            let uv = glyph.glyph.atlas_pos;

            let sdf_params = [
                glyph.glyph.sdf_scale,
                0.0, // SDF edge threshold (0.0 for normal rendering at distance field edge)
                0.0, // Channel combination mode (0=median, 1=max, 2=min)
                0.0, // Debug mode (0=normal, 1=quad outlines, 2/3/4=R/G/B channel, 5=median)
            ];

            // Create quad vertices (2 triangles)
            vertices.extend_from_slice(&[
                // Top-left
                TextVertex {
                    position: [left, top],
                    tex_coords: [uv[0], uv[1]],
                    color: [glyph.color.x, glyph.color.y, glyph.color.z, glyph.color.w],
                    sdf_params,
                },
                // Top-right
                TextVertex {
                    position: [right, top],
                    tex_coords: [uv[2], uv[1]],
                    color: [glyph.color.x, glyph.color.y, glyph.color.z, glyph.color.w],
                    sdf_params,
                },
                // Bottom-right
                TextVertex {
                    position: [right, bottom],
                    tex_coords: [uv[2], uv[3]],
                    color: [glyph.color.x, glyph.color.y, glyph.color.z, glyph.color.w],
                    sdf_params,
                },
                // Bottom-left
                TextVertex {
                    position: [left, bottom],
                    tex_coords: [uv[0], uv[3]],
                    color: [glyph.color.x, glyph.color.y, glyph.color.z, glyph.color.w],
                    sdf_params,
                },
            ]);
        }

        vertices
    }

    /// Create projection matrix for screen coordinates.
    fn create_projection_matrix(&self, width: f32, height: f32) -> [[f32; 4]; 4] {
        // Orthographic projection matrix for screen coordinates
        // Maps [0, width] to [-1, 1] for X and [0, height] to [1, -1] for Y
        [
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ]
    }

    /// Resize buffers if needed.
    fn resize_buffers(&mut self, device: &Device, required_vertices: usize) -> GupResult<()> {
        let new_capacity = (required_vertices * 2).max(self.vertex_capacity * 2);

        // Cap the capacity to stay within u16 index limits (65535 vertices max)
        let new_capacity = new_capacity.min(65535);

        // Create new vertex buffer
        self.vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: (new_capacity * mem::size_of::<TextVertex>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create new index buffer
        let indices: Vec<u16> = (0..new_capacity as u16)
            .step_by(4)
            .flat_map(|i| [i, i + 1, i + 2, i, i + 2, i + 3])
            .collect();

        self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: BufferUsages::INDEX,
        });

        self.vertex_capacity = new_capacity;
        Ok(())
    }
}

impl std::fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRenderer")
            .field("vertex_capacity", &self.vertex_capacity)
            .field("queued_vertices", &self.render_queue.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_vertex_layout() {
        // Ensure TextVertex has the expected size and alignment
        assert_eq!(mem::size_of::<TextVertex>(), 48); // 2*4 + 2*4 + 4*4 + 4*4 = 48 bytes
        assert_eq!(mem::align_of::<TextVertex>(), 4);
    }

    #[test]
    fn test_text_uniforms_layout() {
        // Ensure TextUniforms has the expected size and alignment
        assert_eq!(mem::size_of::<TextUniforms>(), 80); // 16*4 + 2*4 + 2*4 = 80 bytes
        assert_eq!(mem::align_of::<TextUniforms>(), 4);
    }

    #[test]
    #[ignore] // Disabled due to unsafe mem::zeroed() usage in test mock
    #[allow(invalid_value)]
    fn test_projection_matrix_creation() {
        #[allow(invalid_value)]
        let renderer = TextRenderer {
            render_pipeline: unsafe { mem::zeroed() },
            bind_group_layout: unsafe { mem::zeroed() },
            vertex_buffer: unsafe { mem::zeroed() },
            index_buffer: unsafe { mem::zeroed() },
            uniform_buffer: unsafe { mem::zeroed() },
            vertex_capacity: 1024,
            render_queue: Vec::new(),
            sampler: unsafe { mem::zeroed() },
        };

        let matrix = renderer.create_projection_matrix(800.0, 600.0);

        // Check that the matrix transforms screen coordinates correctly
        assert!((matrix[0][0] - 2.0 / 800.0).abs() < 0.001);
        assert!((matrix[1][1] - (-2.0 / 600.0)).abs() < 0.001);
        assert_eq!(matrix[0][3], -1.0);
        assert_eq!(matrix[1][3], 1.0);
    }

    #[test]
    #[ignore] // Disabled due to unsafe mem::zeroed() usage in test mock
    #[allow(invalid_value)]
    fn test_vertex_creation_basic() {
        #[allow(invalid_value)]
        let renderer = TextRenderer {
            render_pipeline: unsafe { mem::zeroed() },
            bind_group_layout: unsafe { mem::zeroed() },
            vertex_buffer: unsafe { mem::zeroed() },
            index_buffer: unsafe { mem::zeroed() },
            uniform_buffer: unsafe { mem::zeroed() },
            vertex_capacity: 1024,
            render_queue: Vec::new(),
            sampler: unsafe { mem::zeroed() },
        };

        let glyph = PositionedGlyph {
            glyph: GlyphInfo {
                character: 'A',
                atlas_pos: [0.0, 0.0, 0.1, 0.1],
                size: Vec2 { x: 10.0, y: 12.0 },
                bearing: Vec2 { x: 0.0, y: 0.0 },
                advance: 8.0,
                sdf_scale: 1.0,
            },
            position: Vec2 { x: 100.0, y: 200.0 },
            color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        let vertices = renderer.create_vertices(&vec![glyph]);

        // Should create 4 vertices per glyph
        assert_eq!(vertices.len(), 4);

        // Check first vertex
        assert_eq!(vertices[0].position, [100.0, 200.0]);
        assert_eq!(vertices[0].tex_coords, [0.0, 0.0]);
        assert_eq!(vertices[0].color, [1.0, 0.0, 0.0, 1.0]);

        // Check last vertex (bottom-left)
        assert_eq!(vertices[3].position, [100.0, 212.0]); // 200 + 12
        assert_eq!(vertices[3].tex_coords, [0.0, 0.1]);
    }

    #[test]
    #[ignore] // Disabled due to unsafe mem::zeroed() usage with wgpu types
    fn test_vertex_creation_performance() {
        // Test vertex creation performance with large glyph batches
        use std::time::Instant;

        #[allow(invalid_value)]
        let renderer = TextRenderer {
            render_pipeline: unsafe { mem::zeroed() },
            bind_group_layout: unsafe { mem::zeroed() },
            vertex_buffer: unsafe { mem::zeroed() },
            index_buffer: unsafe { mem::zeroed() },
            uniform_buffer: unsafe { mem::zeroed() },
            vertex_capacity: 1024,
            render_queue: Vec::new(),
            sampler: unsafe { mem::zeroed() },
        };

        // Create a large batch of glyphs
        let glyph_count = 1000;
        let glyphs: Vec<PositionedGlyph> = (0..glyph_count)
            .map(|i| PositionedGlyph {
                glyph: GlyphInfo {
                    character: char::from_u32(65 + (i % 26) as u32).unwrap_or('A'),
                    atlas_pos: [0.0, 0.0, 0.1, 0.1],
                    size: Vec2 { x: 10.0, y: 12.0 },
                    bearing: Vec2 { x: 0.0, y: 0.0 },
                    advance: 8.0,
                    sdf_scale: 1.0,
                },
                position: Vec2 {
                    x: (i % 100) as f32 * 10.0,
                    y: (i / 100) as f32 * 15.0,
                },
                color: Vec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
            })
            .collect();

        let start = Instant::now();
        let vertices = renderer.create_vertices(&glyphs);
        let duration = start.elapsed();

        // Should create 4 vertices per glyph
        assert_eq!(vertices.len(), glyph_count * 4);

        // Performance: vertex creation should be fast.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 25;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 5;

        println!("Vertex creation for {glyph_count} glyphs took: {duration:?}");
        assert!(
            duration.as_millis() < threshold_ms,
            "Vertex creation too slow: {duration:?} (threshold: {threshold_ms}ms)"
        );
    }

    #[test]
    fn test_memory_usage_efficiency() {
        // Test that TextVertex uses memory efficiently
        let vertex_size = mem::size_of::<TextVertex>();
        let expected_minimum = 44; // Theoretical minimum: 2*4 + 2*4 + 4*4 + 4*4 = 44 bytes
        let expected_maximum = 64; // Allow for some padding

        assert!(
            vertex_size >= expected_minimum,
            "TextVertex too small: {vertex_size} bytes"
        );
        assert!(
            vertex_size <= expected_maximum,
            "TextVertex too large: {vertex_size} bytes"
        );

        // Test alignment
        assert_eq!(
            mem::align_of::<TextVertex>(),
            4,
            "TextVertex alignment incorrect"
        );
    }
}
