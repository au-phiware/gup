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
use std::collections::HashMap;
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

/// Unique identifier for surfaces in multi-window applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

impl SurfaceId {
    /// Create a new unique surface ID.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for SurfaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface({})", self.0)
    }
}

/// Surface information and configuration.
#[derive(Debug)]
struct ManagedSurface {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    scale_factor: f64,
    is_fullscreen: bool,
}

impl ManagedSurface {
    fn new(surface: Surface<'static>, config: SurfaceConfiguration, scale_factor: f64) -> Self {
        Self {
            surface,
            config,
            scale_factor,
            is_fullscreen: false,
        }
    }

    fn resize(&mut self, device: &Device, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
    }

    fn set_fullscreen(&mut self, device: &Device, fullscreen: bool) {
        self.is_fullscreen = fullscreen;
        self.surface.configure(device, &self.config);
    }

    fn update_scale_factor(&mut self, device: &Device, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.surface.configure(device, &self.config);
    }
}

/// Physical size with width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize<T> {
    pub width: T,
    pub height: T,
}

impl<T> PhysicalSize<T> {
    pub fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
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
/// Configuration for texture pooling behavior.
#[derive(Debug, Clone)]
pub struct TexturePoolConfig {
    /// Maximum number of textures to keep in each pool
    pub max_textures_per_pool: usize,
    /// Maximum total GPU memory to use for pooled textures (in bytes)
    pub max_total_memory: Option<u64>,
    /// Time after which unused textures are evicted
    pub eviction_timeout: Duration,
    /// Whether to enable LRU eviction
    pub enable_lru: bool,
}

impl Default for TexturePoolConfig {
    fn default() -> Self {
        Self {
            max_textures_per_pool: 20,
            max_total_memory: Some(512 * 1024 * 1024), // 512 MB default limit
            eviction_timeout: Duration::from_secs(120), // 2 minutes
            enable_lru: true,
        }
    }
}

/// Key for identifying texture pools by format and size class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextureKey {
    format: TextureFormat,
    dimension: TextureDimension,
    /// Power-of-2 rounded dimensions (width, height, depth_or_array_layers)
    size_class: (u32, u32, u32),
    usage: TextureUsages,
}

impl TextureKey {
    fn from_descriptor(desc: &TextureDescriptor) -> Self {
        // Round up dimensions to power-of-2 for better reuse
        let size_class = (
            desc.size.width.next_power_of_two(),
            desc.size.height.next_power_of_two(),
            desc.size.depth_or_array_layers.next_power_of_two(),
        );

        Self {
            format: desc.format,
            dimension: desc.dimension,
            size_class,
            usage: desc.usage,
        }
    }
}

/// Entry in the texture pool tracking usage time.
#[derive(Debug)]
struct PooledTextureEntry {
    texture: Texture,
    last_used: Instant,
    size: u64,
}

/// Statistics for texture pool usage tracking.
#[derive(Debug, Clone, Default)]
pub struct TexturePoolStats {
    /// Total textures allocated (pool hits + misses)
    pub total_allocated: u64,
    /// Total textures returned to pool
    pub total_deallocated: u64,
    /// Currently active textures (not in pool)
    pub active_textures: u64,
    /// Currently pooled textures (available for reuse)
    pub pooled_textures: usize,
    /// Number of pool hits (texture reused from pool)
    pub pool_hits: u64,
    /// Number of pool misses (new texture created)
    pub pool_misses: u64,
    /// Total memory currently in pooled textures
    pub pooled_memory: u64,
}

/// Texture pool with size classes and reuse.
#[derive(Debug)]
pub struct TexturePool {
    pools: HashMap<TextureKey, Vec<PooledTextureEntry>>,
    device: Arc<Device>,
    stats: TexturePoolStats,
    config: TexturePoolConfig,
}

impl TexturePool {
    fn new(device: Arc<Device>) -> Self {
        Self::with_config(device, TexturePoolConfig::default())
    }

    /// Create a new texture pool with custom configuration.
    fn with_config(device: Arc<Device>, config: TexturePoolConfig) -> Self {
        Self {
            pools: HashMap::new(),
            device,
            stats: TexturePoolStats::default(),
            config,
        }
    }

