// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-rendered tooltip background with rounded corners, border, and optional
//! drop shadow.
//!
//! The renderer draws a single instanced quad per tooltip using an SDF-based
//! fragment shader that produces smooth, anti-aliased rounded rectangles.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::text::tooltip_bg::TooltipBackgroundRenderer;
//! use gup::text::hover_reveal::{TooltipConfig, TooltipLayout};
//!
//! // Create once
//! let mut bg_renderer = TooltipBackgroundRenderer::new(&device)?;
//!
//! // Each frame
//! bg_renderer.begin_frame();
//! bg_renderer.queue(&layout, &config);
//!
//! // Inside the render pass — call BEFORE text rendering
//! bg_renderer.render(&mut render_pass, &queue, screen_w, screen_h)?;
//! ```

use super::hover_reveal::{TooltipConfig, TooltipLayout};
use crate::error::GupResult;
use bytemuck::{Pod, Zeroable};
use std::mem;
use wgpu::util::DeviceExt;
use wgpu::*;

// ── GPU data types ──────────────────────────────────────────────────────────

/// Per-instance data uploaded to the GPU for each tooltip background.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TooltipBgInstance {
    /// Top-left corner of the rectangle (screen-space pixels).
    rect_min: [f32; 2],
    /// Bottom-right corner of the rectangle (screen-space pixels).
    rect_max: [f32; 2],
    /// Background fill colour (RGBA).
    bg_color: [f32; 4],
    /// Border colour (RGBA).
    border_color: [f32; 4],
    /// Packed parameters: (corner_radius, border_width, opacity, shadow_radius).
    params: [f32; 4],
    /// Shadow colour (RGBA).
    shadow_color: [f32; 4],
    /// Shadow offset (x, y).
    shadow_offset: [f32; 2],
    /// Arrow parameters: (direction, size, offset_along_edge, 0).
    ///
    /// direction: 0=none, 1=top, 2=bottom, 3=left, 4=right.
    /// size: triangle height in pixels.
    /// offset_along_edge: arrow centre relative to rect centre.
    arrow_params: [f32; 4],
}

/// Uniform data shared by all tooltip backgrounds in a frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct TooltipBgUniforms {
    /// Orthographic projection matrix.
    projection: [[f32; 4]; 4],
    /// Viewport dimensions.
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

// ── Renderer ────────────────────────────────────────────────────────────────

/// GPU-accelerated tooltip background renderer.
///
/// Uses instanced rendering with a rounded-rectangle SDF shader to draw
/// tooltip backgrounds with configurable fill, border, corner radius, and
/// optional drop shadow.
pub struct TooltipBackgroundRenderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    instance_buffer: Buffer,
    uniform_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    instance_capacity: usize,
    queued_instances: Vec<TooltipBgInstance>,
}

impl std::fmt::Debug for TooltipBackgroundRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TooltipBackgroundRenderer")
            .field("instance_capacity", &self.instance_capacity)
            .field("queued_instances", &self.queued_instances.len())
            .finish()
    }
}

