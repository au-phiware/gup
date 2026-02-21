// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Focus ring rendering for data points.
//!
//! This module provides GPU-accelerated rendering of focus rings around focused
//! data points, with configurable visual styles for accessibility.

use crate::error::GupResult;
use crate::interaction::Rect;
use wgpu::util::DeviceExt;

/// Visual style for focus rings.
#[derive(Debug, Clone)]
pub struct FocusRingStyle {
    /// Color of the focus ring (RGBA)
    pub color: [f32; 4],

    /// Width of the focus ring in pixels
    pub width: f32,

    /// Dash pattern (empty for solid line)
    pub dash_pattern: Vec<f32>,

    /// Animation speed (0.0 = no animation)
    pub animation_speed: f32,
}

impl Default for FocusRingStyle {
    fn default() -> Self {
        Self {
            color: [0.0, 0.5, 1.0, 1.0], // Blue focus ring
            width: 2.0,
            dash_pattern: vec![],
            animation_speed: 0.0,
        }
    }
}

impl FocusRingStyle {
    /// High contrast focus ring style (WCAG AAA compliant).
    pub fn high_contrast() -> Self {
        Self {
            color: [1.0, 1.0, 0.0, 1.0], // Yellow
            width: 3.0,
            dash_pattern: vec![],
            animation_speed: 0.0,
        }
    }

    /// Animated focus ring style.
    pub fn animated() -> Self {
        Self {
            color: [0.0, 0.5, 1.0, 1.0],
            width: 2.0,
            dash_pattern: vec![5.0, 5.0],
            animation_speed: 1.0,
        }
    }
}

/// GPU-accelerated focus ring renderer.
///
/// Renders focus rings around data points using instanced rendering for performance.
#[derive(Debug)]
pub struct FocusRingRenderer {
    style: FocusRingStyle,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    pipeline: Option<wgpu::RenderPipeline>,
    animation_time: f32,
}

impl FocusRingRenderer {
    /// Create a new focus ring renderer with default style.
    pub fn new() -> Self {
        Self {
            style: FocusRingStyle::default(),
            vertex_buffer: None,
            instance_buffer: None,
            pipeline: None,
            animation_time: 0.0,
        }
    }

    /// Create a new focus ring renderer with custom style.
    pub fn with_style(style: FocusRingStyle) -> Self {
        Self {
            style,
            vertex_buffer: None,
            instance_buffer: None,
            pipeline: None,
            animation_time: 0.0,
        }
    }

    /// Set the focus ring style.
    pub fn set_style(&mut self, style: FocusRingStyle) {
        self.style = style;
        // Force pipeline recreation on next render
        self.pipeline = None;
    }

    /// Update animation time.
    pub fn update(&mut self, delta_time: f32) {
        if self.style.animation_speed > 0.0 {
            self.animation_time += delta_time * self.style.animation_speed;
            if self.animation_time > 1000.0 {
                self.animation_time = 0.0; // Wrap to avoid overflow
            }
        }
    }

    /// Render a focus ring around the given bounds.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device for buffer creation
    /// * `render_pass` - Active render pass
    /// * `bounds` - Bounds to draw focus ring around
    pub fn render_focus_ring(
        &mut self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass,
        bounds: Rect,
    ) -> GupResult<()> {
        self.render_focus_rings(device, render_pass, &[bounds])
    }

