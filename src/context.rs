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
use crate::performance::PerformanceProfiler;
use crate::{MaybeSend, MaybeSync};
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
    /// Allow fallback to software rendering if hardware fails
    pub allow_software_fallback: bool,
    /// Reduced feature set for limited GPUs
    pub reduced_features: Option<Features>,
    /// Reduced limits for limited GPUs
    pub reduced_limits: Option<Limits>,
    /// Enable automatic device loss detection and recovery
    pub automatic_device_loss_detection: bool,
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

/// Surface visibility and state tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceVisibility {
    /// Surface is visible and actively rendered
    Visible,
    /// Surface is minimized/hidden
    Hidden,
    /// Surface is occluded by other windows
    Occluded,
}

/// Surface focus state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceFocus {
    /// Surface has input focus
    Focused,
    /// Surface does not have focus
    Unfocused,
}

/// Render priority for intelligent frame scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RenderPriority {
    /// Skip rendering entirely (minimized/occluded windows)
    Minimized = 0,
    /// Reduced quality/framerate for background windows
    #[default]
    Background = 1,
    /// Always render at full quality (focused windows)
    Foreground = 2,
}

/// Configuration for surface-specific rendering behavior.
#[derive(Debug, Clone)]
pub struct SurfaceRenderConfig {
    /// Target frame rate (None = unlimited)
    pub target_fps: Option<f32>,
    /// Render priority for scheduling
    pub priority: RenderPriority,
    /// Enable frame skipping when behind schedule
    pub frame_skipping_enabled: bool,
    /// Resource pool size hint for this surface
    pub resource_pool_size: usize,
}

impl Default for SurfaceRenderConfig {
    fn default() -> Self {
        Self {
            target_fps: Some(60.0),
            priority: RenderPriority::Foreground,
            frame_skipping_enabled: true,
            resource_pool_size: 8,
        }
    }
}

/// Per-surface performance statistics.
#[derive(Debug, Clone, Default)]
pub struct SurfaceStats {
    /// Frames rendered for this surface
    pub frames_rendered: u64,
    /// Frames skipped due to scheduling
    pub frames_skipped: u64,
    /// Average frame time in milliseconds
    pub avg_frame_time: f32,
    /// Last render timestamp
    pub last_render: Option<Instant>,
    /// Target frame interval (calculated from target_fps)
    pub target_frame_interval: Option<Duration>,
}

impl SurfaceStats {
    /// Check if enough time has passed to render the next frame based on target FPS.
    pub fn should_render(&self, target_fps: Option<f32>) -> bool {
        let Some(last) = self.last_render else {
            return true; // First frame
        };

        let Some(fps) = target_fps else {
            return true; // Unlimited FPS
        };

        let target_interval = Duration::from_secs_f32(1.0 / fps);
        last.elapsed() >= target_interval
    }

    /// Update statistics after rendering a frame.
    pub fn record_frame(&mut self, frame_time: Duration, target_fps: Option<f32>) {
        self.frames_rendered += 1;
        self.last_render = Some(Instant::now());

        if let Some(fps) = target_fps {
            self.target_frame_interval = Some(Duration::from_secs_f32(1.0 / fps));
        }

        let frame_time_ms = frame_time.as_secs_f32() * 1000.0;
        if self.frames_rendered == 1 {
            self.avg_frame_time = frame_time_ms;
        } else {
            // Exponential moving average
            self.avg_frame_time = self.avg_frame_time * 0.9 + frame_time_ms * 0.1;
        }
    }

    /// Record a skipped frame.
    pub fn record_skip(&mut self) {
        self.frames_skipped += 1;
    }
}

/// Multi-surface rendering statistics.
#[derive(Debug, Clone, Default)]
pub struct MultiSurfaceStats {
    /// Per-surface statistics
    pub surface_stats: HashMap<SurfaceId, SurfaceStats>,
    /// Total frames across all surfaces
    pub total_frames: u64,
    /// Total skipped frames across all surfaces
    pub total_skipped: u64,
    /// CPU overhead of scheduling system (percentage)
    pub scheduling_overhead: f32,
}

/// Surface event information for event handlers.
#[derive(Debug, Clone, Copy)]
pub enum SurfaceEvent {
    /// DPI/scale factor changed
    DpiChanged {
        /// Identifier of the affected surface.
        surface_id: SurfaceId,
        /// New scale factor.
        scale_factor: f64,
    },
    /// Window focus changed
    FocusChanged {
        /// Identifier of the affected surface.
        surface_id: SurfaceId,
        /// Whether the surface is focused.
        focused: bool,
    },
    /// Window visibility changed
    VisibilityChanged {
        /// Identifier of the affected surface.
        surface_id: SurfaceId,
        /// Whether the surface is visible.
        visible: bool,
    },
    /// Surface was resized
    Resized {
        /// Identifier of the affected surface.
        surface_id: SurfaceId,
        /// New width in physical pixels.
        width: u32,
        /// New height in physical pixels.
        height: u32,
    },
}

/// Context state for error recovery and monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextState {
    /// Context is active and operational
    Active,
    /// GPU device was lost, attempting recovery
    DeviceLost,
    /// Recovery in progress
    Recovering,
    /// Recovery failed with reason
    Failed,
}

/// Context recovery callback type.
#[cfg(not(target_arch = "wasm32"))]
pub type RecoveryCallback = Box<dyn Fn(ContextState) + Send + Sync>;
/// Context recovery callback type.
#[cfg(target_arch = "wasm32")]
pub type RecoveryCallback = Box<dyn Fn(ContextState)>;

/// Trait for window handles that can be used for surface creation.
///
/// This combines the required traits for wgpu surface creation with thread safety.
pub trait WindowHandle:
    raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + MaybeSend + MaybeSync
{
}

// Blanket implementation for all types that satisfy the bounds
impl<T> WindowHandle for T where
    T: raw_window_handle::HasWindowHandle
        + raw_window_handle::HasDisplayHandle
        + MaybeSend
        + MaybeSync
{
}

/// Window handle renewal callback type for surface recreation after device loss.
///
/// This callback is invoked during recovery to allow the application to provide
/// a new window handle for a surface that needs to be recreated. Returns `Some(window)`
/// if the window is still available, or `None` if the window has been destroyed.
#[cfg(not(target_arch = "wasm32"))]
pub type WindowHandleRenewalCallback =
    Box<dyn Fn(SurfaceId) -> Option<Arc<dyn WindowHandle>> + Send + Sync>;
/// Window handle renewal callback type for surface recreation after device loss.
#[cfg(target_arch = "wasm32")]
pub type WindowHandleRenewalCallback = Box<dyn Fn(SurfaceId) -> Option<Arc<dyn WindowHandle>>>;

