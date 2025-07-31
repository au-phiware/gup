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

//! Unified render context system for GPU resource management.
//!
//! The GupContext provides the foundation for all GPU operations in Gup, encapsulating
//! wgpu device, queue, surface management, and providing a unified interface for
//! rendering operations across all components.

use crate::buffer::{BufferPool, BufferType, GpuBuffer};
use crate::error::{GupError, GupResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::*;

/// Configuration options for GupContext initialization.
#[derive(Debug, Clone)]
pub struct GupOptions {
    /// Power preference for adapter selection
    pub power_preference: PowerPreference,
    /// Required WebGPU features
    pub required_features: Features,
    /// Required WebGPU limits
    pub required_limits: Limits,
    /// Backend selection preference
    pub backends: Backends,
}

impl Default for GupOptions {
    fn default() -> Self {
        Self {
            power_preference: PowerPreference::HighPerformance,
            required_features: Features::empty(),
            required_limits: Limits::default(),
            #[cfg(target_arch = "wasm32")]
            backends: Backends::BROWSER_WEBGPU | Backends::GL,
            #[cfg(not(target_arch = "wasm32"))]
            backends: Backends::PRIMARY,
        }
    }
}

/// Performance statistics for frame rendering.
#[derive(Debug, Default, Clone)]
pub struct FrameStats {
    /// Total number of frames rendered
    pub frames_rendered: u64,
    /// Average frame time in milliseconds
    pub avg_frame_time: f32,
    /// Minimum frame time in milliseconds
    pub min_frame_time: f32,
    /// Maximum frame time in milliseconds
    pub max_frame_time: f32,
    /// Current frame time in milliseconds
    pub current_frame_time: f32,
    /// GPU memory usage in bytes
    pub gpu_memory_usage: u64,
}

impl FrameStats {
    /// Update statistics with a new frame time.
    pub fn update_frame_time(&mut self, frame_time: Duration) {
        let frame_time_ms = frame_time.as_secs_f32() * 1000.0;

        self.current_frame_time = frame_time_ms;
        self.frames_rendered += 1;

        if self.frames_rendered == 1 {
            self.avg_frame_time = frame_time_ms;
            self.min_frame_time = frame_time_ms;
            self.max_frame_time = frame_time_ms;
        } else {
            // Moving average
            self.avg_frame_time = (self.avg_frame_time * 0.9) + (frame_time_ms * 0.1);
            self.min_frame_time = self.min_frame_time.min(frame_time_ms);
            self.max_frame_time = self.max_frame_time.max(frame_time_ms);
        }
    }

    /// Get frames per second based on average frame time.
    pub fn fps(&self) -> f32 {
        if self.avg_frame_time > 0.0 {
            1000.0 / self.avg_frame_time
        } else {
            0.0
        }
    }
}

/// Texture pool for efficient texture resource management.
#[derive(Debug)]
pub struct TexturePool {
    // Placeholder implementation - can be extended later
    device: Arc<Device>,
}

impl TexturePool {
    fn new(device: Arc<Device>) -> Self {
        Self { device }
    }

    /// Create a texture with the given descriptor.
    pub fn create_texture(&self, descriptor: &TextureDescriptor) -> Texture {
        self.device.create_texture(descriptor)
    }
}

/// Unified render context that manages GPU resources and provides rendering capabilities.
#[derive(Debug)]
pub struct GupContext {
    /// Core wgpu resources
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,

    /// Rendering targets
    surface: Option<Surface<'static>>,
    surface_config: Option<SurfaceConfiguration>,

    /// Resource management
    buffer_pool: BufferPool,
    texture_pool: TexturePool,

    /// Performance monitoring
    frame_stats: FrameStats,
    frame_start_time: Option<Instant>,

    /// WebGPU instance and adapter (kept for potential reconfiguration)
    _instance: Instance,
    _adapter: Adapter,
}

impl GupContext {
    /// Create a new render context with default options.
    pub async fn new() -> GupResult<Arc<Self>> {
        Self::with_options(GupOptions::default()).await
    }

    /// Initialize with specific window/surface.
    pub async fn with_surface<W>(window: Arc<W>) -> GupResult<Arc<Self>>
    where
        W: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let mut context = Self::new().await?;
        Arc::get_mut(&mut context)
            .ok_or_else(|| GupError::ResourceError("Context already shared".to_string()))?
            .init_surface(window)?;
        Ok(context)
    }

    /// Headless initialization for server-side rendering.
    pub async fn headless() -> GupResult<Arc<Self>> {
        Self::new().await
    }

    /// Custom initialization with advanced options.
    pub async fn with_options(options: GupOptions) -> GupResult<Arc<Self>> {
        let instance = Instance::new(&InstanceDescriptor {
            backends: options.backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: options.power_preference,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| {
                GupError::WebGpuError(format!("Failed to find suitable GPU adapter: {e}"))
            })?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("gup_device"),
                required_features: options.required_features,
                required_limits: options.required_limits,
                memory_hints: MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .map_err(|e| GupError::WebGpuError(format!("Failed to create device: {e}")))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let buffer_pool = BufferPool::new(Arc::clone(&device));
        let texture_pool = TexturePool::new(Arc::clone(&device));

        Ok(Arc::new(Self {
            device,
            queue,
            surface: None,
            surface_config: None,
            buffer_pool,
            texture_pool,
            frame_stats: FrameStats::default(),
            frame_start_time: None,
            _instance: instance,
            _adapter: adapter,
        }))
    }

    /// Initialize surface for window rendering.
    pub fn init_surface<W>(&mut self, window: Arc<W>) -> GupResult<()>
    where
        W: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let surface = self
            ._instance
            .create_surface(window)
            .map_err(|e| GupError::WebGpuError(format!("Failed to create surface: {e}")))?;

        let surface_caps = surface.get_capabilities(&self._adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 800,
            height: 600,
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

    /// Begin frame rendering.
    pub fn begin_frame(&mut self) -> GupResult<RenderFrame> {
        self.frame_start_time = Some(Instant::now());

        let (surface_texture, render_target) = if let Some(surface) = &self.surface {
            let output = surface.get_current_texture().map_err(|e| {
                GupError::WebGpuError(format!("Failed to acquire surface texture: {e}"))
            })?;
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());
            (Some(output), view)
        } else {
            // Create offscreen render target for headless rendering
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("offscreen_render_target"),
                size: Extent3d {
                    width: 800,
                    height: 600,
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
            (None, view)
        };

        let command_encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("gup_frame_encoder"),
            });

        Ok(RenderFrame {
            context: self,
            surface_texture,
            render_target,
            command_encoder,
        })
    }

    /// Get current render target (if rendering to surface).
    pub fn current_render_target(&self) -> Option<TextureFormat> {
        self.surface_config.as_ref().map(|config| config.format)
    }

    /// Submit commands to GPU.
    pub fn submit<I: IntoIterator<Item = CommandBuffer>>(&self, commands: I) {
        self.queue.submit(commands);
    }

    /// Present frame (if using surface).
    pub fn present(&mut self) -> GupResult<()> {
        // Frame presentation is handled by RenderFrame::finish()
        Ok(())
    }

    /// Access buffer pool.
    pub fn buffer_pool(&mut self) -> &mut BufferPool {
        &mut self.buffer_pool
    }

    /// Access texture pool.
    pub fn texture_pool(&mut self) -> &mut TexturePool {
        &mut self.texture_pool
    }

    /// Resource creation shortcuts.
    pub fn create_buffer<T>(&mut self, buffer_type: BufferType, capacity: usize) -> GpuBuffer<T>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        self.buffer_pool.allocate(buffer_type, capacity)
    }

    /// Create texture with descriptor.
    pub fn create_texture(&mut self, descriptor: &TextureDescriptor) -> Texture {
        self.texture_pool.create_texture(descriptor)
    }

    /// Get performance monitoring statistics.
    pub fn frame_stats(&self) -> &FrameStats {
        &self.frame_stats
    }

    /// Reset performance statistics.
    pub fn reset_stats(&mut self) {
        self.frame_stats = FrameStats::default();
    }

    /// Get the surface format for pipeline creation.
    pub fn surface_format(&self) -> TextureFormat {
        self.surface_config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(TextureFormat::Bgra8UnormSrgb)
    }

    /// Resize the surface if one exists.
    pub fn resize_surface(&mut self, width: u32, height: u32) -> GupResult<()> {
        if let (Some(surface), Some(config)) = (&self.surface, &mut self.surface_config) {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        }
        Ok(())
    }

    /// Update frame statistics when frame completes.
    fn finish_frame(&mut self) {
        if let Some(start_time) = self.frame_start_time.take() {
            let frame_time = start_time.elapsed();
            self.frame_stats.update_frame_time(frame_time);

            // Update GPU memory usage from buffer pool stats
            let buffer_stats = self.buffer_pool.get_stats();
            self.frame_stats.gpu_memory_usage = buffer_stats.total_bytes_allocated;
        }
    }
}

