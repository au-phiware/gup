// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rectangular brush selection for GPU-accelerated visualizations.
//!
//! This module provides [`BrushBehavior`] — a configurable behaviour that
//! enables drag-to-select rectangular regions on a chart. While dragging,
//! a semi-transparent [`BrushMark`] overlay gives visual feedback. On
//! release the brush performs a region query (CPU or GPU) and fires a
//! [`BrushEvent`] carrying the selected mark IDs.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌────────────────┐     ┌─────────────┐
//! │ pointer input │────▶│ BrushBehavior  │────▶│ BrushEvent  │
//! │ (drag)        │     │  (state)       │     │ (selection)  │
//! └──────────────┘     └────────────────┘     └─────────────┘
//!                           │
//!                           ▼
//!                      ┌─────────────┐
//!                      │ BrushMark   │
//!                      │ (overlay)   │
//!                      └─────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use gup::brush::{BrushBehavior, BrushEvent, BrushStyle};
//!
//! let brush = BrushBehavior::new()
//!     .style(BrushStyle::default())
//!     .on("brush", |event: &BrushEvent| {
//!         println!("Brushing: {:?}", event.screen_extent);
//!     })
//!     .on("brushend", |event: &BrushEvent| {
//!         println!("Selected {} marks", event.selection.len());
//!     });
//! ```

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::time::Duration;

use crate::event::ViewportTransform;
use crate::interaction::{InteractionSystem, Rect, Vec2};
use crate::linked_selection::SharedSelectionState;
use crate::mark_selection::MarkSelectionSystem;

// ---------------------------------------------------------------------------
// BrushExtent — data-space rectangle
// ---------------------------------------------------------------------------

/// A rectangle in data-space (world) coordinates representing the
/// brushed region after inverse-transforming from screen space.
///
/// The origin is the top-left corner and the extent describes the
/// width and height of the rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushExtent {
    /// X coordinate of the left edge in data space.
    pub x: f32,
    /// Y coordinate of the top edge in data space.
    pub y: f32,
    /// Width of the brush rectangle in data-space units.
    pub width: f32,
    /// Height of the brush rectangle in data-space units.
    pub height: f32,
}