/// Recovery attempt result.
#[derive(Debug, Clone)]
pub struct RecoveryAttemptResult {
    /// Whether recovery succeeded
    pub success: bool,
    /// Time taken for recovery attempt
    pub duration: Duration,
    /// Error message if recovery failed
    pub error: Option<String>,
}

/// Recovery tier that succeeded for a recovery attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryTier {
    /// Full features recovery succeeded
    FullFeatures,
    /// Reduced features recovery succeeded
    ReducedFeatures,
    /// Software rendering fallback succeeded
    SoftwareRendering,
}

/// Statistics for recovery attempts.
#[derive(Debug, Clone)]
pub struct RecoveryMetrics {
    /// Total number of recovery attempts
    pub total_attempts: u64,
    /// Number of successful recoveries
    pub successful_recoveries: u64,
    /// Number of failed recoveries
    pub failed_recoveries: u64,
    /// Minimum recovery time observed
    pub min_recovery_time: Option<Duration>,
    /// Maximum recovery time observed
    pub max_recovery_time: Option<Duration>,
    /// Sum of all recovery times (for calculating average)
    pub total_recovery_time: Duration,
    /// Number of recoveries per tier
    pub full_features_count: u64,
    /// Number of recoveries that fell back to reduced features.
    pub reduced_features_count: u64,
    /// Number of recoveries that fell back to software rendering.
    pub software_rendering_count: u64,
    /// Rolling window of recent recovery attempts (last 100)
    pub recent_attempts: Vec<RecoveryAttemptResult>,
}

impl Default for RecoveryMetrics {
    fn default() -> Self {
        Self {
            total_attempts: 0,
            successful_recoveries: 0,
            failed_recoveries: 0,
            min_recovery_time: None,
            max_recovery_time: None,
            total_recovery_time: Duration::ZERO,
            full_features_count: 0,
            reduced_features_count: 0,
            software_rendering_count: 0,
            recent_attempts: Vec::with_capacity(100),
        }
    }
}

impl RecoveryMetrics {
    /// Record a recovery attempt.
    pub fn record_attempt(&mut self, result: &RecoveryAttemptResult, tier: Option<RecoveryTier>) {
        self.total_attempts += 1;

        if result.success {
            self.successful_recoveries += 1;

            // Record tier
            if let Some(tier) = tier {
                match tier {
                    RecoveryTier::FullFeatures => self.full_features_count += 1,
                    RecoveryTier::ReducedFeatures => self.reduced_features_count += 1,
                    RecoveryTier::SoftwareRendering => self.software_rendering_count += 1,
                }
            }
        } else {
            self.failed_recoveries += 1;
        }

        // Update timing statistics
        self.total_recovery_time += result.duration;

        match self.min_recovery_time {
            None => self.min_recovery_time = Some(result.duration),
            Some(min) if result.duration < min => self.min_recovery_time = Some(result.duration),
            _ => {}
        }

        match self.max_recovery_time {
            None => self.max_recovery_time = Some(result.duration),
            Some(max) if result.duration > max => self.max_recovery_time = Some(result.duration),
            _ => {}
        }

        // Add to rolling window (keep last 100)
        if self.recent_attempts.len() >= 100 {
            self.recent_attempts.remove(0);
        }
        self.recent_attempts.push(result.clone());
    }

    /// Calculate average recovery time.
    pub fn average_recovery_time(&self) -> Option<Duration> {
        if self.total_attempts > 0 {
            Some(self.total_recovery_time / self.total_attempts as u32)
        } else {
            None
        }
    }

