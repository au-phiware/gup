// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! General-purpose GPU-rendered UI quad renderer for overlay chrome elements.
//!
//! [`UiQuadRenderer`] draws instanced rounded rectangles with configurable
//! fill, border, corner radius, drop shadow, and optional arrow pointer.  It
//! is designed to be the single shared renderer for all UI overlay elements:
//! tooltips, legend boxes, annotation callouts, focus highlights, etc.
//!
//! # Usage
//!
//! ```rust,ignore
//! use gup::text::ui_quad::{UiQuadRenderer, UiQuadInstance};
//!
//! // Create once
//! let mut renderer = UiQuadRenderer::new(&device)?;
//!
//! // Each frame
//! renderer.begin_frame();
//! renderer.queue(UiQuadInstance { /* ... */ });
//!
//! // Inside the render pass
//! renderer.render(&mut render_pass, &device, &queue, screen_w, screen_h)?;
//! ```

use crate::error::GupResult;
use bytemuck::{Pod, Zeroable};
use std::mem;
use wgpu::util::DeviceExt;
use wgpu::*;

// ── GPU data types ──────────────────────────────────────────────────────────

/// Per-instance data uploaded to the GPU for each UI quad.
///
/// This is the low-level representation sent to the shader.  Higher-level
/// helpers such as [`UiQuadConfig`] can build instances from a friendlier
/// configuration API.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct UiQuadInstance {
    /// Top-left corner of the rectangle (screen-space pixels).
    pub rect_min: [f32; 2],
    /// Bottom-right corner of the rectangle (screen-space pixels).
    pub rect_max: [f32; 2],
    /// Background fill colour (RGBA, linear).
    pub bg_color: [f32; 4],
    /// Border colour (RGBA, linear).
    pub border_color: [f32; 4],
    /// Packed parameters: (corner_radius, border_width, opacity, shadow_radius).
    pub params: [f32; 4],
    /// Shadow colour (RGBA, linear).
    pub shadow_color: [f32; 4],
    /// Shadow offset (x, y) in pixels.
    pub shadow_offset: [f32; 2],
    /// Arrow parameters: (direction, size, offset_along_edge, 0).
    ///
    /// direction: 0 = none, 1 = top, 2 = bottom, 3 = left, 4 = right.
    /// size: triangle height in pixels.
    /// offset_along_edge: arrow centre relative to rect centre.
    pub arrow_params: [f32; 4],
}

/// Uniform data shared by all UI quads in a frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UiQuadUniforms {
    /// Orthographic projection matrix (screen-space → clip-space).
    projection: [[f32; 4]; 4],
    /// Viewport dimensions in pixels.
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

// ── Configuration builder ───────────────────────────────────────────────────

/// Arrow direction for a UI quad.
///
/// Mirrors [`super::hover_reveal::ArrowDirection`] for convenience so that
/// callers that only use the shared renderer do not need to depend on the
/// tooltip-specific module.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum UiQuadArrow {
    /// No arrow pointer (default).
    #[default]
    None,
    /// Arrow points upward from the top edge.
    Top,
    /// Arrow points downward from the bottom edge.
    Bottom,
    /// Arrow points left from the left edge.
    Left,
    /// Arrow points right from the right edge.
    Right,
}

impl UiQuadArrow {
    /// Convert to the shader's float encoding (0–4).
    pub fn to_f32(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Top => 1.0,
            Self::Bottom => 2.0,
            Self::Left => 3.0,
            Self::Right => 4.0,
        }
    }
}

/// Friendly builder for constructing [`UiQuadInstance`] values.
///
/// Provides sensible defaults and a fluent API:
///
/// ```rust,ignore
/// let inst = UiQuadConfig::new(10.0, 20.0, 200.0, 60.0)
///     .bg_color([0.15, 0.15, 0.15, 0.95])
///     .corner_radius(6.0)
///     .border(1.0, [0.4, 0.4, 0.4, 1.0])
///     .shadow(4.0, [0.0, 0.0, 0.0, 0.5], [1.0, 2.0])
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct UiQuadConfig {
    rect_min: [f32; 2],
    rect_max: [f32; 2],
    bg_color: [f32; 4],
    border_color: [f32; 4],
    corner_radius: f32,
    border_width: f32,
    opacity: f32,
    shadow_radius: f32,
    shadow_color: [f32; 4],
    shadow_offset: [f32; 2],
    arrow: UiQuadArrow,
    arrow_size: f32,
    arrow_offset: f32,
}

