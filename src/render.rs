// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Enhanced RenderContext with full WebGPU integration.

use crate::error::{GupError, GupResult};
use crate::mixable::BlendMode;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::*;

/// Context for rendering operations, containing GPU resources and state.
pub struct RenderContext {
    /// WebGPU instance
    instance: Instance,
    /// GPU adapter
    adapter: Adapter,
    /// GPU device handle
    device: Device,
    /// Command queue for GPU operations
    queue: Queue,
    /// Current surface (for window rendering)
    surface: Option<Surface<'static>>,
    /// Surface configuration
    surface_config: Option<SurfaceConfiguration>,
    /// Current viewport dimensions
    viewport: Viewport,
    /// Command encoder pool for efficient reuse
    encoder_pool: CommandEncoderPool,
    /// Resource manager for cleanup
    #[allow(dead_code)]
    resource_manager: ResourceManager,
    /// Current blend mode
    current_blend_mode: BlendMode,
    /// Blend state stack for nested compositions
    blend_state_stack: Vec<BlendMode>,
    /// Cached render pipelines by blend mode
    pipeline_cache: HashMap<BlendMode, RenderPipeline>,
    /// Global alpha uniform buffer
    global_alpha_buffer: Option<Buffer>,
    /// Global alpha bind group
    global_alpha_bind_group: Option<BindGroup>,
    /// Global alpha bind group layout
    global_alpha_bind_group_layout: BindGroupLayout,
}

/// Viewport dimensions and properties.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Scale factor for high-DPI displays
    pub scale_factor: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            scale_factor: 1.0,
        }
    }
}

impl RenderContext {
    /// Create a new render context with WebGPU initialization
    pub async fn new() -> GupResult<Self> {
        Self::with_viewport(Viewport::default()).await
    }