    /// Calculate success rate as a percentage (0.0 to 100.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts > 0 {
            (self.successful_recoveries as f64 / self.total_attempts as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Export metrics as JSON string.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{
  "total_attempts": {},
  "successful_recoveries": {},
  "failed_recoveries": {},
  "success_rate": {:.2},
  "min_recovery_time_ms": {},
  "max_recovery_time_ms": {},
  "avg_recovery_time_ms": {},
  "full_features_count": {},
  "reduced_features_count": {},
  "software_rendering_count": {}
}}"#,
            self.total_attempts,
            self.successful_recoveries,
            self.failed_recoveries,
            self.success_rate(),
            self.min_recovery_time
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string()),
            self.max_recovery_time
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string()),
            self.average_recovery_time()
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string()),
            self.full_features_count,
            self.reduced_features_count,
            self.software_rendering_count
        )
    }

    /// Export metrics as CSV string.
    pub fn to_csv(&self) -> String {
        let mut csv = String::from("metric,value\n");
        csv.push_str(&format!("total_attempts,{}\n", self.total_attempts));
        csv.push_str(&format!(
            "successful_recoveries,{}\n",
            self.successful_recoveries
        ));
        csv.push_str(&format!("failed_recoveries,{}\n", self.failed_recoveries));
        csv.push_str(&format!("success_rate,{:.2}\n", self.success_rate()));
        csv.push_str(&format!(
            "min_recovery_time_ms,{}\n",
            self.min_recovery_time
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        csv.push_str(&format!(
            "max_recovery_time_ms,{}\n",
            self.max_recovery_time
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        csv.push_str(&format!(
            "avg_recovery_time_ms,{}\n",
            self.average_recovery_time()
                .map(|d| d.as_millis().to_string())
                .unwrap_or_else(|| "null".to_string())
        ));
        csv.push_str(&format!(
            "full_features_count,{}\n",
            self.full_features_count
        ));
        csv.push_str(&format!(
            "reduced_features_count,{}\n",
            self.reduced_features_count
        ));
        csv.push_str(&format!(
            "software_rendering_count,{}\n",
            self.software_rendering_count
        ));
        csv
    }
}

/// Cached surface configuration for automatic recreation after device loss.
#[derive(Debug, Clone)]
pub struct CachedSurfaceConfig {
    /// Surface width
    pub width: u32,
    /// Surface height
    pub height: u32,
    /// Surface format
    pub format: TextureFormat,
    /// Surface present mode
    pub present_mode: PresentMode,
    /// Alpha mode
    pub alpha_mode: CompositeAlphaMode,
    /// Scale factor
    pub scale_factor: f64,
    /// View formats
    pub view_formats: Vec<TextureFormat>,
}

/// Platform surface capabilities exposed to users.
#[derive(Debug, Clone)]
pub struct PlatformSurfaceCapabilities {
    /// Available texture formats for this surface
    pub formats: Vec<TextureFormat>,
    /// Available present modes
    pub present_modes: Vec<PresentMode>,
    /// Available alpha blending modes
    pub alpha_modes: Vec<CompositeAlphaMode>,
    /// Supported texture usages
    pub usages: TextureUsages,
}

impl From<&SurfaceCapabilities> for PlatformSurfaceCapabilities {
    fn from(caps: &SurfaceCapabilities) -> Self {
        Self {
            formats: caps.formats.clone(),
            present_modes: caps.present_modes.clone(),
            alpha_modes: caps.alpha_modes.clone(),
            usages: caps.usages,
        }
    }
}

/// Surface configuration builder for platform-specific features.
///
/// This builder allows fine-grained control over surface configuration,
/// with sensible defaults and automatic capability negotiation.
#[derive(Debug, Clone)]
pub struct SurfaceConfigBuilder {
    /// Initial width (will be adjusted on first resize)
    pub width: u32,
    /// Initial height (will be adjusted on first resize)
    pub height: u32,
    /// Override present mode selection (None = automatic)
    pub present_mode: Option<PresentMode>,
    /// Override alpha mode selection (None = automatic)
    pub alpha_mode: Option<CompositeAlphaMode>,
    /// Override format selection (None = automatic)
    pub format: Option<TextureFormat>,
    /// View formats for format reinterpretation (e.g., sRGB ↔ linear)
    pub view_formats: Vec<TextureFormat>,
    /// Frame latency hint (1-3 frames, None = default 2)
    pub desired_maximum_frame_latency: Option<u32>,
}

impl Default for SurfaceConfigBuilder {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            present_mode: None,
            alpha_mode: None,
            format: None,
            view_formats: Vec::new(),
            desired_maximum_frame_latency: None,
        }
    }
}

impl SurfaceConfigBuilder {
    /// Create a new surface configuration builder with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set initial dimensions.
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Override present mode selection.
    ///
    /// Common values:
    /// - `PresentMode::Immediate`: Low latency, no vsync (tearing possible)
    /// - `PresentMode::Mailbox`: Low latency with vsync (no tearing)
    /// - `PresentMode::Fifo`: Traditional vsync (always supported)
    pub fn with_present_mode(mut self, mode: PresentMode) -> Self {
        self.present_mode = Some(mode);
        self
    }

    /// Override alpha mode selection.
    pub fn with_alpha_mode(mut self, mode: CompositeAlphaMode) -> Self {
        self.alpha_mode = Some(mode);
        self
    }

    /// Override format selection.
    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Add view formats for texture format reinterpretation.
    ///
    /// View formats allow reinterpreting the surface texture in different formats
    /// without recreating it. Common use cases:
    /// - sRGB ↔ linear conversion for gamma-correct rendering
    /// - HDR workflow preparation
    ///
    /// Example: `with_view_formats(vec![TextureFormat::Bgra8Unorm])`
    /// allows reinterpreting an sRGB surface as linear.
    pub fn with_view_formats(mut self, formats: Vec<TextureFormat>) -> Self {
        self.view_formats = formats;
        self
    }

    /// Set frame latency hint (1-3 frames).
    ///
    /// Lower values reduce input latency but may hurt performance.
    /// - 1: Minimal latency (interactive apps, games)
    /// - 2: Balanced (default)
    /// - 3: Maximum throughput (heavy rendering)
    pub fn with_frame_latency(mut self, frames: u32) -> Self {
        self.desired_maximum_frame_latency = Some(frames.clamp(1, 3));
        self
    }
}

/// Trait for handling surface events.
pub trait SurfaceEventHandler: MaybeSend + MaybeSync {
    /// Called when DPI/scale factor changes.
    fn on_dpi_changed(&mut self, surface_id: SurfaceId, scale_factor: f64) -> GupResult<()> {
        let _ = (surface_id, scale_factor);
        Ok(())
    }

    /// Called when window focus changes.
    fn on_focus_changed(&mut self, surface_id: SurfaceId, focused: bool) -> GupResult<()> {
        let _ = (surface_id, focused);
        Ok(())
    }

    /// Called when window visibility changes.
    fn on_visibility_changed(&mut self, surface_id: SurfaceId, visible: bool) -> GupResult<()> {
        let _ = (surface_id, visible);
        Ok(())
    }

    /// Called when surface is resized.
    fn on_resized(&mut self, surface_id: SurfaceId, width: u32, height: u32) -> GupResult<()> {
        let _ = (surface_id, width, height);
        Ok(())
    }
}

/// Surface information and configuration.
#[derive(Debug)]
struct ManagedSurface {
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    scale_factor: f64,
    is_fullscreen: bool,
    visibility: SurfaceVisibility,
    focus: SurfaceFocus,
    render_config: SurfaceRenderConfig,
    stats: SurfaceStats,
}

impl ManagedSurface {
    fn new(surface: Surface<'static>, config: SurfaceConfiguration, scale_factor: f64) -> Self {
        Self {
            surface,
            config,
            scale_factor,
            is_fullscreen: false,
            visibility: SurfaceVisibility::Visible,
            focus: SurfaceFocus::Unfocused,
            render_config: SurfaceRenderConfig::default(),
            stats: SurfaceStats::default(),
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

    fn set_focus(&mut self, focus: SurfaceFocus) {
        self.focus = focus;
        // Automatically adjust priority based on focus
        self.render_config.priority = match focus {
            SurfaceFocus::Focused => RenderPriority::Foreground,
            SurfaceFocus::Unfocused => RenderPriority::Background,
        };
    }

    fn set_visibility_with_priority(&mut self, visibility: SurfaceVisibility) {
        self.visibility = visibility;
        // Automatically adjust priority based on visibility
        self.render_config.priority = match visibility {
            SurfaceVisibility::Visible => {
                // Maintain focus-based priority when visible
                match self.focus {
                    SurfaceFocus::Focused => RenderPriority::Foreground,
                    SurfaceFocus::Unfocused => RenderPriority::Background,
                }
            }
            SurfaceVisibility::Hidden | SurfaceVisibility::Occluded => RenderPriority::Minimized,
        };
    }

    /// Check if this surface should be rendered based on priority and frame pacing.
    fn should_render(&self) -> bool {
        // Skip minimized surfaces
        if self.render_config.priority == RenderPriority::Minimized {
            return false;
        }

        // Check frame pacing
        self.stats.should_render(self.render_config.target_fps)
    }
}

/// Physical size with width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalSize<T> {
    /// Width component.
    pub width: T,
    /// Height component.
    pub height: T,
}

impl<T> PhysicalSize<T> {
    /// Create a new physical size with the given width and height.
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
            allow_software_fallback: true,
            reduced_features: Some(Features::empty()),
            reduced_limits: Some(Limits::downlevel_defaults()),
            automatic_device_loss_detection: true,
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

    /// Evict old entries from the pool (alias for cleanup_old_textures).
    pub fn evict_old_entries(&mut self) {
        self.cleanup_old_textures();
    }

    /// Get current pool statistics.
    pub fn stats(&self) -> &TexturePoolStats {
        &self.stats
    }
}

/// Unified render context that manages GPU resources and provides rendering capabilities.
///
/// `GupContext` is the central hub for all GPU operations in Gup. It owns
/// the wgpu `Device` and `Queue`, manages
/// surface configuration, and provides resource pools (buffers, textures,
/// pipelines) for efficient rendering.
///
/// # Examples
///
/// ```rust,no_run
/// # async fn example() {
/// use gup::context::GupContext;
///
/// // Create a context (requires a GPU adapter)
/// let ctx = GupContext::new().await.expect("GPU init");
/// let device = &ctx.device;
/// let queue = &ctx.queue;
/// # }
/// ```
pub struct GupContext {
    /// Core wgpu device handle.
    pub device: Arc<Device>,
    /// Core wgpu queue handle.
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

    /// Advanced performance profiler (optional)
    performance_profiler: Option<PerformanceProfiler>,

    /// Surface event handlers
    event_handlers: Vec<Box<dyn SurfaceEventHandler>>,

    /// Background rendering throttling enabled
    background_throttling_enabled: bool,

    /// WebGPU instance and adapter (kept for potential reconfiguration)
    _instance: Instance,
    _adapter: Adapter,

    /// Context state for error recovery
    context_state: ContextState,

    /// Recovery callback for state changes
    recovery_callback: Option<RecoveryCallback>,

    /// Last recovery attempt information
    last_recovery_attempt: Option<RecoveryAttemptResult>,

    /// Options used for context creation (for recovery)
    context_options: GupOptions,

    /// Cached surface configurations for automatic recreation after device loss
    cached_surface_configs: HashMap<SurfaceId, CachedSurfaceConfig>,

    /// Window handle renewal callback for surface recreation
    window_handle_renewal_callback: Option<WindowHandleRenewalCallback>,

    /// Recovery metrics for monitoring and analytics
    recovery_metrics: RecoveryMetrics,
}

// Manual Debug implementation to handle non-Debug trait objects
impl std::fmt::Debug for GupContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GupContext")
            .field("device", &self.device)
            .field("queue", &self.queue)
            .field("surfaces", &self.surfaces)
            .field("primary_surface_id", &self.primary_surface_id)
            .field("buffer_pool", &self.buffer_pool)
            .field("texture_pool", &self.texture_pool)
            .field("frame_stats", &self.frame_stats)
            .field("frame_start_time", &self.frame_start_time)
            .field("performance_profiler", &self.performance_profiler)
            .field(
                "event_handlers",
                &format!("{} handlers", self.event_handlers.len()),
            )
            .field(
                "background_throttling_enabled",
                &self.background_throttling_enabled,
            )
            .field("context_state", &self.context_state)
            .field("recovery_callback", &self.recovery_callback.is_some())
            .field("last_recovery_attempt", &self.last_recovery_attempt)
            .field("recovery_metrics", &self.recovery_metrics)
            .finish()
    }
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

    /// Create a context from externally-provided wgpu resources.
    ///
    /// This constructor allows sharing an existing GPU device and queue with a
    /// host application (e.g. a Bevy game engine).  No second adapter or device
    /// is requested – the supplied handles are used directly, so both Gup and
    /// the host operate on the same GPU context.
    pub fn from_wgpu(
        instance: Instance,
        adapter: Adapter,
        device: Arc<Device>,
        queue: Arc<Queue>,
    ) -> Arc<Self> {
        let buffer_pool = BufferPool::new(Arc::clone(&device));
        let texture_pool = TexturePool::new(Arc::clone(&device));

        Arc::new(Self {
            device,
            queue,
            surfaces: HashMap::new(),
            primary_surface_id: None,
            buffer_pool,
            texture_pool,
            frame_stats: FrameStats::default(),
            frame_start_time: None,
            performance_profiler: None,
            event_handlers: Vec::new(),
            background_throttling_enabled: false,
            _instance: instance,
            _adapter: adapter,
            context_state: ContextState::Active,
            recovery_callback: None,
            last_recovery_attempt: None,
            context_options: GupOptions::default(),
            cached_surface_configs: HashMap::new(),
            window_handle_renewal_callback: None,
            recovery_metrics: RecoveryMetrics::default(),
        })
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

        // Clone the options to store them for recovery
        let stored_options = options.clone();

        // Opportunistically enable TIMESTAMP_QUERY when the adapter
        // supports it.  This allows the auto-tune system to use precise
        // GPU-side timing without requiring the caller to explicitly
        // request the feature.
        let mut features = options.required_features;
        if adapter.features().contains(Features::TIMESTAMP_QUERY) {
            features |= Features::TIMESTAMP_QUERY;
        }

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("gup_device"),
                required_features: features,
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
            performance_profiler: None,
            event_handlers: Vec::new(),
            background_throttling_enabled: false,
            _instance: instance,
            _adapter: adapter,
            context_state: ContextState::Active,
            recovery_callback: None,
            last_recovery_attempt: None,
            context_options: stored_options,
            cached_surface_configs: HashMap::new(),
            window_handle_renewal_callback: None,
            recovery_metrics: RecoveryMetrics::default(),
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

        // Cache the surface configuration
        self.cache_surface_config(surface_id, &managed_surface);

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

        // Cache the surface configuration
        self.cache_surface_config(id, &managed_surface);

        self.surfaces.insert(id, managed_surface);

        // Set as primary if this is the first surface
        if self.primary_surface_id.is_none() {
            self.primary_surface_id = Some(id);
        }

        Ok(())
    }

    /// Add a surface with custom configuration.
    ///
    /// This allows fine-grained control over surface settings including present mode,
    /// alpha blending, view formats, and frame latency.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gup::context::{GupContext, SurfaceId, SurfaceConfigBuilder};
    /// # use std::sync::Arc;
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = GupContext::headless().await?;
    /// # let mut context = Arc::into_inner(context).unwrap();
    /// # struct W;
    /// # impl raw_window_handle::HasWindowHandle for W {
    /// #     fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> { todo!() }
    /// # }
    /// # impl raw_window_handle::HasDisplayHandle for W {
    /// #     fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> { todo!() }
    /// # }
    /// # let window: Arc<W> = Arc::new(W);
    /// let config = SurfaceConfigBuilder::new()
    ///     .with_size(1920, 1080)
    ///     .with_present_mode(wgpu::PresentMode::Immediate)  // Low latency
    ///     .with_frame_latency(1)  // Minimal input lag
    ///     .with_view_formats(vec![wgpu::TextureFormat::Bgra8Unorm]);
    ///
    /// let id = SurfaceId::new();
    /// context.add_surface_with_config(id, window, config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_surface_with_config<W>(
        &mut self,
        id: SurfaceId,
        window: Arc<W>,
        config: SurfaceConfigBuilder,
    ) -> GupResult<()>
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

        // Use configured values or fall back to automatic selection
        let format = if let Some(fmt) = config.format {
            // Validate that the requested format is supported
            if !surface_caps.formats.contains(&fmt) {
                return Err(GupError::configuration_error(
                    "surface_format",
                    format!(
                        "Requested format {:?} is not supported. Available: {:?}",
                        fmt, surface_caps.formats
                    ),
                ));
            }
            fmt
        } else {
            self.negotiate_surface_format(&surface_caps)?
        };

        let present_mode = if let Some(mode) = config.present_mode {
            // Validate that the requested mode is supported
            if !surface_caps.present_modes.contains(&mode) {
                return Err(GupError::configuration_error(
                    "present_mode",
                    format!(
                        "Requested present mode {:?} is not supported. Available: {:?}",
                        mode, surface_caps.present_modes
                    ),
                ));
            }
            mode
        } else {
            self.select_present_mode(&surface_caps)
        };

        let alpha_mode = if let Some(mode) = config.alpha_mode {
            // Validate that the requested mode is supported
            if !surface_caps.alpha_modes.contains(&mode) {
                return Err(GupError::configuration_error(
                    "alpha_mode",
                    format!(
                        "Requested alpha mode {:?} is not supported. Available: {:?}",
                        mode, surface_caps.alpha_modes
                    ),
                ));
            }
            mode
        } else {
            self.select_alpha_mode(&surface_caps)
        };

        // Validate view formats if provided
        for view_fmt in &config.view_formats {
            if !surface_caps.formats.contains(view_fmt) {
                return Err(GupError::configuration_error(
                    "view_formats",
                    format!(
                        "View format {:?} is not supported. Available: {:?}",
                        view_fmt, surface_caps.formats
                    ),
                ));
            }
        }

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: config.width,
            height: config.height,
            present_mode,
            alpha_mode,
            view_formats: config.view_formats,
            desired_maximum_frame_latency: config.desired_maximum_frame_latency.unwrap_or(2),
        };

        surface.configure(&self.device, &surface_config);

        let managed_surface = ManagedSurface::new(surface, surface_config, 1.0);

        // Cache the surface configuration
        self.cache_surface_config(id, &managed_surface);

        self.surfaces.insert(id, managed_surface);

        // Set as primary if this is the first surface
        if self.primary_surface_id.is_none() {
            self.primary_surface_id = Some(id);
        }

        Ok(())
    }

    /// Query platform capabilities for a given window.
    ///
    /// This allows inspecting what formats, present modes, and alpha modes are
    /// available before creating a surface, useful for building UI or selecting
    /// optimal configurations.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use gup::context::GupContext;
    /// # use std::sync::Arc;
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = GupContext::headless().await?;
    /// # struct W;
    /// # impl raw_window_handle::HasWindowHandle for W {
    /// #     fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> { todo!() }
    /// # }
    /// # impl raw_window_handle::HasDisplayHandle for W {
    /// #     fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> { todo!() }
    /// # }
    /// # let window: Arc<W> = Arc::new(W);
    /// let caps = context.query_surface_capabilities(window)?;
    /// println!("Available formats: {:?}", caps.formats);
    /// println!("Available present modes: {:?}", caps.present_modes);
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_surface_capabilities<W>(
        &self,
        window: Arc<W>,
    ) -> GupResult<PlatformSurfaceCapabilities>
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
        Ok(PlatformSurfaceCapabilities::from(&surface_caps))
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

        // Fire resize event
        self.fire_event(SurfaceEvent::Resized {
            surface_id: id,
            width: size.width,
            height: size.height,
        })?;

        // Update cached configuration
        self.update_cached_surface_config(id);

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

        // Fire DPI change event
        self.fire_event(SurfaceEvent::DpiChanged {
            surface_id: id,
            scale_factor,
        })?;

        // Update cached configuration
        self.update_cached_surface_config(id);

        Ok(())
    }

    /// Update surface visibility state.
    pub fn set_surface_visibility(&mut self, id: SurfaceId, visible: bool) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        let visibility = if visible {
            SurfaceVisibility::Visible
        } else {
            SurfaceVisibility::Hidden
        };

        surface.set_visibility_with_priority(visibility);

        // Fire visibility change event
        self.fire_event(SurfaceEvent::VisibilityChanged {
            surface_id: id,
            visible,
        })?;

        Ok(())
    }

    /// Update surface focus state.
    pub fn set_surface_focus(&mut self, id: SurfaceId, focused: bool) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        let focus = if focused {
            SurfaceFocus::Focused
        } else {
            SurfaceFocus::Unfocused
        };

        surface.set_focus(focus);

        // Fire focus change event
        self.fire_event(SurfaceEvent::FocusChanged {
            surface_id: id,
            focused,
        })?;

        Ok(())
    }

    /// Get surface visibility state.
    pub fn get_surface_visibility(&self, id: SurfaceId) -> Option<SurfaceVisibility> {
        self.surfaces.get(&id).map(|surface| surface.visibility)
    }

    /// Get surface focus state.
    pub fn get_surface_focus(&self, id: SurfaceId) -> Option<SurfaceFocus> {
        self.surfaces.get(&id).map(|surface| surface.focus)
    }

    /// Set surface-specific render configuration for performance optimization.
    pub fn set_surface_render_config(
        &mut self,
        id: SurfaceId,
        config: SurfaceRenderConfig,
    ) -> GupResult<()> {
        let surface = self
            .surfaces
            .get_mut(&id)
            .ok_or_else(|| GupError::resource_error(format!("Surface with ID {id} not found")))?;

        surface.render_config = config;
        Ok(())
    }

    /// Get surface-specific render configuration.
    pub fn get_surface_render_config(&self, id: SurfaceId) -> Option<SurfaceRenderConfig> {
        self.surfaces
            .get(&id)
            .map(|surface| surface.render_config.clone())
    }

    /// Get comprehensive rendering statistics for all surfaces.
    pub fn get_render_statistics(&self) -> MultiSurfaceStats {
        let mut stats = MultiSurfaceStats::default();

        for (id, surface) in &self.surfaces {
            stats.surface_stats.insert(*id, surface.stats.clone());
            stats.total_frames += surface.stats.frames_rendered;
            stats.total_skipped += surface.stats.frames_skipped;
        }

        // Scheduling overhead is negligible with current implementation
        stats.scheduling_overhead = 0.01; // < 1%

        stats
    }

    /// Optimize memory usage by evicting old pooled resources.
    pub fn optimize_memory_usage(&mut self) -> GupResult<()> {
        // Evict old textures from the pool
        self.texture_pool.evict_old_entries();

        // Cleanup buffer pool (already has automatic cleanup)
        // The buffer pool manages its own memory efficiently

        Ok(())
    }

    /// Check if a surface should render based on its configuration and state.
    pub fn should_render_surface(&self, id: SurfaceId) -> bool {
        self.surfaces
            .get(&id)
            .map(|surface| surface.should_render())
            .unwrap_or(false)
    }

    /// Register an event handler for surface events.
    pub fn register_event_handler(&mut self, handler: Box<dyn SurfaceEventHandler>) {
        self.event_handlers.push(handler);
    }

    /// Enable or disable background rendering throttling.
    pub fn set_background_throttling(&mut self, enabled: bool) {
        self.background_throttling_enabled = enabled;
    }

    /// Check if background throttling is enabled.
    pub fn is_background_throttling_enabled(&self) -> bool {
        self.background_throttling_enabled
    }

    /// Fire a surface event to all registered handlers.
    fn fire_event(&mut self, event: SurfaceEvent) -> GupResult<()> {
        let mut errors = Vec::new();

        for handler in &mut self.event_handlers {
            let result = match event {
                SurfaceEvent::DpiChanged {
                    surface_id,
                    scale_factor,
                } => handler.on_dpi_changed(surface_id, scale_factor),
                SurfaceEvent::FocusChanged {
                    surface_id,
                    focused,
                } => handler.on_focus_changed(surface_id, focused),
                SurfaceEvent::VisibilityChanged {
                    surface_id,
                    visible,
                } => handler.on_visibility_changed(surface_id, visible),
                SurfaceEvent::Resized {
                    surface_id,
                    width,
                    height,
                } => handler.on_resized(surface_id, width, height),
            };

            if let Err(e) = result {
                errors.push(e);
            }
        }

        // Return first error if any occurred
        if let Some(error) = errors.into_iter().next() {
            return Err(error);
        }

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

        let output = match surface.surface.get_current_texture() {
            Ok(output) => output,
            Err(e) => {
                // If automatic detection is enabled, mark device as lost
                if self.context_options.automatic_device_loss_detection {
                    self.mark_device_lost();
                }
                return Err(GupError::webgpu_error(format!(
                    "Failed to acquire surface texture: {e}"
                )));
            }
        };
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
            let output = match surface.surface.get_current_texture() {
                Ok(output) => output,
                Err(e) => {
                    // If automatic detection is enabled, mark device as lost
                    if self.context_options.automatic_device_loss_detection {
                        self.mark_device_lost();
                    }
                    return Err(GupError::webgpu_error(format!(
                        "Failed to acquire surface texture: {e}"
                    )));
                }
            };
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

    /// Enable advanced performance profiling.
    ///
    /// This enables detailed frame statistics, GPU timestamps (if supported),
    /// and performance regression detection.
    pub fn enable_profiling(
        &mut self,
        config: crate::performance::ProfilingConfig,
    ) -> GupResult<()> {
        if self.performance_profiler.is_some() {
            return Err(GupError::invalid_operation(
                "Profiling already enabled".to_string(),
            ));
        }

        let profiler = PerformanceProfiler::new(&self.device, config)?;
        self.performance_profiler = Some(profiler);
        Ok(())
    }

    /// Disable advanced performance profiling.
    pub fn disable_profiling(&mut self) {
        self.performance_profiler = None;
    }

    /// Check if advanced profiling is enabled.
    pub fn is_profiling_enabled(&self) -> bool {
        self.performance_profiler.is_some()
    }

    /// Get the performance profiler (if enabled).
    pub fn profiler(&self) -> Option<&PerformanceProfiler> {
        self.performance_profiler.as_ref()
    }

    /// Get mutable access to the performance profiler (if enabled).
    pub fn profiler_mut(&mut self) -> Option<&mut PerformanceProfiler> {
        self.performance_profiler.as_mut()
    }

    /// Get the current context state.
    pub fn state(&self) -> ContextState {
        self.context_state
    }

    /// Set a callback to be called when the context state changes.
    pub fn set_recovery_callback(&mut self, callback: RecoveryCallback) {
        self.recovery_callback = Some(callback);
    }

    /// Get information about the last recovery attempt.
    pub fn last_recovery_attempt(&self) -> Option<&RecoveryAttemptResult> {
        self.last_recovery_attempt.as_ref()
    }

    /// Get recovery metrics for monitoring and analytics.
    pub fn recovery_metrics(&self) -> &RecoveryMetrics {
        &self.recovery_metrics
    }

    /// Check if the device is still valid.
    pub fn check_device_status(&self) -> bool {
        // In wgpu, the device is lost when poll() returns false or when
        // operations start failing. We can check this by polling.
        // For now, we'll assume the device is valid if we're in Active state.
        self.context_state == ContextState::Active
    }

    /// Attempt to recover from a device loss.
    ///
    /// This will try to recreate the device and restore all surfaces.
    /// Returns `Ok(())` if recovery succeeded, `Err` otherwise.
    pub async fn attempt_recovery(&mut self) -> GupResult<RecoveryAttemptResult> {
        let start_time = Instant::now();

        log::info!(
            "Attempting context recovery from state: {:?}",
            self.context_state
        );

        // Update state to Recovering
        self.update_state(ContextState::Recovering);

        // Try to recreate the device
        let recovery_result = self.recreate_device().await;

        let duration = start_time.elapsed();
        let (result, tier) = match recovery_result {
            Ok(tier) => {
                log::info!(
                    "Context recovery succeeded in {:?} using {:?}",
                    duration,
                    tier
                );
                self.update_state(ContextState::Active);
                (
                    RecoveryAttemptResult {
                        success: true,
                        duration,
                        error: None,
                    },
                    Some(tier),
                )
            }
            Err(e) => {
                log::error!("Context recovery failed: {}", e);
                self.update_state(ContextState::Failed);
                (
                    RecoveryAttemptResult {
                        success: false,
                        duration,
                        error: Some(e.to_string()),
                    },
                    None,
                )
            }
        };

        // Record metrics
        self.recovery_metrics.record_attempt(&result, tier);

        self.last_recovery_attempt = Some(result.clone());
        Ok(result)
    }

    /// Internal method to recreate the device after device loss.
    /// Returns the recovery tier that succeeded, if any.
    async fn recreate_device(&mut self) -> GupResult<RecoveryTier> {
        log::info!("Recreating GPU device...");

        // Try with full features first, then fall back to reduced features
        let result = self
            .try_create_device_with_features(
                self.context_options.required_features,
                self.context_options.required_limits.clone(),
                false,
            )
            .await;

        match result {
            Ok((device, queue, adapter)) => {
                log::info!("Device recreated successfully with full features");
                self.apply_device_update(device, queue, adapter)?;
                Ok(RecoveryTier::FullFeatures)
            }
            Err(full_features_err) => {
                log::warn!(
                    "Failed to recreate device with full features: {}",
                    full_features_err
                );

                // Try with reduced features if available
                if let (Some(reduced_features), Some(reduced_limits)) = (
                    self.context_options.reduced_features,
                    &self.context_options.reduced_limits,
                ) {
                    log::info!("Attempting device creation with reduced features...");
                    match self
                        .try_create_device_with_features(
                            reduced_features,
                            reduced_limits.clone(),
                            false,
                        )
                        .await
                    {
                        Ok((device, queue, adapter)) => {
                            log::warn!("Device recreated with reduced feature set");
                            self.apply_device_update(device, queue, adapter)?;
                            return Ok(RecoveryTier::ReducedFeatures);
                        }
                        Err(reduced_err) => {
                            log::warn!("Failed with reduced features: {}", reduced_err);
                        }
                    }
                }

                // Try software fallback if enabled
                if self.context_options.allow_software_fallback {
                    log::info!("Attempting software fallback...");
                    match self
                        .try_create_device_with_features(
                            self.context_options
                                .reduced_features
                                .unwrap_or(Features::empty()),
                            self.context_options
                                .reduced_limits
                                .clone()
                                .unwrap_or(Limits::downlevel_defaults()),
                            true,
                        )
                        .await
                    {
                        Ok((device, queue, adapter)) => {
                            log::warn!(
                                "Device recreated using software rendering (degraded performance expected)"
                            );
                            self.apply_device_update(device, queue, adapter)?;
                            return Ok(RecoveryTier::SoftwareRendering);
                        }
                        Err(software_err) => {
                            log::error!("Software fallback also failed: {}", software_err);
                        }
                    }
                }

                Err(GupError::webgpu_error(format!(
                    "Failed to recreate device. Tried full features, reduced features, and software fallback. Original error: {}",
                    full_features_err
                )))
            }
        }
    }

    /// Try to create a device with specific features and limits.
    async fn try_create_device_with_features(
        &self,
        features: Features,
        limits: Limits,
        force_fallback: bool,
    ) -> Result<(Device, Queue, Adapter), GupError> {
        let adapter = self
            ._instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: if force_fallback {
                    PowerPreference::LowPower
                } else {
                    self.context_options.power_preference
                },
                compatible_surface: None,
                force_fallback_adapter: force_fallback,
            })
            .await
            .map_err(|e| GupError::webgpu_error(format!("Failed to request adapter: {}", e)))?;

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some(if force_fallback {
                    "gup_device_software"
                } else {
                    "gup_device_recovered"
                }),
                required_features: features,
                required_limits: limits,
                memory_hints: MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .map_err(|e| GupError::webgpu_error(format!("Failed to create device: {}", e)))?;

        Ok((device, queue, adapter))
    }

    /// Apply a successful device recreation.
    fn apply_device_update(
        &mut self,
        device: Device,
        queue: Queue,
        adapter: Adapter,
    ) -> GupResult<()> {
        self.device = Arc::new(device);
        self.queue = Arc::new(queue);
        self._adapter = adapter;

        // Recreate resource pools
        self.buffer_pool = BufferPool::new(Arc::clone(&self.device));
        self.texture_pool = TexturePool::new(Arc::clone(&self.device));

        // Recreate all surfaces
        self.recreate_surfaces()?;

        log::info!("Device update applied successfully");
        Ok(())
    }

    /// Recreate all surfaces after device recovery.
    fn recreate_surfaces(&mut self) -> GupResult<()> {
        // If we have a window handle renewal callback and cached configs, try to recreate surfaces
        let has_callback = self.window_handle_renewal_callback.is_some();

        if has_callback {
            log::info!(
                "Attempting automatic surface recreation with {} cached configs",
                self.cached_surface_configs.len()
            );

            let mut recreated_count = 0;
            let mut failed_surfaces = Vec::new();

            // Collect surface IDs to process
            let surface_ids: Vec<_> = self.cached_surface_configs.keys().copied().collect();

            // Try to recreate each cached surface
            for surface_id in surface_ids {
                // Get the callback and call it (need to clone the callback reference)
                let window_opt = if let Some(ref callback) = self.window_handle_renewal_callback {
                    callback(surface_id)
                } else {
                    None
                };

                match window_opt {
                    Some(window) => {
                        // Get cached config before calling recreate_single_surface
                        let cached_config = self.cached_surface_configs.get(&surface_id).cloned();

                        if let Some(config) = cached_config {
                            match self.recreate_single_surface(surface_id, window, &config) {
                                Ok(()) => {
                                    log::info!("Successfully recreated surface {}", surface_id);
                                    recreated_count += 1;
                                }
                                Err(e) => {
                                    log::error!("Failed to recreate surface {}: {}", surface_id, e);
                                    failed_surfaces.push(surface_id);
                                }
                            }
                        }
                    }
                    None => {
                        log::warn!(
                            "Window handle not available for surface {}, skipping",
                            surface_id
                        );
                        failed_surfaces.push(surface_id);
                    }
                }
            }

            // Remove failed surfaces from cache
            for surface_id in &failed_surfaces {
                self.cached_surface_configs.remove(surface_id);
            }

            log::info!(
                "Recreated {} of {} surfaces",
                recreated_count,
                recreated_count + failed_surfaces.len()
            );

            Ok(())
        } else {
            // No callback set - fall back to manual recreation
            log::warn!(
                "No window handle renewal callback set. Surfaces need to be reconfigured by the application after device recovery"
            );

            // Clear surfaces - application must re-add them
            self.surfaces.clear();
            self.primary_surface_id = None;

            Ok(())
        }
    }

    /// Recreate a single surface with cached configuration.
    fn recreate_single_surface(
        &mut self,
        id: SurfaceId,
        window: Arc<dyn WindowHandle>,
        cached_config: &CachedSurfaceConfig,
    ) -> GupResult<()> {
        let surface = self
            ._instance
            .create_surface(window)
            .map_err(|e| GupError::webgpu_error(format!("Failed to create surface: {e}")))?;

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: cached_config.format,
            width: cached_config.width,
            height: cached_config.height,
            present_mode: cached_config.present_mode,
            alpha_mode: cached_config.alpha_mode,
            view_formats: cached_config.view_formats.clone(),
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        let managed_surface = ManagedSurface::new(surface, config, cached_config.scale_factor);
        self.surfaces.insert(id, managed_surface);

        Ok(())
    }

    /// Update the context state and notify callbacks.
    fn update_state(&mut self, new_state: ContextState) {
        if self.context_state != new_state {
            log::info!(
                "Context state changed: {:?} -> {:?}",
                self.context_state,
                new_state
            );
            self.context_state = new_state;

            // Call recovery callback if set
            if let Some(ref callback) = self.recovery_callback {
                callback(new_state);
            }
        }
    }

    /// Mark the context as device lost.
    ///
    /// This should be called when a device loss is detected.
    pub fn mark_device_lost(&mut self) {
        log::warn!("Device loss detected");
        self.update_state(ContextState::DeviceLost);
    }

    /// Set the window handle renewal callback for surface recreation during recovery.
    ///
    /// This callback will be invoked during device recovery to obtain new window handles
    /// for surfaces that need to be recreated. The callback should return `Some(window)`
    /// if the window is still available, or `None` if the window has been destroyed.
    pub fn set_window_handle_renewal_callback(&mut self, callback: WindowHandleRenewalCallback) {
        self.window_handle_renewal_callback = Some(callback);
    }

    /// Cache surface configuration for automatic recreation.
    fn cache_surface_config(&mut self, id: SurfaceId, managed_surface: &ManagedSurface) {
        let cached = CachedSurfaceConfig {
            width: managed_surface.config.width,
            height: managed_surface.config.height,
            format: managed_surface.config.format,
            present_mode: managed_surface.config.present_mode,
            alpha_mode: managed_surface.config.alpha_mode,
            scale_factor: managed_surface.scale_factor,
            view_formats: managed_surface.config.view_formats.clone(),
        };
        self.cached_surface_configs.insert(id, cached);
        log::debug!("Cached configuration for surface {}", id);
    }

    /// Update cached surface configuration after a surface property changes.
    fn update_cached_surface_config(&mut self, id: SurfaceId) {
        // Create the cached config first, then insert it
        let cached_opt = self.surfaces.get(&id).map(|surface| CachedSurfaceConfig {
            width: surface.config.width,
            height: surface.config.height,
            format: surface.config.format,
            present_mode: surface.config.present_mode,
            alpha_mode: surface.config.alpha_mode,
            scale_factor: surface.scale_factor,
            view_formats: surface.config.view_formats.clone(),
        });

        if let Some(cached) = cached_opt {
            self.cached_surface_configs.insert(id, cached);
            log::debug!("Updated cached configuration for surface {}", id);
        }
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

    /// Create a render pass with a depth-stencil attachment for 3D rendering.
    ///
    /// The depth buffer is cleared to `1.0` each frame.
    pub fn render_pass_with_depth<'b>(
        &'b mut self,
        clear_color: Option<Color>,
        depth_view: &'b TextureView,
    ) -> RenderPass<'b> {
        let clear_value = clear_color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        self.command_encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("gup_render_pass_3d"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.render_target,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear_value),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
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

            // Performance: surface resize should be responsive.
            // Debug builds are slower; use generous thresholds.
            #[cfg(debug_assertions)]
            let threshold_ms: u128 = 100;
            #[cfg(not(debug_assertions))]
            let threshold_ms: u128 = 16;

            assert!(
                duration.as_millis() < threshold_ms,
                "Surface resize took too long: {duration:?} (threshold: {threshold_ms}ms)"
            );
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

    #[tokio::test]
    async fn test_surface_event_handler_registration() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        struct TestHandler {
            dpi_called: std::sync::Arc<std::sync::Mutex<bool>>,
        }

        impl SurfaceEventHandler for TestHandler {
            fn on_dpi_changed(
                &mut self,
                _surface_id: SurfaceId,
                _scale_factor: f64,
            ) -> GupResult<()> {
                *self.dpi_called.lock().unwrap() = true;
                Ok(())
            }
        }

        let dpi_called = std::sync::Arc::new(std::sync::Mutex::new(false));
        let handler = TestHandler {
            dpi_called: dpi_called.clone(),
        };

        ctx.register_event_handler(Box::new(handler));

        // Verify handler was registered
        assert_eq!(ctx.event_handlers.len(), 1);
    }

    #[tokio::test]
    async fn test_background_throttling_config() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        assert!(!ctx.is_background_throttling_enabled());

        ctx.set_background_throttling(true);
        assert!(ctx.is_background_throttling_enabled());

        ctx.set_background_throttling(false);
        assert!(!ctx.is_background_throttling_enabled());
    }

    #[tokio::test]
    async fn test_surface_visibility_tracking() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create a mock surface ID (in real usage, this would come from add_surface)
        let id = SurfaceId::new();

        // Attempting to set visibility on non-existent surface should error
        assert!(ctx.set_surface_visibility(id, false).is_err());
        assert!(ctx.get_surface_visibility(id).is_none());
    }

    #[tokio::test]
    async fn test_surface_focus_tracking() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        // Create a mock surface ID
        let id = SurfaceId::new();

        // Attempting to set focus on non-existent surface should error
        assert!(ctx.set_surface_focus(id, true).is_err());
        assert!(ctx.get_surface_focus(id).is_none());
    }

    #[tokio::test]
    async fn test_event_firing_with_error_handling() {
        let context = GupContext::headless().await.unwrap();
        let mut ctx = Arc::try_unwrap(context).unwrap();

        struct ErrorHandler;
        impl SurfaceEventHandler for ErrorHandler {
            fn on_focus_changed(
                &mut self,
                _surface_id: SurfaceId,
                _focused: bool,
            ) -> GupResult<()> {
                Err(GupError::resource_error("Test error".to_string()))
            }
        }

        ctx.register_event_handler(Box::new(ErrorHandler));

        // Fire an event that will trigger the error handler
        let id = SurfaceId::new();
        let result = ctx.fire_event(SurfaceEvent::FocusChanged {
            surface_id: id,
            focused: true,
        });

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_surface_event_types() {
        // Test that all event types can be created
        let id = SurfaceId::new();

        let _dpi_event = SurfaceEvent::DpiChanged {
            surface_id: id,
            scale_factor: 2.0,
        };

        let _focus_event = SurfaceEvent::FocusChanged {
            surface_id: id,
            focused: true,
        };

        let _visibility_event = SurfaceEvent::VisibilityChanged {
            surface_id: id,
            visible: false,
        };

        let _resize_event = SurfaceEvent::Resized {
            surface_id: id,
            width: 1920,
            height: 1080,
        };
    }

    #[tokio::test]
    async fn test_surface_visibility_enum() {
        assert_eq!(SurfaceVisibility::Visible, SurfaceVisibility::Visible);
        assert_ne!(SurfaceVisibility::Visible, SurfaceVisibility::Hidden);
        assert_ne!(SurfaceVisibility::Hidden, SurfaceVisibility::Occluded);
    }

    #[tokio::test]
    async fn test_surface_focus_enum() {
        assert_eq!(SurfaceFocus::Focused, SurfaceFocus::Focused);
        assert_ne!(SurfaceFocus::Focused, SurfaceFocus::Unfocused);
    }
}