impl UiQuadConfig {
    /// Create a new builder for a rectangle defined by its screen-space
    /// corners.
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            rect_min: [left, top],
            rect_max: [right, bottom],
            bg_color: [0.15, 0.15, 0.15, 0.95],
            border_color: [0.4, 0.4, 0.4, 1.0],
            corner_radius: 4.0,
            border_width: 0.0,
            opacity: 1.0,
            shadow_radius: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            shadow_offset: [0.0, 0.0],
            arrow: UiQuadArrow::None,
            arrow_size: 0.0,
            arrow_offset: 0.0,
        }
    }

    /// Set the background fill colour (RGBA, linear).
    pub fn bg_color(mut self, color: [f32; 4]) -> Self {
        self.bg_color = color;
        self
    }

    /// Set the corner radius in pixels.
    pub fn corner_radius(mut self, r: f32) -> Self {
        self.corner_radius = r;
        self
    }

    /// Set border width and colour.
    pub fn border(mut self, width: f32, color: [f32; 4]) -> Self {
        self.border_width = width;
        self.border_color = color;
        self
    }

    /// Set overall opacity (0.0–1.0).
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }

    /// Configure a drop shadow.
    pub fn shadow(mut self, radius: f32, color: [f32; 4], offset: [f32; 2]) -> Self {
        self.shadow_radius = radius;
        self.shadow_color = color;
        self.shadow_offset = offset;
        self
    }

    /// Add an arrow pointer.
    pub fn arrow(mut self, direction: UiQuadArrow, size: f32, offset: f32) -> Self {
        self.arrow = direction;
        self.arrow_size = size;
        self.arrow_offset = offset;
        self
    }

    /// Consume the builder and produce a [`UiQuadInstance`].
    pub fn build(self) -> UiQuadInstance {
        UiQuadInstance {
            rect_min: self.rect_min,
            rect_max: self.rect_max,
            bg_color: self.bg_color,
            border_color: self.border_color,
            params: [
                self.corner_radius,
                self.border_width,
                self.opacity,
                self.shadow_radius,
            ],
            shadow_color: self.shadow_color,
            shadow_offset: self.shadow_offset,
            arrow_params: [self.arrow.to_f32(), self.arrow_size, self.arrow_offset, 0.0],
        }
    }
}

// ── Renderer ────────────────────────────────────────────────────────────────

/// GPU-accelerated renderer for UI overlay quads (rounded rectangles).
///
/// Uses instanced rendering with an SDF-based fragment shader to draw any
/// number of heterogeneous UI elements (tooltips, legend boxes, annotation
/// backgrounds, focus highlights, etc.) in a single draw call per frame.
pub struct UiQuadRenderer {
    render_pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    instance_buffer: Buffer,
    uniform_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    instance_capacity: usize,
    queued_instances: Vec<UiQuadInstance>,
}

impl std::fmt::Debug for UiQuadRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiQuadRenderer")
            .field("instance_capacity", &self.instance_capacity)
            .field("queued_instances", &self.queued_instances.len())
            .finish()
    }
}