impl TooltipBackgroundRenderer {
    /// Create a new tooltip background renderer.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tooltip Background Shader"),
            source: ShaderSource::Wgsl(include_str!("../shaders/tooltip_bg.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Tooltip BG Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tooltip BG Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tooltip BG Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[
                    // Slot 0: unit-quad vertex (position only)
                    VertexBufferLayout {
                        array_stride: mem::size_of::<[f32; 2]>() as BufferAddress,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        }],
                    },
                    // Slot 1: per-instance data
                    VertexBufferLayout {
                        array_stride: mem::size_of::<TooltipBgInstance>() as BufferAddress,
                        step_mode: VertexStepMode::Instance,
                        attributes: &[
                            // rect_min
                            VertexAttribute {
                                offset: 0,
                                shader_location: 1,
                                format: VertexFormat::Float32x2,
                            },
                            // rect_max
                            VertexAttribute {
                                offset: 8,
                                shader_location: 2,
                                format: VertexFormat::Float32x2,
                            },
                            // bg_color
                            VertexAttribute {
                                offset: 16,
                                shader_location: 3,
                                format: VertexFormat::Float32x4,
                            },
                            // border_color
                            VertexAttribute {
                                offset: 32,
                                shader_location: 4,
                                format: VertexFormat::Float32x4,
                            },
                            // params
                            VertexAttribute {
                                offset: 48,
                                shader_location: 5,
                                format: VertexFormat::Float32x4,
                            },
                            // shadow_color
                            VertexAttribute {
                                offset: 64,
                                shader_location: 6,
                                format: VertexFormat::Float32x4,
                            },
                            // shadow_offset
                            VertexAttribute {
                                offset: 80,
                                shader_location: 7,
                                format: VertexFormat::Float32x2,
                            },
                            // arrow_params
                            VertexAttribute {
                                offset: 88,
                                shader_location: 8,
                                format: VertexFormat::Float32x4,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
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

        // Unit quad: 4 vertices forming a [0,1]×[0,1] square as a triangle
        // strip.
        let quad_vertices: [[f32; 2]; 4] = [
            [0.0, 0.0], // top-left
            [1.0, 0.0], // top-right
            [0.0, 1.0], // bottom-left
            [1.0, 1.0], // bottom-right
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tooltip BG Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: BufferUsages::VERTEX,
        });

        let instance_capacity = 8;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tooltip BG Instance Buffer"),
            size: (instance_capacity * mem::size_of::<TooltipBgInstance>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Tooltip BG Uniform Buffer"),
            size: mem::size_of::<TooltipBgUniforms>() as BufferAddress,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            render_pipeline,
            vertex_buffer,
            instance_buffer,
            uniform_buffer,
            bind_group_layout,
            instance_capacity,
            queued_instances: Vec::new(),
        })
    }

    /// Clear queued instances for a new frame.
    pub fn begin_frame(&mut self) {
        self.queued_instances.clear();
    }

    /// Queue a tooltip background for rendering.
    ///
    /// Call this after computing [`TooltipLayout`] and before creating the
    /// render pass.
    pub fn queue(&mut self, layout: &TooltipLayout, config: &TooltipConfig) {
        let bounds = &layout.background_bounds;
        self.queued_instances.push(TooltipBgInstance {
            rect_min: [bounds.left, bounds.top],
            rect_max: [bounds.right, bounds.bottom],
            bg_color: config.background_color,
            border_color: config.border_color,
            params: [
                config.corner_radius,
                config.border_width,
                layout.opacity,
                config.shadow_radius,
            ],
            shadow_color: config.shadow_color,
            shadow_offset: config.shadow_offset,
            arrow_params: [
                layout.arrow_direction.to_f32(),
                layout.arrow_size,
                layout.arrow_offset,
                0.0,
            ],
        });
    }

    /// Render all queued tooltip backgrounds.
    ///
    /// Must be called **inside** a render pass and **before** text rendering so
    /// that the background appears behind the text.
    pub fn render<'a>(
        &mut self,
        render_pass: &mut RenderPass<'a>,
        device: &Device,
        queue: &Queue,
        screen_width: f32,
        screen_height: f32,
    ) -> GupResult<()> {
        if self.queued_instances.is_empty() {
            return Ok(());
        }

        // Grow instance buffer if needed
        let required = self.queued_instances.len();
        if required > self.instance_capacity {
            let new_cap = required.max(self.instance_capacity * 2);
            self.instance_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Tooltip BG Instance Buffer"),
                size: (new_cap * mem::size_of::<TooltipBgInstance>()) as BufferAddress,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        // Upload instance data
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.queued_instances),
        );

        // Upload uniforms
        let projection = orthographic_projection(screen_width, screen_height);
        let uniforms = TooltipBgUniforms {
            projection,
            screen_size: [screen_width, screen_height],
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tooltip BG Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        // Draw
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..4, 0..self.queued_instances.len() as u32);

        Ok(())
    }

    /// Return the number of queued tooltip backgrounds.
    pub fn queued_count(&self) -> usize {
        self.queued_instances.len()
    }
}