    /// Render multiple focus rings (for multi-select support).
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device for buffer creation
    /// * `render_pass` - Active render pass
    /// * `bounds_list` - List of bounds to draw focus rings around
    pub fn render_focus_rings(
        &mut self,
        device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass,
        bounds_list: &[Rect],
    ) -> GupResult<()> {
        if bounds_list.is_empty() {
            return Ok(());
        }

        // Create vertex buffer for rectangle outline (8 vertices for 4 line segments)
        if self.vertex_buffer.is_none() {
            let vertices = self.create_ring_vertices();
            self.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Focus Ring Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create instance buffer with current bounds
        let instances = bounds_list
            .iter()
            .map(|bounds| self.create_ring_instance(*bounds))
            .collect::<Vec<_>>();

        self.instance_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Focus Ring Instance Buffer"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        // Create or use existing pipeline
        if self.pipeline.is_none() {
            self.pipeline = Some(self.create_pipeline(device)?);
        }

        // Render
        if let (Some(pipeline), Some(vertex_buffer), Some(instance_buffer)) =
            (&self.pipeline, &self.vertex_buffer, &self.instance_buffer)
        {
            render_pass.set_pipeline(pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
            render_pass.draw(0..8, 0..bounds_list.len() as u32);
        }

        Ok(())
    }

    fn create_ring_vertices(&self) -> Vec<FocusRingVertex> {
        // Create 8 vertices for 4 line segments (top, right, bottom, left)
        // Each segment is a quad with thickness
        vec![
            // Top edge
            FocusRingVertex {
                position: [-1.0, 1.0],
                local: [0.0, 0.0],
            },
            FocusRingVertex {
                position: [1.0, 1.0],
                local: [1.0, 0.0],
            },
            // Right edge
            FocusRingVertex {
                position: [1.0, 1.0],
                local: [0.0, 0.0],
            },
            FocusRingVertex {
                position: [1.0, -1.0],
                local: [0.0, 1.0],
            },
            // Bottom edge
            FocusRingVertex {
                position: [1.0, -1.0],
                local: [1.0, 0.0],
            },
            FocusRingVertex {
                position: [-1.0, -1.0],
                local: [0.0, 0.0],
            },
            // Left edge
            FocusRingVertex {
                position: [-1.0, -1.0],
                local: [0.0, 1.0],
            },
            FocusRingVertex {
                position: [-1.0, 1.0],
                local: [1.0, 0.0],
            },
        ]
    }

    fn create_ring_instance(&self, bounds: Rect) -> FocusRingInstance {
        FocusRingInstance {
            center: [bounds.center().x, bounds.center().y],
            half_size: [bounds.width() / 2.0, bounds.height() / 2.0],
            color: self.style.color,
            width: self.style.width,
            animation_phase: self.animation_time,
            _padding: [0.0; 3],
        }
    }

    fn create_pipeline(&self, device: &wgpu::Device) -> GupResult<wgpu::RenderPipeline> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Focus Ring Shader"),
            source: wgpu::ShaderSource::Wgsl(FOCUS_RING_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Focus Ring Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Focus Ring Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<FocusRingVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x2, // position
                            1 => Float32x2, // local
                        ],
                    },
                    // Instance buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<FocusRingInstance>()
                            as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            2 => Float32x2, // center
                            3 => Float32x2, // half_size
                            4 => Float32x4, // color
                            5 => Float32,   // width
                            6 => Float32,   // animation_phase
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
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
}

impl Default for FocusRingRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FocusRingVertex {
    position: [f32; 2],
    local: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct FocusRingInstance {
    center: [f32; 2],
    half_size: [f32; 2],
    color: [f32; 4],
    width: f32,
    animation_phase: f32,
    _padding: [f32; 3],
}

const FOCUS_RING_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) local: vec2<f32>,
};

struct InstanceInput {
    @location(2) center: vec2<f32>,
    @location(3) half_size: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) width: f32,
    @location(6) animation_phase: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var output: VertexOutput;
    
    // Transform vertex position to instance bounds
    let world_pos = instance.center + vertex.position * instance.half_size;
    
    // Convert to clip space (assuming normalized device coordinates)
    output.position = vec4<f32>(world_pos, 0.0, 1.0);
    output.color = instance.color;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_style() {
        let style = FocusRingStyle::default();
        assert_eq!(style.width, 2.0);
        assert_eq!(style.animation_speed, 0.0);
    }

    #[test]
    fn test_high_contrast_style() {
        let style = FocusRingStyle::high_contrast();
        assert_eq!(style.width, 3.0);
        assert_eq!(style.color, [1.0, 1.0, 0.0, 1.0]); // Yellow
    }

    #[test]
    fn test_renderer_creation() {
        let renderer = FocusRingRenderer::new();
        assert_eq!(renderer.animation_time, 0.0);
    }

    #[test]
    fn test_animation_update() {
        let mut renderer = FocusRingRenderer::with_style(FocusRingStyle::animated());
        renderer.update(0.016); // 60fps frame
        assert!(renderer.animation_time > 0.0);
    }
}