impl UiQuadRenderer {
    /// Create a new UI quad renderer.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("UI Quad Shader"),
            source: ShaderSource::Wgsl(include_str!("../shaders/ui_quad.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("UI Quad Bind Group Layout"),
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
            label: Some("UI Quad Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Quad Render Pipeline"),
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
                        array_stride: mem::size_of::<UiQuadInstance>() as BufferAddress,
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
            label: Some("UI Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: BufferUsages::VERTEX,
        });

        let instance_capacity = 16;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Quad Instance Buffer"),
            size: (instance_capacity * mem::size_of::<UiQuadInstance>()) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Quad Uniform Buffer"),
            size: mem::size_of::<UiQuadUniforms>() as BufferAddress,
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

    /// Queue a UI quad instance for rendering.
    ///
    /// Call between [`begin_frame`](Self::begin_frame) and
    /// [`render`](Self::render).
    pub fn queue(&mut self, instance: UiQuadInstance) {
        self.queued_instances.push(instance);
    }

    /// Return the number of queued UI quad instances.
    pub fn queued_count(&self) -> usize {
        self.queued_instances.len()
    }

    /// Render all queued UI quads.
    ///
    /// Must be called **inside** an active render pass.  Typically called
    /// before text rendering so that backgrounds appear behind text.
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
                label: Some("UI Quad Instance Buffer"),
                size: (new_cap * mem::size_of::<UiQuadInstance>()) as BufferAddress,
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
        let uniforms = UiQuadUniforms {
            projection,
            screen_size: [screen_width, screen_height],
            _padding: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("UI Quad Bind Group"),
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

    #[test]
    fn ui_quad_instance_is_pod() {
        // Compile-time check that the struct satisfies Pod/Zeroable.
        let _zero: UiQuadInstance = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn ui_quad_instance_layout() {
        // Ensure offsets match the vertex attribute declarations.
        assert_eq!(mem::offset_of!(UiQuadInstance, rect_min), 0);
        assert_eq!(mem::offset_of!(UiQuadInstance, rect_max), 8);
        assert_eq!(mem::offset_of!(UiQuadInstance, bg_color), 16);
        assert_eq!(mem::offset_of!(UiQuadInstance, border_color), 32);
        assert_eq!(mem::offset_of!(UiQuadInstance, params), 48);
        assert_eq!(mem::offset_of!(UiQuadInstance, shadow_color), 64);
        assert_eq!(mem::offset_of!(UiQuadInstance, shadow_offset), 80);
        assert_eq!(mem::offset_of!(UiQuadInstance, arrow_params), 88);
        assert_eq!(
            mem::size_of::<UiQuadInstance>(),
            104, // 26 × f32 = 104 bytes
        );
    }

    #[test]
    fn config_builder_defaults() {
        let inst = UiQuadConfig::new(10.0, 20.0, 200.0, 60.0).build();
        assert_eq!(inst.rect_min, [10.0, 20.0]);
        assert_eq!(inst.rect_max, [200.0, 60.0]);
        // Default opacity = 1.0
        assert_eq!(inst.params[2], 1.0);
        // No border by default
        assert_eq!(inst.params[1], 0.0);
        // No shadow by default
        assert_eq!(inst.params[3], 0.0);
        // No arrow by default
        assert_eq!(inst.arrow_params[0], 0.0);
    }

    #[test]
    fn config_builder_full() {
        let inst = UiQuadConfig::new(10.0, 20.0, 200.0, 60.0)
            .bg_color([0.1, 0.2, 0.3, 0.9])
            .corner_radius(8.0)
            .border(2.0, [0.5, 0.5, 0.5, 1.0])
            .opacity(0.8)
            .shadow(4.0, [0.0, 0.0, 0.0, 0.5], [1.0, 2.0])
            .arrow(UiQuadArrow::Bottom, 6.0, 3.0)
            .build();

        assert_eq!(inst.bg_color, [0.1, 0.2, 0.3, 0.9]);
        assert_eq!(inst.params[0], 8.0); // corner_radius
        assert_eq!(inst.params[1], 2.0); // border_width
        assert_eq!(inst.params[2], 0.8); // opacity
        assert_eq!(inst.params[3], 4.0); // shadow_radius
        assert_eq!(inst.border_color, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(inst.shadow_color, [0.0, 0.0, 0.0, 0.5]);
        assert_eq!(inst.shadow_offset, [1.0, 2.0]);
        assert_eq!(inst.arrow_params[0], 2.0); // Bottom
        assert_eq!(inst.arrow_params[1], 6.0); // arrow_size
        assert_eq!(inst.arrow_params[2], 3.0); // arrow_offset
    }

    #[test]
    fn arrow_direction_encoding() {
        assert_eq!(UiQuadArrow::None.to_f32(), 0.0);
        assert_eq!(UiQuadArrow::Top.to_f32(), 1.0);
        assert_eq!(UiQuadArrow::Bottom.to_f32(), 2.0);
        assert_eq!(UiQuadArrow::Left.to_f32(), 3.0);
        assert_eq!(UiQuadArrow::Right.to_f32(), 4.0);
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
}