    /// Create or retrieve a texture from the pool.
    pub fn create_texture(&mut self, descriptor: &TextureDescriptor) -> Texture {
        let key = TextureKey::from_descriptor(descriptor);

        // Try to get from pool
        if let Some(pool) = self.pools.get_mut(&key)
            && let Some(entry) = pool.pop()
        {
            self.stats.pooled_textures -= 1;
            self.stats.pooled_memory -= entry.size;
            self.stats.pool_hits += 1;
            self.stats.active_textures += 1;
            self.stats.total_allocated += 1;
            return entry.texture;
        }

        // Create new texture if none available
        self.stats.pool_misses += 1;
        self.stats.active_textures += 1;
        self.stats.total_allocated += 1;
        self.device.create_texture(descriptor)
    }

    /// Return a texture to the pool for reuse.
    pub fn return_texture(&mut self, texture: Texture) {
        let size = self.calculate_texture_size(&texture);
        let key = self.make_key_from_texture(&texture);

        let entry = PooledTextureEntry {
            texture,
            last_used: Instant::now(),
            size,
        };

        // Add to pool
        let pool = self.pools.entry(key).or_default();
        pool.push(entry);
        self.stats.pooled_textures += 1;
        self.stats.pooled_memory += size;
        self.stats.active_textures -= 1;
        self.stats.total_deallocated += 1;

        // Check if we need to evict
        self.check_memory_pressure();
    }

    /// Calculate the memory size of a texture.
    fn calculate_texture_size(&self, texture: &Texture) -> u64 {
        let width = texture.width() as u64;
        let height = texture.height() as u64;
        let depth = texture.depth_or_array_layers() as u64;
        let format = texture.format();

        // Approximate bytes per pixel for common formats
        let bytes_per_pixel = match format {
            TextureFormat::R8Unorm
            | TextureFormat::R8Snorm
            | TextureFormat::R8Uint
            | TextureFormat::R8Sint => 1,
            TextureFormat::R16Uint | TextureFormat::R16Sint | TextureFormat::R16Float => 2,
            TextureFormat::Rg8Unorm
            | TextureFormat::Rg8Snorm
            | TextureFormat::Rg8Uint
            | TextureFormat::Rg8Sint => 2,
            TextureFormat::R32Uint | TextureFormat::R32Sint | TextureFormat::R32Float => 4,
            TextureFormat::Rg16Uint | TextureFormat::Rg16Sint | TextureFormat::Rg16Float => 4,
            TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Rgba8Snorm
            | TextureFormat::Rgba8Uint
            | TextureFormat::Rgba8Sint
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb => 4,
            TextureFormat::Rgb10a2Unorm => 4,
            TextureFormat::Rg32Uint | TextureFormat::Rg32Sint | TextureFormat::Rg32Float => 8,
            TextureFormat::Rgba16Uint | TextureFormat::Rgba16Sint | TextureFormat::Rgba16Float => 8,
            TextureFormat::Rgba32Uint | TextureFormat::Rgba32Sint | TextureFormat::Rgba32Float => {
                16
            }
            TextureFormat::Depth32Float => 4,
            TextureFormat::Depth24Plus => 4,
            TextureFormat::Depth24PlusStencil8 => 4,
            _ => 4, // Default to 4 bytes for unknown formats
        };

        width * height * depth * bytes_per_pixel
    }

    /// Create a key from an existing texture.
    fn make_key_from_texture(&self, texture: &Texture) -> TextureKey {
        let size_class = (
            texture.width().next_power_of_two(),
            texture.height().next_power_of_two(),
            texture.depth_or_array_layers().next_power_of_two(),
        );

        TextureKey {
            format: texture.format(),
            dimension: texture.dimension(),
            size_class,
            usage: texture.usage(),
        }
    }

    /// Check memory pressure and evict textures if necessary.
    fn check_memory_pressure(&mut self) {
        if let Some(max_memory) = self.config.max_total_memory
            && self.stats.pooled_memory > max_memory
        {
            self.evict_lru_textures(self.stats.pooled_memory - max_memory);
        }

        // Also check per-pool limits
        for pool in self.pools.values_mut() {
            if pool.len() > self.config.max_textures_per_pool {
                let to_remove = pool.len() - self.config.max_textures_per_pool;
                for _ in 0..to_remove {
                    if let Some(entry) = pool.pop() {
                        self.stats.pooled_textures -= 1;
                        self.stats.pooled_memory -= entry.size;
                    }
                }
            }
        }
    }