/// Build a standard 2-D orthographic projection matrix mapping screen-space
/// pixels to clip-space (top-left origin, Y-down).
fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextBounds;
    use crate::text::hover_reveal::{ArrowDirection, TooltipConfig};

    #[test]
    fn tooltip_bg_instance_is_pod() {
        // Compile-time check that the struct satisfies Pod/Zeroable.
        let _zero: TooltipBgInstance = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn tooltip_bg_instance_layout() {
        // Ensure offsets match the vertex attribute declarations.
        assert_eq!(mem::offset_of!(TooltipBgInstance, rect_min), 0);
        assert_eq!(mem::offset_of!(TooltipBgInstance, rect_max), 8);
        assert_eq!(mem::offset_of!(TooltipBgInstance, bg_color), 16);
        assert_eq!(mem::offset_of!(TooltipBgInstance, border_color), 32);
        assert_eq!(mem::offset_of!(TooltipBgInstance, params), 48);
        assert_eq!(mem::offset_of!(TooltipBgInstance, shadow_color), 64);
        assert_eq!(mem::offset_of!(TooltipBgInstance, shadow_offset), 80);
        assert_eq!(mem::offset_of!(TooltipBgInstance, arrow_params), 88);
        assert_eq!(
            mem::size_of::<TooltipBgInstance>(),
            104, // 26 × f32 = 104 bytes
        );
    }

    #[test]
    fn queue_populates_instance_data() {
        // We can't create a real renderer without a GPU device, but we can
        // test the instance building logic by constructing one manually.
        let config = TooltipConfig {
            background_color: [0.1, 0.2, 0.3, 0.9],
            border_color: [0.5, 0.5, 0.5, 1.0],
            border_width: 2.0,
            corner_radius: 6.0,
            shadow_radius: 4.0,
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            shadow_offset: [1.0, 2.0],
            ..Default::default()
        };

        let layout = TooltipLayout {
            background_bounds: TextBounds::new(10.0, 20.0, 200.0, 60.0),
            text_position: crate::shader_function::Vec2 { x: 16.0, y: 24.0 },
            text: "Hello".to_string(),
            opacity: 0.8,
            arrow_direction: ArrowDirection::None,
            arrow_size: 0.0,
            arrow_offset: 0.0,
        };

        let mut instances: Vec<TooltipBgInstance> = Vec::new();

        // Simulate queue() logic
        let bounds = &layout.background_bounds;
        instances.push(TooltipBgInstance {
            rect_min: [bounds.left, bounds.top],
            rect_max: [bounds.right, bounds.bottom],
            bg_color: config.background_color,
            border_color: config.border_color,
            params: [
                config.corner_radius,
                config.border_width,
                layout.opacity,
                config.shadow_radius,
            ],
            shadow_color: config.shadow_color,
            shadow_offset: config.shadow_offset,
            arrow_params: [
                layout.arrow_direction.to_f32(),
                layout.arrow_size,
                layout.arrow_offset,
                0.0,
            ],
        });

        let inst = &instances[0];
        assert_eq!(inst.rect_min, [10.0, 20.0]);
        assert_eq!(inst.rect_max, [200.0, 60.0]);
        assert_eq!(inst.params[0], 6.0); // corner_radius
        assert_eq!(inst.params[1], 2.0); // border_width
        assert_eq!(inst.params[2], 0.8); // opacity
        assert_eq!(inst.params[3], 4.0); // shadow_radius
    }

    #[test]
    fn orthographic_projection_identity_at_origin() {
        let proj = orthographic_projection(800.0, 600.0);
        // Top-left corner (0,0) should map to clip (-1, 1)
        let x = proj[0][0] * 0.0 + proj[3][0]; // 0 + (-1) = -1
        let y = proj[1][1] * 0.0 + proj[3][1]; // 0 + 1 = 1
        assert!((x - (-1.0)).abs() < 0.001);
        assert!((y - 1.0).abs() < 0.001);

        // Centre (400, 300) should map to clip (0, 0)
        let cx = proj[0][0] * 400.0 + proj[3][0];
        let cy = proj[1][1] * 300.0 + proj[3][1];
        assert!(cx.abs() < 0.001);
        assert!(cy.abs() < 0.001);
    }

    #[test]
    fn default_config_has_corner_radius_and_shadow() {
        let config = TooltipConfig::default();
        assert!(config.corner_radius > 0.0);
        assert_eq!(config.shadow_radius, 0.0); // Shadow off by default
        assert!(config.shadow_color[3] > 0.0); // But colour is set for easy opt-in
    }
}
