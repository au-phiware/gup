// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual test utilities for rendering to offscreen textures and comparing results
//!
//! This module provides utilities for visual validation testing, enabling
//! pixel-perfect comparison of rendered output against reference images.
//! Particularly useful for validating blend modes and visual correctness.

use crate::error::{GupError, GupResult};
use crate::mixable::BlendMode;
use std::collections::HashMap;
use wgpu::util::DeviceExt;
use wgpu::*;

/// Utilities for visual testing and validation
pub struct VisualTestUtils {
    /// WebGPU device for texture creation
    device: Device,
    /// Command queue for GPU operations
    queue: Queue,
    /// Reference images stored as RGBA pixel data
    reference_images: HashMap<String, Vec<u8>>,
    /// Texture format for rendering
    texture_format: TextureFormat,
}

impl VisualTestUtils {
    /// Create a new visual test utilities instance
    pub async fn new() -> GupResult<Self> {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| GupError::WebGpuError {
                message: format!("Failed to find suitable GPU adapter: {e}"),
            })?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("visual_test_device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .map_err(|e| GupError::WebGpuError {
                message: format!("Failed to create device: {e}"),
            })?;

        Ok(Self {
            device,
            queue,
            reference_images: HashMap::new(),
            texture_format: TextureFormat::Rgba8UnormSrgb,
        })
    }

    /// Render a blend mode test to an offscreen texture and return pixel data
    ///
    /// # Arguments
    /// * `bg_color` - Background color [r, g, b, a] in 0.0-1.0 range
    /// * `fg_color` - Foreground color [r, g, b, a] in 0.0-1.0 range
    /// * `blend_mode` - The blend mode to test
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    ///
    /// # Returns
    /// RGBA pixel data as a Vec<u8>
    pub async fn render_blend_test(
        &self,
        bg_color: [f32; 4],
        fg_color: [f32; 4],
        blend_mode: BlendMode,
        width: u32,
        height: u32,
    ) -> GupResult<Vec<u8>> {
        // Create offscreen render target texture
        let render_texture = self.device.create_texture(&TextureDescriptor {
            label: Some("blend_test_render_texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.texture_format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let render_texture_view = render_texture.create_view(&TextureViewDescriptor::default());

        // Create a buffer to read back pixel data
        let buffer_size = (width * height * 4) as u64; // RGBA, 1 byte per channel
        let readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("blend_test_readback_buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Create shader and pipeline for rendering test pattern
        let shader = self.create_blend_test_shader();
        let pipeline = self.create_blend_test_pipeline(&shader, blend_mode);

        // Create vertex buffer with two overlapping quads
        let vertices = self.create_test_quad_vertices(bg_color, fg_color);
        let vertex_buffer = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("blend_test_vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: BufferUsages::VERTEX,
        });

        // Render the test pattern
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("blend_test_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("blend_test_render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &render_texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        // Copy texture to buffer for readback
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &render_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back the pixel data
        let buffer_slice = readback_buffer.slice(..);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Poll the device to complete the mapping operation
        let _ = self.device.poll(PollType::Wait);

        // Wait for the mapping to complete
        receiver
            .await
            .map_err(|_| GupError::WebGpuError {
                message: "Buffer mapping callback was dropped".to_string(),
            })?
            .map_err(|e| GupError::WebGpuError {
                message: format!("Failed to map readback buffer: {e:?}"),
            })?;

        let data = buffer_slice.get_mapped_range();
        let pixels = data.to_vec();
        drop(data);
        readback_buffer.unmap();

        Ok(pixels)
    }

    /// Compare pixel data with a reference image
    ///
    /// # Arguments
    /// * `actual` - Actual pixel data from render
    /// * `reference_name` - Name of stored reference image
    /// * `tolerance` - Acceptable difference per channel (0-255)
    ///
    /// # Returns
    /// true if images match within tolerance, false otherwise
    pub fn compare_with_reference(
        &self,
        actual: &[u8],
        reference_name: &str,
        tolerance: u8,
    ) -> bool {
        let Some(reference) = self.reference_images.get(reference_name) else {
            eprintln!("Reference image '{}' not found", reference_name);
            return false;
        };

        if actual.len() != reference.len() {
            eprintln!(
                "Size mismatch: actual {} bytes vs reference {} bytes",
                actual.len(),
                reference.len()
            );
            return false;
        }

        // Compare pixel by pixel with tolerance
        for (i, (a, r)) in actual.iter().zip(reference.iter()).enumerate() {
            let diff = (*a as i32 - *r as i32).unsigned_abs() as u8;
            if diff > tolerance {
                eprintln!(
                    "Pixel {} mismatch: actual {} vs reference {} (diff {})",
                    i, a, r, diff
                );
                return false;
            }
        }

        true
    }

    /// Store a reference image for later comparison
    pub fn store_reference_image(&mut self, name: String, pixels: Vec<u8>) {
        self.reference_images.insert(name, pixels);
    }

    /// Generate reference images for all blend modes
    ///
    /// This creates a baseline set of reference images that can be used for
    /// regression testing. Should be called once to establish ground truth.
    pub async fn generate_reference_images(&mut self, width: u32, height: u32) -> GupResult<()> {
        let test_cases = vec![
            (
                "none_red_blue",
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                BlendMode::None,
            ),
            (
                "alpha_red_blue",
                [1.0, 0.0, 0.0, 0.5],
                [0.0, 0.0, 1.0, 0.5],
                BlendMode::AlphaBlending,
            ),
            (
                "additive_red_blue",
                [0.5, 0.0, 0.0, 1.0],
                [0.0, 0.0, 0.5, 1.0],
                BlendMode::Additive,
            ),
            (
                "multiply_red_blue",
                [1.0, 0.5, 0.5, 1.0],
                [0.5, 0.5, 1.0, 1.0],
                BlendMode::Multiply,
            ),
        ];

        for (name, bg_color, fg_color, blend_mode) in test_cases {
            let pixels = self
                .render_blend_test(bg_color, fg_color, blend_mode, width, height)
                .await?;
            self.store_reference_image(name.to_string(), pixels);
        }

        Ok(())
    }

    /// Create shader module for blend testing
    fn create_blend_test_shader(&self) -> ShaderModule {
        self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("blend_test_shader"),
            source: ShaderSource::Wgsl(
                r#"
                struct VertexInput {
                    @location(0) position: vec2<f32>,
                    @location(1) color: vec4<f32>,
                }

                struct VertexOutput {
                    @builtin(position) clip_position: vec4<f32>,
                    @location(0) color: vec4<f32>,
                }

                @vertex
                fn vs_main(in: VertexInput) -> VertexOutput {
                    var out: VertexOutput;
                    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
                    out.color = in.color;
                    return out;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    return in.color;
                }
            "#
                .into(),
            ),
        })
    }

    /// Create render pipeline with specific blend mode
    fn create_blend_test_pipeline(
        &self,
        shader: &ShaderModule,
        blend_mode: BlendMode,
    ) -> RenderPipeline {
        let blend_state = match blend_mode {
            BlendMode::None => None,
            BlendMode::AlphaBlending => Some(BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::REPLACE,
            }),
            BlendMode::Multiply => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::REPLACE,
            }),
        };

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("blend_test_pipeline_layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        self.device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("blend_test_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[VertexBufferLayout {
                        array_stride: std::mem::size_of::<BlendTestVertex>() as u64,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format: self.texture_format,
                        blend: blend_state,
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
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
            })
    }

    /// Create test vertices for two overlapping quads
    fn create_test_quad_vertices(
        &self,
        bg_color: [f32; 4],
        fg_color: [f32; 4],
    ) -> Vec<BlendTestVertex> {
        let mut vertices = Vec::new();

        // Background quad (larger, left-positioned)
        let bg_left = -0.6;
        let bg_right = 0.2;
        let bg_bottom = -0.6;
        let bg_top = 0.6;

        vertices.extend_from_slice(&[
            // Triangle 1
            BlendTestVertex {
                position: [bg_left, bg_bottom],
                color: bg_color,
            },
            BlendTestVertex {
                position: [bg_right, bg_bottom],
                color: bg_color,
            },
            BlendTestVertex {
                position: [bg_left, bg_top],
                color: bg_color,
            },
            // Triangle 2
            BlendTestVertex {
                position: [bg_right, bg_bottom],
                color: bg_color,
            },
            BlendTestVertex {
                position: [bg_right, bg_top],
                color: bg_color,
            },
            BlendTestVertex {
                position: [bg_left, bg_top],
                color: bg_color,
            },
        ]);

        // Foreground quad (smaller, right-positioned, overlapping)
        let fg_left = -0.2;
        let fg_right = 0.6;
        let fg_bottom = -0.6;
        let fg_top = 0.6;

        vertices.extend_from_slice(&[
            // Triangle 1
            BlendTestVertex {
                position: [fg_left, fg_bottom],
                color: fg_color,
            },
            BlendTestVertex {
                position: [fg_right, fg_bottom],
                color: fg_color,
            },
            BlendTestVertex {
                position: [fg_left, fg_top],
                color: fg_color,
            },
            // Triangle 2
            BlendTestVertex {
                position: [fg_right, fg_bottom],
                color: fg_color,
            },
            BlendTestVertex {
                position: [fg_right, fg_top],
                color: fg_color,
            },
            BlendTestVertex {
                position: [fg_left, fg_top],
                color: fg_color,
            },
        ]);

        vertices
    }
}