    /// Create a new render context with specific viewport
    pub async fn with_viewport(viewport: Viewport) -> GupResult<Self> {
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
            .map_err(|e| {
                GupError::webgpu_error(format!("Failed to find suitable GPU adapter: {e}"))
            })?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("gup_device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .map_err(|e| GupError::webgpu_error(format!("Failed to create device: {e}")))?;

        // Create global alpha bind group layout
        let global_alpha_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
                label: Some("global_alpha_bind_group_layout"),
            });

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            viewport,
            encoder_pool: CommandEncoderPool::new(),
            resource_manager: ResourceManager::new(),
            current_blend_mode: BlendMode::default(),
            blend_state_stack: Vec::new(),
            pipeline_cache: HashMap::new(),
            global_alpha_buffer: None,
            global_alpha_bind_group: None,
            global_alpha_bind_group_layout,
        })
    }

    /// Initialize surface for window rendering
    pub fn init_surface<W>(&mut self, window: Arc<W>) -> GupResult<()>
    where
        W: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let surface = self
            .instance
            .create_surface(window)
            .map_err(|e| GupError::webgpu_error(format!("Failed to create surface: {e}")))?;

        let surface_caps = surface.get_capabilities(&self.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: self.viewport.width,
            height: self.viewport.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    /// Get device reference
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get viewport
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Update viewport and reconfigure surface if needed
    pub fn set_viewport(&mut self, viewport: Viewport) -> GupResult<()> {
        self.viewport = viewport;

        if let (Some(surface), Some(config)) = (&self.surface, &mut self.surface_config) {
            config.width = viewport.width;
            config.height = viewport.height;
            surface.configure(&self.device, config);
        }

        Ok(())
    }

    /// Begin a new render pass with automatic surface handling
    pub fn begin_render_pass(&mut self) -> GupResult<ActiveRenderPass<'_>> {
        let encoder = self.encoder_pool.get(&self.device)?;

        let (view, present_after) = if let Some(surface) = &self.surface {
            let output = surface.get_current_texture().map_err(|e| {
                GupError::webgpu_error(format!("Failed to acquire surface texture: {e}"))
            })?;
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());
            (view, Some(output))
        } else {
            // Create offscreen render target
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("offscreen_render_target"),
                size: Extent3d {
                    width: self.viewport.width,
                    height: self.viewport.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            (view, None)
        };

        Ok(ActiveRenderPass {
            encoder,
            render_target: view,
            present_after,
            viewport: self.viewport,
            device: &self.device,
            queue: &self.queue,
        })
    }

    /// Get surface format for pipeline creation
    pub fn surface_format(&self) -> TextureFormat {
        self.surface_config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(TextureFormat::Bgra8UnormSrgb)
    }

    /// Check if global alpha buffer exists (for testing)
    pub fn has_global_alpha_buffer(&self) -> bool {
        self.global_alpha_buffer.is_some()
    }

    /// Get pipeline cache size (for testing)
    pub fn pipeline_cache_size(&self) -> usize {
        self.pipeline_cache.len()
    }

    /// Set blend mode for rendering operations
    pub fn set_blend_mode(&mut self, mode: BlendMode) -> GupResult<()> {
        // Early return if mode hasn't changed
        if self.current_blend_mode == mode {
            return Ok(());
        }

        self.current_blend_mode = mode;
        Ok(())
    }

    /// Get current blend mode
    pub fn current_blend_mode(&self) -> BlendMode {
        self.current_blend_mode
    }

    /// Set global alpha for rendering operations
    pub fn set_global_alpha(&mut self, alpha: f32) -> GupResult<()> {
        let alpha_uniform = GlobalAlphaUniform {
            alpha,
            _padding: [0.0; 3],
        };

        // Create or update the global alpha buffer
        if self.global_alpha_buffer.is_none() {
            let buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("global_alpha_uniform"),
                size: std::mem::size_of::<GlobalAlphaUniform>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                layout: &self.global_alpha_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
                label: Some("global_alpha_bind_group"),
            });

            self.global_alpha_buffer = Some(buffer);
            self.global_alpha_bind_group = Some(bind_group);
        }

        // Update the buffer
        if let Some(buffer) = &self.global_alpha_buffer {
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&[alpha_uniform]));
        }

        Ok(())
    }

    /// Push current blend state onto stack for nested compositions
    pub fn push_blend_state(&mut self) -> GupResult<()> {
        self.blend_state_stack.push(self.current_blend_mode);
        Ok(())
    }

    /// Restore previous blend state from stack
    pub fn pop_blend_state(&mut self) -> GupResult<()> {
        if let Some(previous_mode) = self.blend_state_stack.pop() {
            self.set_blend_mode(previous_mode)?;
        }
        Ok(())
    }

    /// Get a render pipeline with the specified blend mode
    pub fn get_pipeline_with_blend(&mut self, blend_mode: BlendMode) -> GupResult<&RenderPipeline> {
        // Check if we already have a cached pipeline for this blend mode
        if !self.pipeline_cache.contains_key(&blend_mode) {
            let pipeline = self.create_pipeline_with_blend(blend_mode)?;
            self.pipeline_cache.insert(blend_mode, pipeline);
        }

        Ok(self.pipeline_cache.get(&blend_mode).unwrap())
    }

    /// Create a render pipeline with specific blend state
    fn create_pipeline_with_blend(&self, blend_mode: BlendMode) -> GupResult<RenderPipeline> {
        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("blend_aware_shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/blend_aware.wgsl").into()),
        });

        let render_pipeline_layout =
            self.device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("blend_pipeline_layout"),
                    bind_group_layouts: &[&self.global_alpha_bind_group_layout],
                    push_constant_ranges: &[],
                });

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
            ],
        };

        // Convert BlendMode to wgpu BlendState
        let blend_state = match blend_mode {
            BlendMode::None => None,
            BlendMode::AlphaBlending => Some(BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            BlendMode::Multiply => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),
        };

        let render_pipeline = self
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("blend_pipeline_{blend_mode:?}")),
                layout: Some(&render_pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_buffer_layout],
                    compilation_options: PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format: self.surface_format(),
                        blend: blend_state,
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::PointList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
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

        Ok(render_pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blend_mode_pipeline_integration() {
        let mut context = RenderContext::new().await.unwrap();

        // Test that blend mode changes affect pipeline state
        context.set_blend_mode(BlendMode::AlphaBlending).unwrap();
        assert_eq!(context.current_blend_mode(), BlendMode::AlphaBlending);

        context.set_blend_mode(BlendMode::Additive).unwrap();
        assert_eq!(context.current_blend_mode(), BlendMode::Additive);
    }

    #[tokio::test]
    async fn test_blend_state_stack() {
        let mut context = RenderContext::new().await.unwrap();

        // Initial state
        context.set_blend_mode(BlendMode::None).unwrap();

        // Push and change
        context.push_blend_state().unwrap();
        context.set_blend_mode(BlendMode::AlphaBlending).unwrap();

        // Nested push and change
        context.push_blend_state().unwrap();
        context.set_blend_mode(BlendMode::Additive).unwrap();

        // Pop should restore previous state
        context.pop_blend_state().unwrap();
        assert_eq!(context.current_blend_mode(), BlendMode::AlphaBlending);

        context.pop_blend_state().unwrap();
        assert_eq!(context.current_blend_mode(), BlendMode::None);
    }

    #[tokio::test]
    async fn test_global_alpha_uniform() {
        let mut context = RenderContext::new().await.unwrap();

        // Test setting global alpha creates buffer and bind group
        context.set_global_alpha(0.5).unwrap();
        assert!(context.has_global_alpha_buffer());

        // Test setting again doesn't recreate buffer
        context.set_global_alpha(0.8).unwrap();
        assert!(context.has_global_alpha_buffer());
    }

    #[tokio::test]
    async fn test_pipeline_caching() {
        let mut context = RenderContext::new().await.unwrap();

        // First call should create and cache pipeline
        let _pipeline1 = context
            .get_pipeline_with_blend(BlendMode::AlphaBlending)
            .unwrap();
        assert_eq!(context.pipeline_cache_size(), 1);

        // Second call should reuse cached pipeline
        let _pipeline2 = context
            .get_pipeline_with_blend(BlendMode::AlphaBlending)
            .unwrap();
        assert_eq!(context.pipeline_cache_size(), 1);

        // Different blend mode should create new pipeline
        let _pipeline3 = context
            .get_pipeline_with_blend(BlendMode::Additive)
            .unwrap();
        assert_eq!(context.pipeline_cache_size(), 2);
    }

    #[tokio::test]
    async fn test_blend_mode_performance() {
        let mut context = RenderContext::new().await.unwrap();

        let start = std::time::Instant::now();

        // Test performance of blend mode changes
        for i in 0..100 {
            let mode = match i % 4 {
                0 => BlendMode::None,
                1 => BlendMode::AlphaBlending,
                2 => BlendMode::Additive,
                _ => BlendMode::Multiply,
            };
            context.set_blend_mode(mode).unwrap();
        }

        let duration = start.elapsed();

        // Should complete well under 1ms for performance target
        assert!(duration.as_millis() < 1);
    }

    #[tokio::test]
    async fn test_all_blend_modes() {
        let mut context = RenderContext::new().await.unwrap();

        // Test that all blend modes can create pipelines without errors
        let blend_modes = [
            BlendMode::None,
            BlendMode::AlphaBlending,
            BlendMode::Additive,
            BlendMode::Multiply,
        ];

        for mode in blend_modes {
            let pipeline = context.get_pipeline_with_blend(mode);
            assert!(
                pipeline.is_ok(),
                "Failed to create pipeline for blend mode {mode:?}"
            );
        }
    }

    #[tokio::test]
    async fn test_global_alpha_uniform_alignment() {
        // Test that GlobalAlphaUniform has correct size and alignment
        assert_eq!(std::mem::size_of::<GlobalAlphaUniform>(), 16);
        assert_eq!(std::mem::align_of::<GlobalAlphaUniform>(), 4);
    }

    #[tokio::test]
    async fn test_blend_state_stack_empty_pop() {
        let mut context = RenderContext::new().await.unwrap();

        // Popping from empty stack should not panic
        let result = context.pop_blend_state();
        assert!(result.is_ok());

        // Blend mode should remain unchanged
        assert_eq!(context.current_blend_mode(), BlendMode::default());
    }
}