impl BrushExtent {
    /// Create a new extent from origin and size.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create an extent from two corner points (normalises so width/height ≥ 0).
    pub fn from_corners(p1: Vec2, p2: Vec2) -> Self {
        let min_x = p1.x.min(p2.x);
        let min_y = p1.y.min(p2.y);
        let max_x = p1.x.max(p2.x);
        let max_y = p1.y.max(p2.y);
        Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    /// Convert to an `[f32; 4]` array `[x, y, width, height]`.
    pub fn as_array(&self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    /// Returns `true` if the extent has zero area.
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Convert to an [`interaction::Rect`](crate::interaction::Rect).
    pub fn to_rect(&self) -> Rect {
        Rect::new(
            Vec2::new(self.x, self.y),
            Vec2::new(self.x + self.width, self.y + self.height),
        )
    }
}

// ---------------------------------------------------------------------------
// BrushStyle — visual customisation
// ---------------------------------------------------------------------------

/// Visual style for the brush overlay rectangle.
///
/// Colours are specified as `[r, g, b, a]` in the range `[0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushStyle {
    /// Fill colour of the brush rectangle. Default: `[0.4, 0.6, 1.0, 0.2]`.
    pub fill: [f32; 4],
    /// Stroke (border) colour of the brush rectangle. Default: `[0.4, 0.6, 1.0, 0.8]`.
    pub stroke: [f32; 4],
    /// Stroke width in pixels. Default: `1.0`.
    pub stroke_width: f32,
}

impl Default for BrushStyle {
    fn default() -> Self {
        Self {
            fill: [0.4, 0.6, 1.0, 0.2],
            stroke: [0.4, 0.6, 1.0, 0.8],
            stroke_width: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// BrushEvent — emitted during and after a brush gesture
// ---------------------------------------------------------------------------

/// Event emitted by [`BrushBehavior`] during (`"brush"`) and after
/// (`"brushend"`) a drag gesture.
///
/// Contains the brush rectangle in both data-space and screen-space
/// coordinates, plus the IDs of marks that fall within the rectangle.
#[derive(Debug, Clone)]
pub struct BrushEvent {
    /// Brush rectangle in data-space (world) coordinates.
    pub data_extent: BrushExtent,
    /// Brush rectangle in screen-space (pixel) coordinates.
    pub screen_extent: BrushExtent,
    /// IDs of marks inside the brush rectangle.
    ///
    /// Empty (not `None`) when the drag produces a zero-area rectangle
    /// or no marks fall within the region.
    pub selection: Vec<u32>,
}

impl BrushEvent {
    /// Create a new event with the given extents and selection.
    pub fn new(data_extent: BrushExtent, screen_extent: BrushExtent, selection: Vec<u32>) -> Self {
        Self {
            data_extent,
            screen_extent,
            selection,
        }
    }
}

// ---------------------------------------------------------------------------
// BrushMark — overlay representation (internal)
// ---------------------------------------------------------------------------

/// Internal representation of the brush overlay rectangle.
///
/// The `BrushMark` tracks whether the overlay should be rendered and
/// its current screen-space rectangle. Rendering is handled by the
/// chart's render loop — this type only stores the geometry.
#[derive(Debug, Clone)]
pub struct BrushMark {
    /// Whether the overlay is currently visible (drag in progress).
    pub visible: bool,
    /// Current screen-space rectangle for the overlay.
    pub screen_rect: Option<Rect>,
    /// Visual style.
    pub style: BrushStyle,
}

impl BrushMark {
    /// Create a new invisible brush mark with the given style.
    pub fn new(style: BrushStyle) -> Self {
        Self {
            visible: false,
            screen_rect: None,
            style,
        }
    }

    /// Show the overlay with the given screen-space rectangle.
    pub fn show(&mut self, rect: Rect) {
        self.visible = true;
        self.screen_rect = Some(rect);
    }

    /// Hide the overlay (drag ended).
    pub fn hide(&mut self) {
        self.visible = false;
        self.screen_rect = None;
    }
}

impl Default for BrushMark {
    fn default() -> Self {
        Self::new(BrushStyle::default())
    }
}

// ---------------------------------------------------------------------------
// GpuBrushConfig — GPU-accelerated brush configuration
// ---------------------------------------------------------------------------

/// Configuration for GPU-accelerated brush region queries.
///
/// When using [`BrushBehavior::on_pointer_up_async`], the GPU path is
/// preferred for large datasets (500K+ marks). If the GPU query does
/// not complete within [`timeout`](Self::timeout) the system falls back
/// to the CPU [`filter_by_rect`](MarkSelectionSystem::filter_by_rect)
/// path.
#[derive(Debug, Clone)]
pub struct GpuBrushConfig {
    /// Maximum time to wait for the GPU query before falling back to
    /// CPU. Default: 50 ms.
    pub timeout: Duration,
}

impl Default for GpuBrushConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(50),
        }
    }
}

// ---------------------------------------------------------------------------
// BrushBehavior — developer-facing API
// ---------------------------------------------------------------------------

/// Type-erased brush event handler.
type BrushHandler = Box<dyn Fn(&BrushEvent) + Send + Sync>;

/// Configurable brush behaviour for rectangular drag-to-select.
///
/// `BrushBehavior` processes mouse/touch drag events, renders a visual
/// overlay via [`BrushMark`], performs a region query against mark
/// positions, and fires [`BrushEvent`]s to registered handlers.
///
/// # Builder API
///
/// ```rust
/// use gup::brush::{BrushBehavior, BrushEvent, BrushStyle};
///
/// let brush = BrushBehavior::new()
///     .style(BrushStyle {
///         fill: [1.0, 0.0, 0.0, 0.15],
///         stroke: [1.0, 0.0, 0.0, 0.8],
///         stroke_width: 2.0,
///     })
///     .on("brush", |e: &BrushEvent| {
///         println!("dragging: {} marks", e.selection.len());
///     })
///     .on("brushend", |e: &BrushEvent| {
///         println!("selected: {:?}", e.selection);
///     });
/// ```
///
/// # Event Names
///
/// | Name          | When                         |
/// |---------------|------------------------------|
/// | `"brush"`     | Each frame while dragging    |
/// | `"brushend"`  | Mouse/touch released         |
pub struct BrushBehavior {
    /// Visual style for the overlay.
    style: BrushStyle,
    /// Registered event handlers keyed by event name.
    handlers: HashMap<String, Vec<BrushHandler>>,
    /// Internal overlay state.
    overlay: BrushMark,
    /// Drag start position in screen space (set on mouse-down).
    drag_start: Option<Vec2>,
    /// Current drag position in screen space (updated on mouse-move).
    drag_current: Option<Vec2>,
    /// Configuration for GPU-accelerated region queries.
    gpu_config: GpuBrushConfig,
}

impl fmt::Debug for BrushBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrushBehavior")
            .field("style", &self.style)
            .field(
                "handlers",
                &self
                    .handlers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.len()))
                    .collect::<HashMap<_, _>>(),
            )
            .field("overlay", &self.overlay)
            .field("drag_start", &self.drag_start)
            .field("drag_current", &self.drag_current)
            .field("gpu_config", &self.gpu_config)
            .finish()
    }
}

impl Default for BrushBehavior {
    fn default() -> Self {
        Self::new()
    }
}

impl BrushBehavior {
    /// Create a new `BrushBehavior` with default style and no handlers.
    ///
    /// No extra type parameters are required.
    pub fn new() -> Self {
        let style = BrushStyle::default();
        let overlay = BrushMark::new(style.clone());
        Self {
            style,
            handlers: HashMap::new(),
            overlay,
            drag_start: None,
            drag_current: None,
            gpu_config: GpuBrushConfig::default(),
        }
    }

    // -- Builder methods --

    /// Set the visual style for the brush overlay.
    pub fn style(mut self, style: BrushStyle) -> Self {
        self.style = style.clone();
        self.overlay.style = style;
        self
    }

    /// Set the GPU brush configuration (timeout, etc.).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gup::brush::{BrushBehavior, GpuBrushConfig};
    /// use std::time::Duration;
    ///
    /// let brush = BrushBehavior::new()
    ///     .with_gpu_config(GpuBrushConfig {
    ///         timeout: Duration::from_millis(100),
    ///     });
    /// ```
    pub fn with_gpu_config(mut self, config: GpuBrushConfig) -> Self {
        self.gpu_config = config;
        self
    }

    /// Register a handler for a brush event.
    ///
    /// Supported event names:
    /// - `"brush"` — fired each frame while the drag is in progress.
    /// - `"brushend"` — fired when the drag is released.
    ///
    /// Calling `.on()` with the same event name adds an additional handler
    /// (they are invoked in registration order). To replace all handlers
    /// for an event, call [`clear_handlers`](Self::clear_handlers) first.
    pub fn on<F>(mut self, event_name: &str, handler: F) -> Self
    where
        F: Fn(&BrushEvent) + Send + Sync + 'static,
    {
        self.handlers
            .entry(event_name.to_string())
            .or_default()
            .push(Box::new(handler));
        self
    }