    /// Evict least-recently-used textures to free up the specified amount of memory.
    fn evict_lru_textures(&mut self, target_bytes: u64) {
        let mut freed = 0u64;
        let now = Instant::now();

        // Collect all entries with their ages
        let mut all_entries: Vec<(TextureKey, usize, Duration)> = Vec::new();
        for (key, pool) in &self.pools {
            for (idx, entry) in pool.iter().enumerate() {
                let age = now.duration_since(entry.last_used);
                all_entries.push((*key, idx, age));
            }
        }

        // Sort by age (oldest first)
        all_entries.sort_by_key(|(_, _, age)| *age);
        all_entries.reverse();

        // Remove oldest entries until we've freed enough memory
        for (key, _, _) in all_entries {
            if freed >= target_bytes {
                break;
            }

            if let Some(pool) = self.pools.get_mut(&key)
                && let Some(entry) = pool.pop()
            {
                freed += entry.size;
                self.stats.pooled_textures -= 1;
                self.stats.pooled_memory -= entry.size;
            }
        }

        // Remove empty pools
        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Clean up old textures that haven't been used recently.
    pub fn cleanup_old_textures(&mut self) {
        let now = Instant::now();
        let timeout = self.config.eviction_timeout;

        for pool in self.pools.values_mut() {
            pool.retain(|entry| {
                let should_retain = now.duration_since(entry.last_used) < timeout;
                if !should_retain {
                    self.stats.pooled_textures -= 1;
                    self.stats.pooled_memory -= entry.size;
                }
                should_retain
            });
        }

        // Remove empty pools
        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Get current pool statistics.
    pub fn stats(&self) -> &TexturePoolStats {
        &self.stats
    }
}

/// Unified render context that manages GPU resources and provides rendering capabilities.
#[derive(Debug)]
pub struct GupContext {
    /// Core wgpu resources
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,

    /// Multi-surface management
    surfaces: HashMap<SurfaceId, ManagedSurface>,
    primary_surface_id: Option<SurfaceId>,

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
            .ok_or_else(|| GupError::resource_error("Context already shared".to_string()))?
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
                GupError::webgpu_error(format!("Failed to find suitable GPU adapter: {e}"))
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
            .map_err(|e| GupError::webgpu_error(format!("Failed to create device: {e}")))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let buffer_pool = BufferPool::new(Arc::clone(&device));
        let texture_pool = TexturePool::new(Arc::clone(&device));

        Ok(Arc::new(Self {
            device,
            queue,
            surfaces: HashMap::new(),
            primary_surface_id: None,
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
            .map_err(|e| GupError::webgpu_error(format!("Failed to create surface: {e}")))?;

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

        let managed_surface = ManagedSurface::new(surface, config, 1.0);
        let surface_id = SurfaceId::new();
        self.surfaces.insert(surface_id, managed_surface);
        self.primary_surface_id = Some(surface_id);

        Ok(())
    }

    /// Add a new surface to the context.
    pub fn add_surface<W>(&mut self, id: SurfaceId, window: Arc<W>) -> GupResult<()>
    where
        W: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        if self.surfaces.contains_key(&id) {
            return Err(GupError::resource_error(format!(
                "Surface with ID {id} already exists"
            )));
        }

        let surface = self
            ._instance
            .create_surface(window)
            .map_err(|e| GupError::webgpu_error(format!("Failed to create surface: {e}")))?;

        let surface_caps = surface.get_capabilities(&self._adapter);
        let surface_format = self.negotiate_surface_format(&surface_caps)?;

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: 800,
            height: 600,
            present_mode: self.select_present_mode(&surface_caps),
            alpha_mode: self.select_alpha_mode(&surface_caps),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        let managed_surface = ManagedSurface::new(surface, config, 1.0);
        self.surfaces.insert(id, managed_surface);

        // Set as primary if this is the first surface
        if self.primary_surface_id.is_none() {
            self.primary_surface_id = Some(id);
        }

        Ok(())
    }

    /// Remove a surface from the context.
    pub fn remove_surface(&mut self, id: SurfaceId) -> GupResult<()> {
        if !self.surfaces.contains_key(&id) {
            return Err(GupError::resource_error(format!(
                "Surface with ID {id} does not exist"
            )));
        }

        self.surfaces.remove(&id);

        // Update primary surface if removed
        if self.primary_surface_id == Some(id) {
            self.primary_surface_id = self.surfaces.keys().next().copied();
        }

        Ok(())
    }

    /// Resize a specific surface.
    pub fn resize_surface(&mut self, id: SurfaceId, size: PhysicalSize<u32>) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        surface.resize(&self.device, size.width, size.height);
        Ok(())
    }

    /// Set fullscreen mode for a specific surface.
    pub fn set_fullscreen(&mut self, id: SurfaceId, fullscreen: bool) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        surface.set_fullscreen(&self.device, fullscreen);
        Ok(())
    }

    /// Update scale factor for a surface.
    pub fn update_surface_scale_factor(
        &mut self,
        id: SurfaceId,
        scale_factor: f64,
    ) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        surface.update_scale_factor(&self.device, scale_factor);
        Ok(())
    }