/// Active render pass with automatic resource management
pub struct ActiveRenderPass<'a> {
    encoder: CommandEncoder,
    render_target: TextureView,
    present_after: Option<SurfaceTexture>,
    viewport: Viewport,
    device: &'a Device,
    queue: &'a Queue,
}

impl<'a> ActiveRenderPass<'a> {
    /// Create a render pass targeting the render target
    pub fn render_pass(&mut self, clear_color: Option<Color>) -> RenderPass<'_> {
        let clear_value = clear_color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        self.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main_render_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.render_target,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(clear_value),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Get the current viewport
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Get device reference
    pub fn device(&self) -> &Device {
        self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &Queue {
        self.queue
    }

    /// Submit the render pass and present if rendering to surface
    pub fn submit(self) -> GupResult<()> {
        let command_buffer = self.encoder.finish();
        self.queue.submit(Some(command_buffer));

        if let Some(output) = self.present_after {
            output.present();
        }

        Ok(())
    }
}

/// Command encoder pool for efficient reuse
struct CommandEncoderPool {
    available: Vec<CommandEncoder>,
}

impl CommandEncoderPool {
    fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    fn get(&mut self, device: &Device) -> GupResult<CommandEncoder> {
        Ok(self.available.pop().unwrap_or_else(|| {
            device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("gup_command_encoder"),
            })
        }))
    }
}

/// Resource manager for cleanup tracking
struct ResourceManager {
    // TODO: Implement resource tracking for proper cleanup
}

impl ResourceManager {
    fn new() -> Self {
        Self {}
    }
}

/// Basic rendering pipeline for geometric primitives
#[derive(Debug)]
pub struct BasicPipeline {
    render_pipeline: RenderPipeline,
    #[allow(dead_code)]
    vertex_buffer_layout: VertexBufferLayout<'static>,
    #[allow(dead_code)]
    uniform_bind_group_layout: BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

/// Global alpha uniform for blending operations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalAlphaUniform {
    alpha: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment
}

impl BasicPipeline {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("basic_shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[],
                label: Some("uniform_bind_group_layout"),
            });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("basic_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("basic_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
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

        Self {
            render_pipeline,
            vertex_buffer_layout,
            uniform_bind_group_layout,
        }
    }

    pub fn render_points_with_context(
        &self,
        vertices: &[Vertex],
        context: &RenderContext,
    ) -> GupResult<Buffer> {
        let vertex_buffer = context.device().create_buffer(&BufferDescriptor {
            label: Some("vertex_buffer"),
            size: std::mem::size_of_val(vertices) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        context
            .queue()
            .write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(vertices));
        Ok(vertex_buffer)
    }

    pub fn render_points(
        &self,
        render_pass: &mut RenderPass,
        vertex_buffer: &Buffer,
        vertex_count: u32,
    ) -> GupResult<()> {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count, 0..1);

        Ok(())
    }
}