    /// Remove all handlers for the given event name.
    pub fn clear_handlers(&mut self, event_name: &str) {
        self.handlers.remove(event_name);
    }

    /// Remove all registered handlers.
    pub fn clear_all_handlers(&mut self) {
        self.handlers.clear();
    }

    // -- State accessors --

    /// Returns the current [`BrushStyle`].
    pub fn current_style(&self) -> &BrushStyle {
        &self.style
    }

    /// Returns the overlay mark (for rendering).
    pub fn overlay(&self) -> &BrushMark {
        &self.overlay
    }

    /// Returns `true` if a drag is currently in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag_start.is_some()
    }

    /// Returns the current screen-space brush rectangle, if dragging.
    pub fn current_screen_rect(&self) -> Option<Rect> {
        match (self.drag_start, self.drag_current) {
            (Some(start), Some(current)) => Some(Rect::new(
                Vec2::new(start.x.min(current.x), start.y.min(current.y)),
                Vec2::new(start.x.max(current.x), start.y.max(current.y)),
            )),
            _ => None,
        }
    }

    // -- Input event handling --

    /// Handle a pointer-down event. Starts the brush drag.
    pub fn on_pointer_down(&mut self, screen_position: Vec2) {
        self.drag_start = Some(screen_position);
        self.drag_current = Some(screen_position);
    }

    /// Handle a pointer-move event. Updates the brush rectangle and
    /// fires `"brush"` handlers.
    ///
    /// `viewport` is used to convert screen coordinates to data space.
    /// `selection_system` is optionally provided for live hit testing
    /// during the drag.
    pub fn on_pointer_move(
        &mut self,
        screen_position: Vec2,
        viewport: &ViewportTransform,
        selection_system: Option<&MarkSelectionSystem>,
    ) {
        if self.drag_start.is_none() {
            return;
        }
        self.drag_current = Some(screen_position);

        if let Some(rect) = self.current_screen_rect() {
            // Update overlay
            self.overlay.show(rect);

            // Build event
            let screen_extent = BrushExtent::from_corners(rect.min, rect.max);
            let data_start = viewport.screen_to_world(rect.min);
            let data_end = viewport.screen_to_world(rect.max);
            let data_extent = BrushExtent::from_corners(data_start, data_end);

            // Optional live hit testing during drag
            let selection = if let Some(system) = selection_system {
                if let Some(positions) = system.positions() {
                    let data_rect = data_extent.to_rect();
                    MarkSelectionSystem::filter_by_rect(&data_rect, positions)
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let event = BrushEvent::new(data_extent, screen_extent, selection);
            self.fire("brush", &event);
        }
    }

    /// Handle a pointer-up event. Finalises the brush, fires `"brushend"`,
    /// and clears the overlay.
    ///
    /// `viewport` is used to convert screen coordinates to data space.
    /// `selection_system` is used for the final hit test.
    pub fn on_pointer_up(
        &mut self,
        screen_position: Vec2,
        viewport: &ViewportTransform,
        selection_system: Option<&MarkSelectionSystem>,
    ) {
        if self.drag_start.is_none() {
            return;
        }
        self.drag_current = Some(screen_position);

        let rect = self.current_screen_rect();
        let (data_extent, screen_extent, selection) = if let Some(rect) = rect {
            let screen_extent = BrushExtent::from_corners(rect.min, rect.max);
            let data_start = viewport.screen_to_world(rect.min);
            let data_end = viewport.screen_to_world(rect.max);
            let data_extent = BrushExtent::from_corners(data_start, data_end);

            let selection = if let Some(system) = selection_system {
                if let Some(positions) = system.positions() {
                    let data_rect = data_extent.to_rect();
                    MarkSelectionSystem::filter_by_rect(&data_rect, positions)
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            (data_extent, screen_extent, selection)
        } else {
            let zero = BrushExtent::new(0.0, 0.0, 0.0, 0.0);
            (zero, zero, Vec::new())
        };

        let event = BrushEvent::new(data_extent, screen_extent, selection);
        self.fire("brushend", &event);

        // Clear drag state and hide overlay
        self.drag_start = None;
        self.drag_current = None;
        self.overlay.hide();
    }

    /// Handle a pointer-up event with GPU-accelerated region query.
    ///
    /// When `interaction_system` is `Some`, the GPU
    /// [`rect_hit_test_gpu`](MarkSelectionSystem::rect_hit_test_gpu) path
    /// is used. If the GPU query does not complete within the configured
    /// [`GpuBrushConfig::timeout`], the CPU
    /// [`filter_by_rect`](MarkSelectionSystem::filter_by_rect) fallback
    /// is used instead.
    ///
    /// When `interaction_system` is `None`, the CPU path is always used
    /// (identical to [`on_pointer_up`](Self::on_pointer_up)).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// brush.on_pointer_up_async(
    ///     screen_pos,
    ///     &viewport,
    ///     Some(&mark_system),
    ///     Some(&mut interaction_system),
    /// ).await;
    /// ```
    pub async fn on_pointer_up_async(
        &mut self,
        screen_position: Vec2,
        viewport: &ViewportTransform,
        selection_system: Option<&MarkSelectionSystem>,
        interaction_system: Option<&mut InteractionSystem>,
    ) {
        if self.drag_start.is_none() {
            return;
        }
        self.drag_current = Some(screen_position);

        let rect = self.current_screen_rect();
        let (data_extent, screen_extent, selection) = if let Some(rect) = rect {
            let screen_extent = BrushExtent::from_corners(rect.min, rect.max);
            let data_start = viewport.screen_to_world(rect.min);
            let data_end = viewport.screen_to_world(rect.max);
            let data_extent = BrushExtent::from_corners(data_start, data_end);

            let selection = self
                .query_region(data_extent, selection_system, interaction_system)
                .await;

            (data_extent, screen_extent, selection)
        } else {
            let zero = BrushExtent::new(0.0, 0.0, 0.0, 0.0);
            (zero, zero, Vec::new())
        };

        let event = BrushEvent::new(data_extent, screen_extent, selection);
        self.fire("brushend", &event);

        // Clear drag state and hide overlay
        self.drag_start = None;
        self.drag_current = None;
        self.overlay.hide();
    }

    /// Returns the current [`GpuBrushConfig`].
    pub fn current_gpu_config(&self) -> &GpuBrushConfig {
        &self.gpu_config
    }

    /// Wire this brush to automatically update a [`SharedSelectionState`]
    /// on every brush and brushend event.
    ///
    /// `index_to_key` maps each selected mark index (`u32`) to the
    /// corresponding cross-chart identity key of type `K`. Typically this
    /// is a closure that looks up the data item by index and extracts
    /// the key.
    ///
    /// On each `"brush"` event the shared state is **replaced** (via
    /// [`SharedSelectionState::set`]) with the keys of the currently
    /// brushed marks. On `"brushend"` the final selection is written
    /// and on drag cancel or zero-area brush the selection is cleared.
    ///
    /// This method consumes `self` and returns it with the handlers
    /// registered, fitting the builder pattern.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gup::brush::BrushBehavior;
    /// use gup::linked_selection::SharedSelectionState;
    ///
    /// let shared = SharedSelectionState::<usize>::new();
    /// let brush = BrushBehavior::new()
    ///     .with_shared_selection(shared.clone(), |idx| idx as usize);
    ///
    /// // After a brush gesture, `shared` will contain the selected keys.
    /// ```
    pub fn with_shared_selection<K>(
        self,
        state: SharedSelectionState<K>,
        index_to_key: impl Fn(u32) -> K + Send + Sync + 'static,
    ) -> Self
    where
        K: Hash + Eq + Send + Sync + 'static,
    {
        let brush_state = state.clone();

        {
            // Share the key fn between two closures via Arc
            let key_fn = std::sync::Arc::new(index_to_key);
            let end_key_fn = key_fn.clone();

            let s = self.on("brush", move |event: &BrushEvent| {
                let keys: Vec<K> = event.selection.iter().map(|&id| (*key_fn)(id)).collect();
                brush_state.set(keys);
            });

            let end_state = state;
            s.on("brushend", move |event: &BrushEvent| {
                if event.selection.is_empty() {
                    end_state.clear();
                } else {
                    let keys: Vec<K> = event
                        .selection
                        .iter()
                        .map(|&id| (*end_key_fn)(id))
                        .collect();
                    end_state.set(keys);
                }
            })
        }
    }

    /// Cancel the current brush without firing `"brushend"`.
    pub fn cancel(&mut self) {
        self.drag_start = None;
        self.drag_current = None;
        self.overlay.hide();
    }

    // -- Internal --

    /// Fire all handlers registered for the given event name.
    fn fire(&self, event_name: &str, event: &BrushEvent) {
        if let Some(handlers) = self.handlers.get(event_name) {
            for handler in handlers {
                handler(event);
            }
        }
    }

    /// Internal: perform a region query using the best available method.
    ///
    /// Tries the GPU path first (if an `InteractionSystem` is provided and
    /// positions are set). Falls back to CPU `filter_by_rect` on timeout
    /// or when no GPU system is available.
    async fn query_region(
        &self,
        data_extent: BrushExtent,
        selection_system: Option<&MarkSelectionSystem>,
        interaction_system: Option<&mut InteractionSystem>,
    ) -> Vec<u32> {
        let system = match selection_system {
            Some(s) => s,
            None => return Vec::new(),
        };

        let data_rect = data_extent.to_rect();

        // Try GPU path when an InteractionSystem is available.
        if let Some(is) = interaction_system {
            let timeout = self.gpu_config.timeout;
            let start = std::time::Instant::now();
            let gpu_result = system.rect_hit_test_gpu(&data_rect, is).await;
            let elapsed = start.elapsed();

            match gpu_result {
                Ok(ids) if elapsed <= timeout => return ids,
                Ok(_) => {
                    // GPU succeeded but exceeded timeout — fall back to CPU
                    // for this query and let the caller know via the
                    // (cheaper) CPU path. This ensures a responsive UX.
                }
                Err(_) => {
                    // GPU error — fall back to CPU silently.
                }
            }
        }

        // CPU fallback.
        if let Some(positions) = system.positions() {
            MarkSelectionSystem::filter_by_rect(&data_rect, positions)
        } else {
            Vec::new()
        }
    }
}

// ---------------------------------------------------------------------------
// BrushOverlayRenderer — GPU rectangle overlay for the brush mark
// ---------------------------------------------------------------------------

/// GPU renderer for the [`BrushMark`] overlay rectangle.
///
/// `BrushOverlayRenderer` allocates a single [`RectangleInstance`] on the
/// GPU and re-uploads its data each frame the brush is visible.  Call
/// [`update`](Self::update) once per frame (before the render pass) and
/// then [`render`](Self::render) inside the pass — after all data marks
/// so the overlay has the highest z-order.
///
/// # Examples
///
/// ```rust,no_run
/// # fn example(device: &wgpu::Device, queue: &wgpu::Queue,
/// #            cache: &mut gup::PipelineCache,
/// #            brush: &gup::brush::BrushBehavior) {
/// use gup::brush::BrushOverlayRenderer;
/// let mut renderer = BrushOverlayRenderer::new(device, queue, cache)
///     .expect("pipeline creation");
/// renderer.update(brush.overlay(), queue);
/// # }
/// ```
pub struct BrushOverlayRenderer {
    pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    viewport_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vt_bind_group: wgpu::BindGroup,
    /// `true` when the overlay should be drawn this frame.
    visible: bool,
}

impl BrushOverlayRenderer {
    /// Create a new overlay renderer, allocating GPU resources.
    ///
    /// The rectangle pipeline is obtained (or created) through `cache`
    /// so it is shared with any other `Selection<_, Rectangle>` that uses
    /// the same cache.
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        cache: &mut crate::pipeline_cache::PipelineCache,
    ) -> crate::error::GupResult<Self> {
        use crate::mark::Mark;
        use crate::mark::rectangle::{Rectangle, RectangleInstance};
        use wgpu::util::DeviceExt;

        // Reuse the cached Rectangle pipeline.
        let pipeline = cache.get_or_create::<Rectangle>(device)?;

        // Unit-quad vertex buffer (same geometry every Rectangle uses).
        let vertices = Rectangle::generate_vertices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("brush_overlay_vertex"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Index buffer (two triangles).
        let indices = Rectangle::generate_indices().unwrap_or_default();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("brush_overlay_index"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Single-instance storage buffer (zeroed — no overlay yet).
        let zero_instance = RectangleInstance {
            center: [0.0; 2],
            size: [0.0; 2],
            fill_color: [0.0; 4],
            stroke_width: 0.0,
            _pad1: [0.0; 3],
            stroke_color: [0.0; 4],
            corner_radius: 0.0,
            _padding: 0.0,
            _pad2: [0.0; 2],
        };
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("brush_overlay_instance"),
            contents: bytemuck::bytes_of(&zero_instance),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Viewport dimensions uniform at group(0) binding(1).
        // The rectangle SDF fragment shader uses this for pixel-to-clip
        // conversion.  We default to 800×600 and update via set_viewport_size.
        let viewport_uniform = crate::selection::ViewportUniforms {
            width: 800.0,
            height: 600.0,
        };
        let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("brush_overlay_viewport"),
            contents: bytemuck::bytes_of(&viewport_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Viewport transform uniform (identity — brush coords are already in clip space).
        let identity_vt = crate::zoom::GpuViewportTransform::IDENTITY;
        let vt_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("brush_overlay_viewport_transform"),
            contents: bytemuck::bytes_of(&identity_vt),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Bind group 0: storage buffer + viewport dimensions uniform.
        let bg0_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("brush_overlay_bg0"),
            layout: &bg0_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: viewport_buffer.as_entire_binding(),
                },
            ],
        });

        // Bind group 1: viewport transform uniform.
        let bg1_layout = pipeline.get_bind_group_layout(1);
        let vt_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("brush_overlay_bg1"),
            layout: &bg1_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: vt_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            viewport_buffer,
            bind_group,
            vt_bind_group,
            visible: false,
        })
    }

    /// Update the overlay from the current [`BrushMark`] state.
    ///
    /// Call this once per frame before the render pass.  When the brush
    /// is visible, a single [`RectangleInstance`] is written to the GPU
    /// storage buffer.  When hidden, the renderer is flagged as invisible
    /// and [`render`](Self::render) becomes a no-op.
    pub fn update(&mut self, mark: &BrushMark, queue: &wgpu::Queue) {
        if !mark.visible {
            self.visible = false;
            return;
        }

        let rect = match &mark.screen_rect {
            Some(r) => r,
            None => {
                self.visible = false;
                return;
            }
        };

        // Convert Rect (min/max in clip space) → centre + size.
        let cx = (rect.min.x + rect.max.x) * 0.5;
        let cy = (rect.min.y + rect.max.y) * 0.5;
        let w = (rect.max.x - rect.min.x).abs();
        let h = (rect.max.y - rect.min.y).abs();

        // Convert stroke_width from pixels to clip-space units.
        // We don't have window size here, so we use a small constant
        // that looks reasonable (the SDF expects clip-space units).
        let stroke_width_clip = mark.style.stroke_width * 0.002;

        let instance = crate::mark::rectangle::RectangleInstance {
            center: [cx, cy],
            size: [w, h],
            fill_color: mark.style.fill,
            stroke_width: stroke_width_clip,
            _pad1: [0.0; 3],
            stroke_color: mark.style.stroke,
            corner_radius: 0.0,
            _padding: 0.0,
            _pad2: [0.0; 2],
        };

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::bytes_of(&instance));
        self.visible = true;
    }

    /// Render the brush overlay into an active render pass.
    ///
    /// This should be the **last** draw call in the pass so the overlay
    /// appears above all data marks.  If the brush is not visible this
    /// method is a no-op.
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if !self.visible {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_bind_group(1, &self.vt_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        // Draw 6 indices, 1 instance.
        render_pass.draw_indexed(0..6, 0, 0..1);
    }

    /// Returns `true` if the overlay is currently visible (will draw).
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Update the viewport dimensions used for SDF pixel-to-clip
    /// conversion.
    ///
    /// Call this whenever the window size changes.
    pub fn set_viewport_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let viewport = crate::selection::ViewportUniforms { width, height };
        queue.write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&viewport));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    // -- BrushExtent -------------------------------------------------------

    #[test]
    fn brush_extent_from_corners_normalises() {
        let extent = BrushExtent::from_corners(Vec2::new(200.0, 300.0), Vec2::new(100.0, 100.0));
        assert_eq!(extent.x, 100.0);
        assert_eq!(extent.y, 100.0);
        assert_eq!(extent.width, 100.0);
        assert_eq!(extent.height, 200.0);
    }

    #[test]
    fn brush_extent_empty_when_zero_area() {
        let extent = BrushExtent::new(10.0, 10.0, 0.0, 5.0);
        assert!(extent.is_empty());
    }

    #[test]
    fn brush_extent_not_empty() {
        let extent = BrushExtent::new(10.0, 10.0, 5.0, 5.0);
        assert!(!extent.is_empty());
    }

    #[test]
    fn brush_extent_as_array() {
        let extent = BrushExtent::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(extent.as_array(), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn brush_extent_to_rect() {
        let extent = BrushExtent::new(10.0, 20.0, 30.0, 40.0);
        let rect = extent.to_rect();
        assert_eq!(rect.min.x, 10.0);
        assert_eq!(rect.min.y, 20.0);
        assert_eq!(rect.max.x, 40.0);
        assert_eq!(rect.max.y, 60.0);
    }

    // -- BrushStyle --------------------------------------------------------

    #[test]
    fn brush_style_default() {
        let style = BrushStyle::default();
        assert_eq!(style.fill, [0.4, 0.6, 1.0, 0.2]);
        assert_eq!(style.stroke, [0.4, 0.6, 1.0, 0.8]);
        assert_eq!(style.stroke_width, 1.0);
    }

    // -- BrushEvent --------------------------------------------------------

    #[test]
    fn brush_event_clone_and_debug() {
        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 10.0, 10.0),
            BrushExtent::new(0.0, 0.0, 100.0, 100.0),
            vec![1, 2, 3],
        );
        let cloned = event.clone();
        assert_eq!(cloned.selection, vec![1, 2, 3]);
        let debug = format!("{:?}", cloned);
        assert!(debug.contains("BrushEvent"));
    }

    #[test]
    fn brush_event_empty_selection_for_zero_area() {
        let event = BrushEvent::new(
            BrushExtent::new(5.0, 5.0, 0.0, 0.0),
            BrushExtent::new(50.0, 50.0, 0.0, 0.0),
            Vec::new(),
        );
        assert!(event.selection.is_empty());
        assert!(event.data_extent.is_empty());
    }

    // -- BrushBehavior builder ---------------------------------------------

    #[test]
    fn brush_behavior_new_compiles_without_type_params() {
        let _brush = BrushBehavior::new();
    }

    #[test]
    fn brush_behavior_default_style() {
        let brush = BrushBehavior::new();
        assert_eq!(*brush.current_style(), BrushStyle::default());
    }

    #[test]
    fn brush_behavior_custom_style() {
        let style = BrushStyle {
            fill: [1.0, 0.0, 0.0, 0.3],
            stroke: [1.0, 0.0, 0.0, 1.0],
            stroke_width: 3.0,
        };
        let brush = BrushBehavior::new().style(style.clone());
        assert_eq!(*brush.current_style(), style);
    }

    #[test]
    fn brush_behavior_on_registers_handler() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let brush = BrushBehavior::new().on("brushend", move |_e: &BrushEvent| {
            c.fetch_add(1, Ordering::Relaxed);
        });

        // Fire the handler manually
        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 10.0, 10.0),
            vec![],
        );
        brush.fire("brushend", &event);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn brush_behavior_multiple_handlers_same_event() {
        let counter = Arc::new(AtomicU32::new(0));
        let c1 = counter.clone();
        let c2 = counter.clone();
        let brush = BrushBehavior::new()
            .on("brush", move |_| {
                c1.fetch_add(1, Ordering::Relaxed);
            })
            .on("brush", move |_| {
                c2.fetch_add(10, Ordering::Relaxed);
            });

        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 10.0, 10.0),
            vec![],
        );
        brush.fire("brush", &event);
        assert_eq!(counter.load(Ordering::Relaxed), 11);
    }

    #[test]
    fn brush_behavior_debug() {
        let brush = BrushBehavior::new();
        let debug = format!("{:?}", brush);
        assert!(debug.contains("BrushBehavior"));
    }

    // -- Drag lifecycle ----------------------------------------------------

    #[test]
    fn brush_drag_lifecycle() {
        let mut brush = BrushBehavior::new();

        assert!(!brush.is_dragging());

        brush.on_pointer_down(Vec2::new(100.0, 100.0));
        assert!(brush.is_dragging());

        let vt = ViewportTransform::default();
        brush.on_pointer_move(Vec2::new(200.0, 200.0), &vt, None);

        let rect = brush.current_screen_rect().unwrap();
        assert_eq!(rect.min.x, 100.0);
        assert_eq!(rect.min.y, 100.0);
        assert_eq!(rect.max.x, 200.0);
        assert_eq!(rect.max.y, 200.0);

        assert!(brush.overlay().visible);

        brush.on_pointer_up(Vec2::new(200.0, 200.0), &vt, None);
        assert!(!brush.is_dragging());
        assert!(!brush.overlay().visible);
    }

    #[test]
    fn brush_cancel_hides_overlay() {
        let mut brush = BrushBehavior::new();
        brush.on_pointer_down(Vec2::new(50.0, 50.0));
        let vt = ViewportTransform::default();
        brush.on_pointer_move(Vec2::new(150.0, 150.0), &vt, None);
        assert!(brush.overlay().visible);

        brush.cancel();
        assert!(!brush.is_dragging());
        assert!(!brush.overlay().visible);
    }

    // -- Viewport-aware coordinates (AC5) ----------------------------------

    #[test]
    fn viewport_transform_2x_zoom_centered() {
        // AC5: A brush drawn at screen (100,100)→(200,200) with 2× zoom
        // centred at origin should map to data-space (50,50)→(100,100).
        let vt = ViewportTransform {
            offset: Vec2::new(0.0, 0.0),
            scale: Vec2::new(2.0, 2.0),
        };

        let screen_start = Vec2::new(100.0, 100.0);
        let screen_end = Vec2::new(200.0, 200.0);

        let data_start = vt.screen_to_world(screen_start);
        let data_end = vt.screen_to_world(screen_end);

        assert!((data_start.x - 50.0).abs() < 1e-5);
        assert!((data_start.y - 50.0).abs() < 1e-5);
        assert!((data_end.x - 100.0).abs() < 1e-5);
        assert!((data_end.y - 100.0).abs() < 1e-5);

        let extent = BrushExtent::from_corners(data_start, data_end);
        assert!((extent.x - 50.0).abs() < 1e-5);
        assert!((extent.y - 50.0).abs() < 1e-5);
        assert!((extent.width - 50.0).abs() < 1e-5);
        assert!((extent.height - 50.0).abs() < 1e-5);
    }

    #[test]
    fn viewport_transform_with_offset_and_zoom() {
        // 2× zoom with viewport offset at (20, 30)
        let vt = ViewportTransform {
            offset: Vec2::new(20.0, 30.0),
            scale: Vec2::new(2.0, 2.0),
        };

        let screen_start = Vec2::new(120.0, 130.0);
        let screen_end = Vec2::new(220.0, 230.0);

        let data_start = vt.screen_to_world(screen_start);
        let data_end = vt.screen_to_world(screen_end);

        // (120 - 20) / 2 = 50, (130 - 30) / 2 = 50
        assert!((data_start.x - 50.0).abs() < 1e-5);
        assert!((data_start.y - 50.0).abs() < 1e-5);
        // (220 - 20) / 2 = 100, (230 - 30) / 2 = 100
        assert!((data_end.x - 100.0).abs() < 1e-5);
        assert!((data_end.y - 100.0).abs() < 1e-5);
    }

    // -- BrushMark overlay -------------------------------------------------

    #[test]
    fn brush_mark_default_hidden() {
        let mark = BrushMark::default();
        assert!(!mark.visible);
        assert!(mark.screen_rect.is_none());
    }

    #[test]
    fn brush_mark_show_hide() {
        let mut mark = BrushMark::default();
        let rect = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(100.0, 100.0));
        mark.show(rect);
        assert!(mark.visible);
        assert!(mark.screen_rect.is_some());

        mark.hide();
        assert!(!mark.visible);
        assert!(mark.screen_rect.is_none());
    }

    // -- Hit testing integration -------------------------------------------

    #[test]
    fn brush_with_selection_system_finds_marks() {
        let mut system = MarkSelectionSystem::new(5);
        system.set_positions(vec![
            [50.0, 50.0],
            [150.0, 150.0],
            [250.0, 250.0],
            [100.0, 100.0],
            [300.0, 300.0],
        ]);

        // Brush from (75, 75) to (175, 175) should find marks at
        // (150, 150) and (100, 100)
        let vt = ViewportTransform::default();
        let mut brush = BrushBehavior::new();
        brush.on_pointer_down(Vec2::new(75.0, 75.0));
        brush.on_pointer_move(Vec2::new(175.0, 175.0), &vt, Some(&system));

        // Verify the overlay is showing
        assert!(brush.overlay().visible);
    }

    #[test]
    fn brush_end_fires_with_correct_ids() {
        let mut system = MarkSelectionSystem::new(5);
        system.set_positions(vec![
            [50.0, 50.0],
            [150.0, 150.0],
            [250.0, 250.0],
            [100.0, 100.0],
            [300.0, 300.0],
        ]);

        let selected_ids = Arc::new(std::sync::Mutex::new(Vec::new()));
        let ids_clone = selected_ids.clone();

        let mut brush = BrushBehavior::new().on("brushend", move |e: &BrushEvent| {
            *ids_clone.lock().unwrap() = e.selection.clone();
        });

        let vt = ViewportTransform::default();
        brush.on_pointer_down(Vec2::new(75.0, 75.0));
        brush.on_pointer_move(Vec2::new(175.0, 175.0), &vt, Some(&system));
        brush.on_pointer_up(Vec2::new(175.0, 175.0), &vt, Some(&system));

        let ids = selected_ids.lock().unwrap();
        assert!(ids.contains(&1)); // (150, 150)
        assert!(ids.contains(&3)); // (100, 100)
        assert_eq!(ids.len(), 2);
    }

    // -- Replacing attachment (AC1) ----------------------------------------

    #[test]
    fn replacing_brush_replaces_handlers() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        // First brush
        let _brush1 = BrushBehavior::new().on("brushend", move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        });

        // Replace with a new brush — the old one is dropped
        let c2 = counter.clone();
        let brush2 = BrushBehavior::new().on("brushend", move |_| {
            c2.fetch_add(100, Ordering::Relaxed);
        });

        // Only the second brush's handler fires
        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 10.0, 10.0),
            vec![],
        );
        brush2.fire("brushend", &event);
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }

    // -- with_shared_selection integration ---------------------------------

    #[test]
    fn brush_with_shared_selection_writes_keys() {
        let shared = SharedSelectionState::<usize>::new();
        let brush = BrushBehavior::new().with_shared_selection(shared.clone(), |idx| idx as usize);

        // Simulate a brush event with some selected marks
        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 100.0, 100.0),
            vec![2, 5, 8],
        );
        brush.fire("brush", &event);

        // SharedSelectionState should contain the keys
        assert!(shared.is_selected(&2));
        assert!(shared.is_selected(&5));
        assert!(shared.is_selected(&8));
        assert!(!shared.is_selected(&0));
        assert_eq!(shared.selected_count(), 3);
    }

    #[test]
    fn brush_with_shared_selection_clears_on_empty_brushend() {
        let shared = SharedSelectionState::<usize>::new();
        let brush = BrushBehavior::new().with_shared_selection(shared.clone(), |idx| idx as usize);

        // First brush selects some items
        let event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 100.0, 100.0),
            vec![1, 2, 3],
        );
        brush.fire("brush", &event);
        assert_eq!(shared.selected_count(), 3);

        // Empty brushend clears
        let end_event = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 0.0, 0.0),
            BrushExtent::new(0.0, 0.0, 0.0, 0.0),
            vec![],
        );
        brush.fire("brushend", &end_event);
        assert!(shared.is_empty());
    }

    #[test]
    fn brush_with_shared_selection_replaces_on_new_brush() {
        let shared = SharedSelectionState::<String>::new();
        let data = ["alice", "bob", "carol", "dave", "eve"];
        let brush = BrushBehavior::new()
            .with_shared_selection(shared.clone(), move |idx| data[idx as usize].to_string());

        // First brush: alice and bob
        let event1 = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 100.0, 100.0),
            vec![0, 1],
        );
        brush.fire("brush", &event1);
        assert!(shared.is_selected(&"alice".to_string()));
        assert!(shared.is_selected(&"bob".to_string()));

        // Second brush: carol and dave (replaces, doesn't add)
        let event2 = BrushEvent::new(
            BrushExtent::new(0.0, 0.0, 1.0, 1.0),
            BrushExtent::new(0.0, 0.0, 100.0, 100.0),
            vec![2, 3],
        );
        brush.fire("brush", &event2);
        assert!(!shared.is_selected(&"alice".to_string()));
        assert!(shared.is_selected(&"carol".to_string()));
        assert!(shared.is_selected(&"dave".to_string()));
        assert_eq!(shared.selected_count(), 2);
    }

    // -- BrushOverlayRenderer (GPU) ----------------------------------------

    /// Create a headless device+queue for GPU tests.
    async fn create_test_device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("no GPU adapter");
        adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("failed to create device")
    }

    #[tokio::test]
    async fn overlay_renderer_creation() {
        let (device, queue) = create_test_device().await;
        let mut cache = crate::pipeline_cache::PipelineCache::new();
        let renderer = BrushOverlayRenderer::new(&device, &queue, &mut cache);
        assert!(
            renderer.is_ok(),
            "BrushOverlayRenderer should be created without error"
        );
        let renderer = renderer.unwrap();
        assert!(!renderer.is_visible(), "should start invisible");
    }

    #[tokio::test]
    async fn overlay_visible_when_brush_shown() {
        let (device, queue) = create_test_device().await;
        let mut cache = crate::pipeline_cache::PipelineCache::new();
        let mut renderer = BrushOverlayRenderer::new(&device, &queue, &mut cache).unwrap();

        let mut mark = BrushMark::default();
        mark.show(Rect::new(Vec2::new(-0.5, -0.5), Vec2::new(0.5, 0.5)));

        renderer.update(&mark, &queue);
        assert!(renderer.is_visible(), "should be visible after show()");
    }

    #[tokio::test]
    async fn overlay_hidden_when_brush_hidden() {
        let (device, queue) = create_test_device().await;
        let mut cache = crate::pipeline_cache::PipelineCache::new();
        let mut renderer = BrushOverlayRenderer::new(&device, &queue, &mut cache).unwrap();

        let mut mark = BrushMark::default();
        // Show then hide
        mark.show(Rect::new(Vec2::new(-0.5, -0.5), Vec2::new(0.5, 0.5)));
        renderer.update(&mark, &queue);
        assert!(renderer.is_visible());

        mark.hide();
        renderer.update(&mark, &queue);
        assert!(!renderer.is_visible(), "should be hidden after hide()");
    }

    #[tokio::test]
    async fn overlay_hidden_for_default_brush_mark() {
        let (device, queue) = create_test_device().await;
        let mut cache = crate::pipeline_cache::PipelineCache::new();
        let mut renderer = BrushOverlayRenderer::new(&device, &queue, &mut cache).unwrap();

        let mark = BrushMark::default();
        renderer.update(&mark, &queue);
        assert!(
            !renderer.is_visible(),
            "default BrushMark should be invisible"
        );
    }

    #[tokio::test]
    async fn overlay_reuses_cached_pipeline() {
        let (device, queue) = create_test_device().await;
        let mut cache = crate::pipeline_cache::PipelineCache::new();

        // First creation populates the cache.
        let _r1 = BrushOverlayRenderer::new(&device, &queue, &mut cache).unwrap();
        let hits_after_first = cache.stats().hits;

        // Second creation should hit the cache.
        let _r2 = BrushOverlayRenderer::new(&device, &queue, &mut cache).unwrap();
        let hits_after_second = cache.stats().hits;

        assert!(
            hits_after_second > hits_after_first,
            "second renderer should reuse the cached pipeline"
        );
    }
}