/// Active render frame with automatic resource management.
pub struct RenderFrame<'a> {
    context: &'a mut GupContext,
    surface_texture: Option<SurfaceTexture>,
    render_target: TextureView,
    command_encoder: CommandEncoder,
}

impl<'a> RenderFrame<'a> {
    /// Create a render pass targeting the render target.
    pub fn render_pass(&mut self, clear_color: Option<Color>) -> RenderPass {
        let clear_value = clear_color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        self.command_encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("gup_render_pass"),
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

    /// Get reference to the render target.
    pub fn render_target(&self) -> &TextureView {
        &self.render_target
    }

    /// Get device reference.
    pub fn device(&self) -> &Device {
        &self.context.device
    }

    /// Get queue reference.
    pub fn queue(&self) -> &Queue {
        &self.context.queue
    }

    /// Finish the render frame and present if rendering to surface.
    pub fn finish(self) -> GupResult<()> {
        let command_buffer = self.command_encoder.finish();
        self.context.queue.submit(Some(command_buffer));

        if let Some(output) = self.surface_texture {
            output.present();
        }

        self.context.finish_frame();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_creation() {
        let context = GupContext::headless().await;
        assert!(context.is_ok());

        let ctx = context.unwrap();
        assert!(ctx.device.features().contains(Features::default()));
    }

    #[tokio::test]
    async fn test_context_sharing() {
        let context = GupContext::headless().await.unwrap();
        let context_clone = Arc::clone(&context);

        // Verify both references point to same underlying resources
        assert!(Arc::ptr_eq(&context.device, &context_clone.device));
    }

    #[tokio::test]
    async fn test_frame_lifecycle() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let frame = ctx.begin_frame().unwrap();
        frame.finish().unwrap();

        // Verify frame stats were updated
        assert!(ctx.frame_stats().frames_rendered > 0);
    }

    #[tokio::test]
    async fn test_buffer_creation_shortcut() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let buffer = ctx.create_buffer::<f32>(BufferType::Vertex, 100);
        assert_eq!(buffer.capacity(), 128); // Power of 2 rounded up
        assert_eq!(buffer.buffer_type(), BufferType::Vertex);
    }

    #[tokio::test]
    async fn test_custom_options() {
        let options = GupOptions {
            power_preference: PowerPreference::LowPower,
            required_features: Features::empty(),
            ..Default::default()
        };

        let context = GupContext::with_options(options).await;
        assert!(context.is_ok());
    }

    #[tokio::test]
    async fn test_frame_stats_tracking() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Render a few frames
        for _ in 0..3 {
            let frame = ctx.begin_frame().unwrap();
            frame.finish().unwrap();
        }

        let stats = ctx.frame_stats();
        assert_eq!(stats.frames_rendered, 3);
        assert!(stats.avg_frame_time >= 0.0);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn test_wasm_context_creation() {
        let context = GupContext::new().await;
        assert!(context.is_ok());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_native_context_creation() {
        let context = GupContext::new().await;
        assert!(context.is_ok());
    }
}