/// Vertex structure for blend test rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BlendTestVertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_visual_utils_creation() {
        let utils = VisualTestUtils::new().await;
        assert!(utils.is_ok());
    }

    #[tokio::test]
    async fn test_render_blend_test() {
        let utils = VisualTestUtils::new().await.unwrap();
        let pixels = utils
            .render_blend_test(
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                BlendMode::None,
                64,
                64,
            )
            .await;
        assert!(pixels.is_ok());
        let pixels = pixels.unwrap();
        // Should have 64x64x4 bytes
        assert_eq!(pixels.len(), 64 * 64 * 4);
    }

    #[tokio::test]
    async fn test_reference_image_generation() {
        let mut utils = VisualTestUtils::new().await.unwrap();
        let result = utils.generate_reference_images(64, 64).await;
        assert!(result.is_ok());

        // Verify reference images were stored
        assert!(utils.reference_images.contains_key("none_red_blue"));
        assert!(utils.reference_images.contains_key("alpha_red_blue"));
        assert!(utils.reference_images.contains_key("additive_red_blue"));
        assert!(utils.reference_images.contains_key("multiply_red_blue"));
    }

    #[tokio::test]
    async fn test_pixel_comparison() {
        let mut utils = VisualTestUtils::new().await.unwrap();

        // Generate reference
        let reference = utils
            .render_blend_test(
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                BlendMode::None,
                64,
                64,
            )
            .await
            .unwrap();

        utils.store_reference_image("test_ref".to_string(), reference.clone());

        // Compare identical images
        assert!(utils.compare_with_reference(&reference, "test_ref", 0));

        // Compare with itself should always match
        let actual = utils
            .render_blend_test(
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                BlendMode::None,
                64,
                64,
            )
            .await
            .unwrap();

        assert!(utils.compare_with_reference(&actual, "test_ref", 2));
    }
}