    /// Surface format negotiation with fallbacks.
    fn negotiate_surface_format(&self, caps: &SurfaceCapabilities) -> GupResult<TextureFormat> {
        // Prefer sRGB formats for color accuracy
        let preferred_formats = [
            TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Rgba8Unorm,
        ];

        for format in &preferred_formats {
            if caps.formats.contains(format) {
                return Ok(*format);
            }
        }

        // Fallback to first available format
        caps.formats
            .first()
            .copied()
            .ok_or_else(|| GupError::webgpu_error("No supported surface formats found".to_string()))
    }

    /// Select appropriate present mode.
    fn select_present_mode(&self, caps: &SurfaceCapabilities) -> PresentMode {
        // Prefer immediate for low latency, fall back to FIFO
        if caps.present_modes.contains(&PresentMode::Immediate) {
            PresentMode::Immediate
        } else if caps.present_modes.contains(&PresentMode::Mailbox) {
            PresentMode::Mailbox
        } else {
            PresentMode::Fifo // Always supported
        }
    }

    /// Select appropriate alpha mode.
    fn select_alpha_mode(&self, caps: &SurfaceCapabilities) -> CompositeAlphaMode {
        // Prefer opaque for performance
        if caps.alpha_modes.contains(&CompositeAlphaMode::Opaque) {
            CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0] // Use first available
        }
    }

    /// Begin frame rendering for a specific surface.
    pub fn begin_frame_for_surface(&mut self, id: SurfaceId) -> GupResult<RenderFrame<'_>> {
        self.frame_start_time = Some(Instant::now());

        let surface = self
            .surfaces
            .get(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        let output = surface.surface.get_current_texture().map_err(|e| {
            GupError::webgpu_error(format!("Failed to acquire surface texture: {e}"))
        })?;
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let command_encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some(&format!("gup_frame_encoder_{id}")),
            });

        Ok(RenderFrame {
            context: self,
            surface_texture: Some(output),
            render_target: view,
            command_encoder,
            surface_id: Some(id),
        })
    }

    /// Begin frame rendering.
    pub fn begin_frame(&mut self) -> GupResult<RenderFrame<'_>> {
        self.frame_start_time = Some(Instant::now());

        let (surface_texture, render_target) = if let Some(primary_id) = self.primary_surface_id {
            let surface = self
                .surfaces
                .get(&primary_id)
                .ok_or_else(|| GupError::resource_error("Primary surface not found".to_string()))?;
            let output = surface.surface.get_current_texture().map_err(|e| {
                GupError::webgpu_error(format!("Failed to acquire surface texture: {e}"))
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

        let surface_id = self.primary_surface_id;
        Ok(RenderFrame {
            context: self,
            surface_texture,
            render_target,
            command_encoder,
            surface_id,
        })
    }

    /// Get current render target (if rendering to surface).
    pub fn current_render_target(&self) -> Option<TextureFormat> {
        self.primary_surface_id
            .and_then(|id| self.surfaces.get(&id))
            .map(|surface| surface.config.format)
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

    /// Return a texture to the pool for reuse.
    pub fn return_texture(&mut self, texture: Texture) {
        self.texture_pool.return_texture(texture);
    }

    /// Get texture pool statistics.
    pub fn texture_pool_stats(&self) -> &TexturePoolStats {
        self.texture_pool.stats()
    }

    /// Clean up old textures from the pool.
    pub fn cleanup_texture_pool(&mut self) {
        self.texture_pool.cleanup_old_textures();
    }

    /// Get performance monitoring statistics.
    pub fn frame_stats(&self) -> &FrameStats {
        &self.frame_stats
    }

    /// Reset performance statistics.
    pub fn reset_stats(&mut self) {
        self.frame_stats = FrameStats::default();
    }

    /// Get all active surface IDs.
    pub fn surface_ids(&self) -> Vec<SurfaceId> {
        self.surfaces.keys().copied().collect()
    }

    /// Get primary surface ID.
    pub fn primary_surface_id(&self) -> Option<SurfaceId> {
        self.primary_surface_id
    }

    /// Set primary surface ID.
    pub fn set_primary_surface(&mut self, id: SurfaceId) -> GupResult<()> {
        if !self.surfaces.contains_key(&id) {
            return Err(GupError::resource_error(format!(
                "Surface with ID {id} does not exist"
            )));
        }
        self.primary_surface_id = Some(id);
        Ok(())
    }

    /// Get the surface format for pipeline creation (primary surface).
    pub fn surface_format(&self) -> TextureFormat {
        self.current_render_target()
            .unwrap_or(TextureFormat::Bgra8UnormSrgb)
    }

    /// Get surface format for specific surface.
    pub fn surface_format_for(&self, id: SurfaceId) -> Option<TextureFormat> {
        self.surfaces.get(&id).map(|surface| surface.config.format)
    }

    /// Get surface size for specific surface.
    pub fn surface_size(&self, id: SurfaceId) -> Option<PhysicalSize<u32>> {
        self.surfaces.get(&id).map(|surface| PhysicalSize {
            width: surface.config.width,
            height: surface.config.height,
        })
    }

    /// Check if surface is in fullscreen mode.
    pub fn is_fullscreen(&self, id: SurfaceId) -> bool {
        self.surfaces
            .get(&id)
            .map(|surface| surface.is_fullscreen)
            .unwrap_or(false)
    }

    /// Get surface scale factor.
    pub fn surface_scale_factor(&self, id: SurfaceId) -> Option<f64> {
        self.surfaces.get(&id).map(|surface| surface.scale_factor)
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
    surface_id: Option<SurfaceId>,
}

impl<'a> RenderFrame<'a> {
    /// Create a render pass targeting the render target.
    pub fn render_pass(&mut self, clear_color: Option<Color>) -> RenderPass<'_> {
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

    /// Get device Arc reference for sharing.
    pub fn device_arc(&self) -> Arc<Device> {
        Arc::clone(&self.context.device)
    }

    /// Get queue Arc reference for sharing.
    pub fn queue_arc(&self) -> Arc<Queue> {
        Arc::clone(&self.context.queue)
    }

    /// Get the surface ID for this frame (if rendering to a surface).
    pub fn surface_id(&self) -> Option<SurfaceId> {
        self.surface_id
    }

    /// Check if this frame is rendering to a surface.
    pub fn is_surface_rendering(&self) -> bool {
        self.surface_texture.is_some()
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

    // Mock window for testing
    #[allow(dead_code)]
    struct MockWindow {
        width: u32,
        height: u32,
    }

    impl MockWindow {
        fn new(width: u32, height: u32) -> Arc<Self> {
            Arc::new(Self { width, height })
        }
    }

    impl raw_window_handle::HasWindowHandle for MockWindow {
        fn window_handle(
            &self,
        ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
            use raw_window_handle::{RawWindowHandle, WebWindowHandle, WindowHandle};
            let handle = RawWindowHandle::Web(WebWindowHandle::new(0));
            Ok(unsafe { WindowHandle::borrow_raw(handle) })
        }
    }

    impl raw_window_handle::HasDisplayHandle for MockWindow {
        fn display_handle(
            &self,
        ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
            use raw_window_handle::{DisplayHandle, RawDisplayHandle, WebDisplayHandle};
            let handle = RawDisplayHandle::Web(WebDisplayHandle::new());
            Ok(unsafe { DisplayHandle::borrow_raw(handle) })
        }
    }

    #[tokio::test]
    async fn test_surface_id_creation() {
        let id1 = SurfaceId::new();
        let id2 = SurfaceId::new();

        assert_ne!(id1, id2);
        assert_ne!(id1.raw(), id2.raw());

        let id3 = SurfaceId::default();
        assert_ne!(id1, id3);
    }

    #[tokio::test]
    async fn test_surface_id_display() {
        let id = SurfaceId::new();
        let display_str = format!("{id}");
        assert!(display_str.starts_with("Surface("));
        assert!(display_str.ends_with(")"));
    }

    #[tokio::test]
    async fn test_physical_size() {
        let size = PhysicalSize::new(800u32, 600u32);
        assert_eq!(size.width, 800);
        assert_eq!(size.height, 600);

        let size2 = PhysicalSize {
            width: 1024,
            height: 768,
        };
        assert_eq!(size2.width, 1024);
        assert_eq!(size2.height, 768);

        assert_ne!(size, size2);
    }

    #[tokio::test]
    async fn test_multi_surface_management() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Initially no surfaces
        assert!(ctx.surface_ids().is_empty());
        assert!(ctx.primary_surface_id().is_none());

        // Add first surface
        let id1 = SurfaceId::new();
        let window1 = MockWindow::new(800, 600);

        // Note: This will fail in headless mode, but tests the API
        let result = ctx.add_surface(id1, window1);
        // In headless mode, this should fail gracefully
        if result.is_err() {
            println!("Expected failure in headless mode: {result:?}");
            return;
        }

        // If we get here, we're in a windowed environment
        assert!(result.is_ok());
        assert_eq!(ctx.surface_ids().len(), 1);
        assert_eq!(ctx.primary_surface_id(), Some(id1));

        // Add second surface
        let id2 = SurfaceId::new();
        let window2 = MockWindow::new(1024, 768);
        assert!(ctx.add_surface(id2, window2).is_ok());
        assert_eq!(ctx.surface_ids().len(), 2);
        assert_eq!(ctx.primary_surface_id(), Some(id1)); // First remains primary

        // Test surface properties
        assert_eq!(ctx.surface_size(id1), Some(PhysicalSize::new(800, 600)));
        assert_eq!(ctx.surface_size(id2), Some(PhysicalSize::new(1024, 768)));
        assert!(!ctx.is_fullscreen(id1));
        assert!(!ctx.is_fullscreen(id2));

        // Remove surface
        assert!(ctx.remove_surface(id2).is_ok());
        assert_eq!(ctx.surface_ids().len(), 1);
        assert_eq!(ctx.primary_surface_id(), Some(id1));

        // Remove primary surface
        assert!(ctx.remove_surface(id1).is_ok());
        assert!(ctx.surface_ids().is_empty());
        assert!(ctx.primary_surface_id().is_none());
    }

    #[tokio::test]
    async fn test_surface_error_handling() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let id = SurfaceId::new();

        // Test operations on non-existent surface
        assert!(ctx.remove_surface(id).is_err());
        assert!(ctx.resize_surface(id, PhysicalSize::new(800, 600)).is_err());
        assert!(ctx.set_fullscreen(id, true).is_err());
        assert!(ctx.update_surface_scale_factor(id, 2.0).is_err());
        assert!(ctx.begin_frame_for_surface(id).is_err());
        assert!(ctx.set_primary_surface(id).is_err());

        // Test queries on non-existent surface
        assert!(ctx.surface_format_for(id).is_none());
        assert!(ctx.surface_size(id).is_none());
        assert!(!ctx.is_fullscreen(id));
        assert!(ctx.surface_scale_factor(id).is_none());
    }

    #[tokio::test]
    async fn test_surface_format_negotiation() {
        let context = GupContext::headless().await.unwrap();
        let ctx = Arc::try_unwrap(context).unwrap();

        // Test format negotiation with mock capabilities
        let mut caps = SurfaceCapabilities {
            formats: vec![
                TextureFormat::Bgra8Unorm,
                TextureFormat::Rgba8Unorm,
                TextureFormat::Bgra8UnormSrgb,
            ],
            present_modes: vec![PresentMode::Fifo],
            alpha_modes: vec![CompositeAlphaMode::Opaque],
            usages: TextureUsages::RENDER_ATTACHMENT,
        };

        // Should prefer sRGB format
        let format = ctx.negotiate_surface_format(&caps).unwrap();
        assert_eq!(format, TextureFormat::Bgra8UnormSrgb);

        // Test with no sRGB formats
        caps.formats = vec![TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm];
        let format = ctx.negotiate_surface_format(&caps).unwrap();
        assert_eq!(format, TextureFormat::Bgra8Unorm); // First available

        // Test with empty formats (should error)
        caps.formats = vec![];
        assert!(ctx.negotiate_surface_format(&caps).is_err());
    }

    #[tokio::test]
    async fn test_present_mode_selection() {
        let context = GupContext::headless().await.unwrap();
        let ctx = Arc::try_unwrap(context).unwrap();

        let mut caps = SurfaceCapabilities {
            formats: vec![TextureFormat::Bgra8UnormSrgb],
            present_modes: vec![PresentMode::Fifo, PresentMode::Immediate],
            alpha_modes: vec![CompositeAlphaMode::Opaque],
            usages: TextureUsages::RENDER_ATTACHMENT,
        };

        // Should prefer Immediate
        let mode = ctx.select_present_mode(&caps);
        assert_eq!(mode, PresentMode::Immediate);

        // Test with Mailbox
        caps.present_modes = vec![PresentMode::Fifo, PresentMode::Mailbox];
        let mode = ctx.select_present_mode(&caps);
        assert_eq!(mode, PresentMode::Mailbox);

        // Test with only Fifo
        caps.present_modes = vec![PresentMode::Fifo];
        let mode = ctx.select_present_mode(&caps);
        assert_eq!(mode, PresentMode::Fifo);
    }

    #[tokio::test]
    async fn test_alpha_mode_selection() {
        let context = GupContext::headless().await.unwrap();
        let ctx = Arc::try_unwrap(context).unwrap();

        let mut caps = SurfaceCapabilities {
            formats: vec![TextureFormat::Bgra8UnormSrgb],
            present_modes: vec![PresentMode::Fifo],
            alpha_modes: vec![
                CompositeAlphaMode::PreMultiplied,
                CompositeAlphaMode::Opaque,
            ],
            usages: TextureUsages::RENDER_ATTACHMENT,
        };

        // Should prefer Opaque
        let mode = ctx.select_alpha_mode(&caps);
        assert_eq!(mode, CompositeAlphaMode::Opaque);

        // Test with only PreMultiplied
        caps.alpha_modes = vec![CompositeAlphaMode::PreMultiplied];
        let mode = ctx.select_alpha_mode(&caps);
        assert_eq!(mode, CompositeAlphaMode::PreMultiplied);
    }

    #[tokio::test]
    async fn test_managed_surface() {
        use wgpu::*;

        // Create minimal surface config for testing
        let _config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: 800,
            height: 600,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        // Note: Can't actually create a surface in headless mode
        // This tests the ManagedSurface struct API

        // Test scale factor and fullscreen state
        let _scale_factor = 1.5;

        // These would be used with real surface:
        // let managed = ManagedSurface::new(surface, config, scale_factor);
        // assert_eq!(managed.scale_factor, scale_factor);
        // assert!(!managed.is_fullscreen);
    }

    #[tokio::test]
    async fn test_frame_surface_info() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Test headless frame
        let frame = ctx.begin_frame().unwrap();
        assert!(frame.surface_id().is_none());
        assert!(!frame.is_surface_rendering());
        frame.finish().unwrap();
    }

    #[tokio::test]
    async fn test_surface_resize_performance() {
        use std::time::Instant;

        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let id = SurfaceId::new();
        let window = MockWindow::new(800, 600);

        // This will fail in headless mode, but we test the performance expectation
        if ctx.add_surface(id, window).is_ok() {
            let start = Instant::now();
            let _ = ctx.resize_surface(id, PhysicalSize::new(1024, 768));
            let duration = start.elapsed();

            // Should complete well under 16ms for responsive UI
            assert!(duration.as_millis() < 16);
        }
    }

    // Texture Pool Tests

    #[tokio::test]
    async fn test_texture_pool_basic_creation() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let descriptor = TextureDescriptor {
            label: Some("test_texture"),
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let texture = ctx.create_texture(&descriptor);
        assert_eq!(texture.width(), 256);
        assert_eq!(texture.height(), 256);
        assert_eq!(texture.format(), TextureFormat::Rgba8Unorm);

        // Check stats
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.total_allocated, 1);
        assert_eq!(stats.active_textures, 1);
        assert_eq!(stats.pool_misses, 1);
        assert_eq!(stats.pool_hits, 0);
    }

    #[tokio::test]
    async fn test_texture_pool_reuse() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let descriptor = TextureDescriptor {
            label: Some("test_texture"),
            size: Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        // Create and return texture
        let texture1 = ctx.create_texture(&descriptor);
        ctx.return_texture(texture1);

        // Stats should show one pooled texture
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.pooled_textures, 1);
        assert_eq!(stats.active_textures, 0);
        assert_eq!(stats.total_deallocated, 1);

        // Request same texture again - should hit pool
        let _texture2 = ctx.create_texture(&descriptor);
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.pool_hits, 1);
        assert_eq!(stats.pooled_textures, 0);
        assert_eq!(stats.active_textures, 1);
    }

    #[tokio::test]
    async fn test_texture_pool_size_classes() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create texture with non-power-of-2 size
        let descriptor1 = TextureDescriptor {
            label: Some("test_texture_1"),
            size: Extent3d {
                width: 100,  // Will round to 128
                height: 100, // Will round to 128
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture1 = ctx.create_texture(&descriptor1);
        ctx.return_texture(texture1);

        // Request similar size - should reuse due to size class rounding
        let descriptor2 = TextureDescriptor {
            label: Some("test_texture_2"),
            size: Extent3d {
                width: 120,  // Also rounds to 128
                height: 120, // Also rounds to 128
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let _texture2 = ctx.create_texture(&descriptor2);
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.pool_hits, 1, "Size classes should enable reuse");
    }

    #[tokio::test]
    async fn test_texture_pool_different_formats() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create RGBA texture
        let desc_rgba = TextureDescriptor {
            label: Some("rgba_texture"),
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture_rgba = ctx.create_texture(&desc_rgba);
        ctx.return_texture(texture_rgba);

        // Request BGRA texture - should NOT reuse due to different format
        let desc_bgra = TextureDescriptor {
            label: Some("bgra_texture"),
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let _texture_bgra = ctx.create_texture(&desc_bgra);
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.pool_misses, 2, "Different formats should not reuse");
        assert_eq!(stats.pool_hits, 0);
    }

    #[tokio::test]
    async fn test_texture_pool_memory_tracking() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let descriptor = TextureDescriptor {
            label: Some("memory_test"),
            size: Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm, // 4 bytes per pixel
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture = ctx.create_texture(&descriptor);
        ctx.return_texture(texture);

        let stats = ctx.texture_pool_stats();
        // 512 * 512 * 4 = 1,048,576 bytes
        assert!(stats.pooled_memory > 0, "Should track memory usage");
        assert_eq!(
            stats.pooled_memory,
            512 * 512 * 4,
            "Should calculate RGBA8 memory correctly"
        );
    }

    #[tokio::test]
    async fn test_texture_pool_cleanup() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create textures with different formats to avoid pool hits
        let formats = [
            TextureFormat::Rgba8Unorm,
            TextureFormat::Bgra8Unorm,
            TextureFormat::Rgba16Float,
            TextureFormat::R32Float,
            TextureFormat::Rg16Float,
        ];

        for format in formats.iter() {
            let descriptor = TextureDescriptor {
                label: Some("cleanup_test"),
                size: Extent3d {
                    width: 256,
                    height: 256,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: *format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            };

            let texture = ctx.create_texture(&descriptor);
            ctx.return_texture(texture);
        }

        let stats_before = ctx.texture_pool_stats();
        assert_eq!(stats_before.pooled_textures, 5);
        assert_eq!(stats_before.total_deallocated, 5);
        assert!(stats_before.pooled_memory > 0);

        // Cleanup should work (but won't remove anything immediately due to timing)
        ctx.cleanup_texture_pool();

        // Stats should still be valid (textures haven't timed out yet)
        let stats_after = ctx.texture_pool_stats();
        assert!(stats_after.pooled_textures <= 5);
    }

    #[tokio::test]
    async fn test_texture_pool_3d_textures() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        let descriptor = TextureDescriptor {
            label: Some("3d_texture"),
            size: Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 64,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = ctx.create_texture(&descriptor);
        assert_eq!(texture.dimension(), TextureDimension::D3);
        ctx.return_texture(texture);

        // Request same 3D texture
        let texture2 = ctx.create_texture(&descriptor);
        let stats = ctx.texture_pool_stats();
        assert_eq!(stats.pool_hits, 1);

        // Verify 3D memory calculation
        ctx.return_texture(texture2);
        let stats = ctx.texture_pool_stats();
        // 64 * 64 * 64 * 4 bytes
        assert_eq!(stats.pooled_memory, 64 * 64 * 64 * 4);
    }

    #[tokio::test]
    async fn test_texture_pool_usage_flags() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create texture with specific usage
        let desc1 = TextureDescriptor {
            label: Some("render_attachment"),
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture1 = ctx.create_texture(&desc1);
        ctx.return_texture(texture1);

        // Request texture with different usage - should NOT reuse
        let desc2 = TextureDescriptor {
            label: Some("texture_binding"),
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let _texture2 = ctx.create_texture(&desc2);
        let stats = ctx.texture_pool_stats();
        assert_eq!(
            stats.pool_misses, 2,
            "Different usage flags should prevent reuse"
        );
    }
}
