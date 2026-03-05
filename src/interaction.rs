// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated interaction system for high-performance hit testing and event handling.
//!
//! This module provides GPU-based spatial queries and collision detection for massive datasets,
//! enabling responsive hover, click, and selection interactions even with millions of data points.
//!
//! # Architecture
//!
//! The interaction system uses compute shaders to perform parallel hit testing against
//! GPU-resident spatial data structures. This approach provides:
//!
//! - **<1ms hit testing** for 1M+ points
//! - **Parallel processing** of all elements against queries
//! - **Spatial indexing** for efficient region queries
//! - **GPU-CPU synchronization** with minimal latency
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::interaction::{InteractionSystem, InteractionType};
//! use gup::RenderContext;
//! use std::sync::Arc;
//!
//! async fn setup_interaction() -> Result<(), Box<dyn std::error::Error>> {
//!     let context = Arc::new(RenderContext::new().await?);
//!     let mut interaction_system = InteractionSystem::new(&context).await?;
//!
//!     // Query point at screen coordinates
//!     let hits = interaction_system.query_point([100.0, 200.0], &selections).await?;
//!
//!     // Query rectangular region
//!     let region = Rect::new([50.0, 50.0], [150.0, 150.0]);
//!     let region_hits = interaction_system.query_region(region, &selections).await?;
//!
//!     Ok(())
//! }
//! ```

use crate::buffer::{BufferPool, BufferPoolConfig, BufferType as PoolBufferType};
use crate::error::{GupError, GupResult};
use crate::event::ModifierFlags;
use crate::spatial_index::{
    Aabb, ElementPosition, MortonEntry, SpatialAlgorithm, SpatialIndex, SpatialQuery,
};
use crate::{MaybeSend, MaybeSync, RenderContext};
use futures_channel;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineLayoutDescriptor, PollType, Queue,
    ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

/// Geometric shapes for spatial queries
pub use crate::math::Vec2;

/// Axis-aligned bounding rectangle defined by minimum and maximum corners.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    /// Minimum corner of the rectangle.
    pub min: Vec2,
    /// Maximum corner of the rectangle.
    pub max: Vec2,
}

impl Rect {
    /// Create a new rectangle from minimum and maximum corners.
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Create a new rectangle from a center point and size.
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half_size = size * 0.5;
        Self {
            min: center - half_size,
            max: center + half_size,
        }
    }

    /// Return the width of the rectangle.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Return the height of the rectangle.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Return the center point of the rectangle.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
}

/// Types of interaction queries supported by the GPU system
#[derive(Debug)]
pub enum InteractionType {
    /// Mouse hover at screen position
    Hover(Vec2),
    /// Mouse click at screen position
    Click(Vec2),
    /// Drag from start to end position
    Drag(Vec2, Vec2),
    /// Rectangular selection region
    RegionSelect(Rect),
    /// Custom interaction query
    Custom(Box<dyn CustomInteractionQuery>),
}

/// Trait for custom interaction queries
pub trait CustomInteractionQuery: MaybeSend + MaybeSync + std::fmt::Debug {
    /// Execute the query against GPU data
    fn execute(&self, system: &mut InteractionSystem) -> GupResult<Vec<ElementHit>>;

    /// Get a description of this query for debugging
    fn description(&self) -> String;
}

/// Result of a hit test query
#[derive(Debug, Clone)]
pub struct ElementHit {
    /// ID of the element that was hit
    pub element_id: u32,
    /// Selection ID this element belongs to
    pub selection_id: u32,
    /// Distance from query point to element center
    pub distance: f32,
    /// Intersection point in world coordinates
    pub intersection_point: Vec2,
    /// Additional metadata about the hit
    pub metadata: HashMap<String, String>,
}

impl ElementHit {
    /// Create a new element hit with the given identifiers, distance and intersection point.
    pub fn new(
        element_id: u32,
        selection_id: u32,
        distance: f32,
        intersection_point: Vec2,
    ) -> Self {
        Self {
            element_id,
            selection_id,
            distance,
            intersection_point,
            metadata: HashMap::new(),
        }
    }

    /// Attach a metadata key-value pair to this hit.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Represents a single touch point for multi-touch interactions
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Unique identifier for this touch
    pub id: u64,
    /// Screen position of the touch
    pub position: Vec2,
    /// Timestamp of the touch event
    pub timestamp: f64,
}

impl TouchPoint {
    /// Create a new touch point with the given id, position and timestamp.
    pub fn new(id: u64, position: Vec2, timestamp: f64) -> Self {
        Self {
            id,
            position,
            timestamp,
        }
    }
}

/// Multi-touch gesture types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureType {
    /// Two-finger pinch gesture (scale)
    Pinch {
        /// Center point of the pinch
        center: Vec2,
        /// Current scale factor (1.0 = no scale)
        scale: f32,
        /// Change in scale since last event
        delta_scale: f32,
    },
    /// Two-finger rotation gesture
    Rotate {
        /// Center point of the rotation
        center: Vec2,
        /// Current rotation angle in radians
        angle: f32,
        /// Change in angle since last event
        delta_angle: f32,
    },
    /// Swipe gesture with velocity
    Swipe {
        /// Start position
        start: Vec2,
        /// End position
        end: Vec2,
        /// Swipe direction vector (normalized)
        direction: Vec2,
        /// Swipe velocity in pixels per second
        velocity: f32,
    },
    /// Generic multi-touch pan
    Pan {
        /// Start position
        start: Vec2,
        /// Current position
        current: Vec2,
        /// Delta from last position
        delta: Vec2,
    },
}

/// Event handler trait for processing interaction events
pub trait EventHandler: MaybeSend + MaybeSync {
    /// Handle an interaction event
    fn handle_event(&self, event: &InteractionEvent);
}

/// Event propagation phase during event bubbling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropagationPhase {
    /// Capture phase - event travels down the hierarchy
    Capture,
    /// Target phase - event is at the target element
    Target,
    /// Bubble phase - event travels up the hierarchy
    Bubble,
}

/// Interaction event data with propagation support
#[derive(Debug, Clone)]
pub struct InteractionEvent {
    /// Type of interaction that occurred
    pub interaction_type: String,
    /// Screen coordinates of the interaction
    pub screen_position: Vec2,
    /// World coordinates of the interaction (if applicable)
    pub world_position: Option<Vec2>,
    /// Element that was hit (if any)
    pub hit: Option<ElementHit>,
    /// Additional event-specific data
    pub metadata: HashMap<String, String>,
    /// Whether propagation has been stopped
    propagation_stopped: bool,
    /// Whether immediate propagation has been stopped
    immediate_propagation_stopped: bool,
    /// Whether default behavior has been prevented
    default_prevented: bool,
    /// Current propagation phase
    phase: PropagationPhase,
    /// Active touch points (for multi-touch interactions)
    pub touch_points: Vec<TouchPoint>,
    /// Recognized gesture (if any)
    pub gesture: Option<GestureType>,
    /// Monotonic timestamp of the input event (if available).
    pub timestamp: Option<Instant>,
    /// Keyboard modifier flags at the time the event was dispatched.
    pub modifiers: ModifierFlags,
}

impl InteractionEvent {
    /// Create a new interaction event with the given type and screen position.
    pub fn new(interaction_type: &str, screen_position: Vec2) -> Self {
        Self {
            interaction_type: interaction_type.to_string(),
            screen_position,
            world_position: None,
            hit: None,
            metadata: HashMap::new(),
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            default_prevented: false,
            phase: PropagationPhase::Target,
            touch_points: Vec::new(),
            gesture: None,
            timestamp: None,
            modifiers: ModifierFlags::NONE,
        }
    }

    /// Set the world position for this event.
    pub fn with_world_position(mut self, world_position: Vec2) -> Self {
        self.world_position = Some(world_position);
        self
    }

    /// Attach an element hit to this event.
    pub fn with_hit(mut self, hit: ElementHit) -> Self {
        self.hit = Some(hit);
        self
    }

    /// Attach a metadata key-value pair to this event.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Add touch points to this event for multi-touch interactions.
    pub fn with_touch_points(mut self, touch_points: Vec<TouchPoint>) -> Self {
        self.touch_points = touch_points;
        self
    }

    /// Add a recognized gesture to this event.
    pub fn with_gesture(mut self, gesture: GestureType) -> Self {
        self.gesture = Some(gesture);
        self
    }

    /// Stop propagation of this event to other elements in the hierarchy.
    ///
    /// After calling this, the event will not bubble up or capture down to other elements,
    /// but remaining handlers on the current element will still execute.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// Stop immediate propagation of this event.
    ///
    /// After calling this, no further event handlers will execute, including handlers
    /// on the current element.
    pub fn stop_immediate_propagation(&mut self) {
        self.immediate_propagation_stopped = true;
        self.propagation_stopped = true;
    }

    /// Prevent the default behavior associated with this event.
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// Check if propagation has been stopped.
    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    /// Check if immediate propagation has been stopped.
    pub fn is_immediate_propagation_stopped(&self) -> bool {
        self.immediate_propagation_stopped
    }

    /// Check if default behavior has been prevented.
    pub fn is_default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// Get the current propagation phase.
    pub fn phase(&self) -> PropagationPhase {
        self.phase
    }

    /// Set the propagation phase (internal use only).
    #[allow(dead_code)]
    pub(crate) fn set_phase(&mut self, phase: PropagationPhase) {
        self.phase = phase;
    }
}

/// GPU data structures for compute shaders
///
/// Query data sent to GPU
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuInteractionQuery {
    /// 0 = point, 1 = region, 2 = custom
    pub query_type: u32,
    /// Maximum number of results to return
    pub max_results: u32,
    /// Query position or region center (aligned to 8-byte boundary)
    pub position: [f32; 2],
    /// For region queries: width and height
    pub region_size: [f32; 2],
    /// Padding for 16-byte alignment
    pub _padding: [u32; 2],
}

impl GpuInteractionQuery {
    /// Create a point query at the given position.
    pub fn point(position: Vec2, max_results: u32) -> Self {
        Self {
            query_type: 0,
            max_results,
            position: position.into(),
            region_size: [0.0, 0.0],
            _padding: [0; 2],
        }
    }

    /// Create a region query covering the given rectangle.
    pub fn region(rect: Rect, max_results: u32) -> Self {
        let center = rect.center();
        Self {
            query_type: 1,
            max_results,
            position: center.into(),
            region_size: [rect.width(), rect.height()],
            _padding: [0; 2],
        }
    }
}

/// Element data for GPU hit testing
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, serde::Serialize)]
pub struct ElementData {
    /// Center position of the element
    pub position: [f32; 2],
    /// Size of the element (width, height or radius, 0)
    pub size: [f32; 2],
    /// Mark type: 0 = circle, 1 = rectangle, 2 = line
    pub mark_type: u32,
    /// Element ID within its selection
    pub element_id: u32,
    /// Selection ID this element belongs to
    pub selection_id: u32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

/// Result data from GPU hit testing
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InteractionResult {
    /// Element ID that was hit
    pub element_id: u32,
    /// Selection ID
    pub selection_id: u32,
    /// Distance from query point to element
    pub distance: f32,
    /// 1 if hit, 0 if miss
    pub is_hit: u32,
    /// Intersection point (moved to 16-byte boundary)
    pub intersection_point: [f32; 2],
    /// Padding for 16-byte alignment
    pub _padding: [u32; 2],
}

/// Performance statistics for interaction queries
#[derive(Debug, Clone, Default)]
pub struct QueryStats {
    /// Total number of queries processed
    pub total_queries: u64,
    /// Total number of elements tested
    pub total_elements_tested: u64,
    /// Total number of hits found
    pub total_hits: u64,
    /// Average query time in microseconds
    pub average_query_time_us: f32,
    /// Maximum query time in microseconds
    pub max_query_time_us: f32,
}

impl QueryStats {
    /// Record the results of a single query.
    pub fn update(&mut self, elements_tested: u32, hits_found: u32, query_time_us: f32) {
        self.total_queries += 1;
        self.total_elements_tested += elements_tested as u64;
        self.total_hits += hits_found as u64;

        // Update average query time using running average
        let n = self.total_queries as f32;
        self.average_query_time_us = ((n - 1.0) * self.average_query_time_us + query_time_us) / n;

        if query_time_us > self.max_query_time_us {
            self.max_query_time_us = query_time_us;
        }
    }

    /// Return the ratio of hits to elements tested.
    pub fn hit_rate(&self) -> f32 {
        if self.total_elements_tested > 0 {
            self.total_hits as f32 / self.total_elements_tested as f32
        } else {
            0.0
        }
    }
}

/// Pending query awaiting GPU processing
#[derive(Debug)]
#[allow(dead_code)]
struct PendingQuery {
    query_id: u32,
    query_type: InteractionType,
    max_results: u32,
    submitted_at: std::time::Instant,
}

/// Spatial indexing configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpatialIndexConfig {
    /// Number of cells in X and Y
    pub grid_size: [u32; 2],
    /// Size of each cell in world units
    pub cell_size: [f32; 2],
    /// Minimum world coordinates
    pub world_bounds_min: [f32; 2],
    /// Maximum world coordinates
    pub world_bounds_max: [f32; 2],
}

/// GPU spatial cell data
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpatialCell {
    /// Number of elements in this cell.
    pub element_count: u32,
    /// Start index into the element array.
    pub element_start_index: u32,
    /// Minimum bounds of this cell in world coordinates.
    pub bounds_min: [f32; 2],
    /// Maximum bounds of this cell in world coordinates.
    pub bounds_max: [f32; 2],
}

/// Configuration for GPU-side Morton range query.
///
/// This struct is uploaded as a uniform buffer to the `morton_query` compute
/// shader. Layout matches the WGSL `MortonQueryConfig` struct.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MortonQueryConfig {
    /// Query type: 0 = point, 1 = region.
    pub query_type: u32,
    /// Search radius in grid cells for point queries.
    pub search_radius: u32,
    /// Total number of entries in the sorted Morton buffer.
    pub entry_count: u32,
    /// Maximum number of candidates to output.
    pub max_candidates: u32,
    /// Query position in world coordinates.
    pub query_position: [f32; 2],
    /// Query region half-extents (for region queries).
    pub query_half_extent: [f32; 2],
    /// World bounds min.
    pub world_bounds_min: [f32; 2],
    /// World bounds max.
    pub world_bounds_max: [f32; 2],
}

/// Configuration for the hit test compute shader.
///
/// Uploaded as a uniform buffer so the shader uses the actual query count
/// for result indexing instead of `arrayLength(&queries)` (the buffer capacity).
/// This allows single-query dispatches to test up to `max_results` candidates
/// rather than being limited to `max_results / max_queries`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HitTestConfig {
    /// Actual number of queries dispatched (not the buffer capacity).
    pub query_count: u32,
    /// Padding for 16-byte alignment.
    pub _padding: [u32; 3],
}

/// GPU-accelerated interaction system for high-performance hit testing
pub struct InteractionSystem {
    /// GPU compute pipeline for hit testing
    hit_test_pipeline: ComputePipeline,
    /// GPU compute pipeline for spatial indexing (build phase)
    #[allow(dead_code)]
    spatial_index_pipeline: ComputePipeline,
    /// GPU compute pipeline for spatial index population phase
    #[allow(dead_code)]
    spatial_populate_pipeline: ComputePipeline,
    /// Explicit bind group layout for spatial index pipelines
    spatial_bind_group_layout: BindGroupLayout,

    /// GPU buffers for query processing
    element_buffer: Buffer,
    query_buffer: Buffer,
    result_buffer: Buffer,
    /// Hit test config uniform (carries actual query count).
    hit_test_config_buffer: Buffer,

    /// Spatial indexing buffers
    spatial_cells_buffer: Buffer,
    element_indices_buffer: Buffer,
    spatial_config_buffer: Buffer,

    /// GPU-side Morton range query resources
    morton_query_pipeline: ComputePipeline,
    morton_query_bind_group_layout: BindGroupLayout,
    morton_entries_buffer: Buffer,
    morton_query_config_buffer: Buffer,
    morton_candidates_buffer: Buffer,
    morton_candidate_count_buffer: Buffer,
    /// Whether the GPU Morton entries buffer has been populated.
    morton_gpu_index_built: bool,
    /// Number of Morton entries currently on the GPU.
    morton_gpu_entry_count: u32,

    /// Persistent staging buffer for result readback (GUP-197).
    /// Created once and reused across queries to eliminate per-query buffer
    /// allocation overhead.
    result_staging_buffer: Buffer,

    /// GPU-resident candidate pipeline resources (GUP-193).
    /// Gather compute pipeline that compacts candidates on the GPU.
    gather_pipeline: ComputePipeline,
    gather_bind_group_layout: BindGroupLayout,
    /// Compacted candidate elements (written by gather, read by hit test).
    gathered_element_buffer: Buffer,
    /// Indirect dispatch arguments for the hit test (written by gather).
    hit_test_indirect_buffer: Buffer,

    /// CPU-side management
    event_handlers: HashMap<String, Vec<Box<dyn EventHandler>>>,
    #[allow(dead_code)]
    active_queries: Vec<PendingQuery>,
    #[allow(dead_code)]
    next_query_id: u32,

    /// Performance monitoring
    query_stats: QueryStats,

    /// Spatial indexing configuration
    spatial_config: SpatialIndexConfig,
    spatial_index_built: bool,
    /// CPU-side spatial index: sorted element indices per cell
    spatial_element_indices: Vec<u32>,
    /// Advanced spatial index (Morton or Hierarchical, built lazily)
    advanced_spatial_index: Option<SpatialIndex>,
    /// Which algorithm to use for the advanced spatial index
    spatial_algorithm: SpatialAlgorithm,

    /// GPU resources
    device: Arc<Device>,
    queue: Arc<Queue>,

    /// Buffer capacities
    max_elements: usize,
    max_queries: usize,
    #[allow(dead_code)]
    max_results: usize,
    max_spatial_cells: usize,
    max_morton_candidates: usize,

    /// Cached element data version. When a caller uploads element data with
    /// a matching version and count, the upload is skipped and the GPU-resident
    /// buffer is reused.
    cached_element_version: u64,
    /// Number of elements currently cached on the GPU.
    cached_element_count: usize,

    /// Number of result slots written by the last compute dispatch (GUP-197).
    /// Used to limit the staging buffer copy size in `download_results()`.
    /// A value of 0 means "copy the full buffer" (conservative fallback).
    last_dispatch_result_slots: usize,

    /// Buffer pool for staging buffers used in GPU readback operations (GUP-079).
    /// Reuses MAP_READ staging buffers across Morton count/candidate readbacks
    /// instead of creating and destroying them per query.
    staging_pool: BufferPool,

    /// Double-buffered staging slots for non-blocking queries (GUP-198).
    /// Two slots allow a new query to be submitted while a previous result is
    /// still being read, enabling CPU-GPU overlap.
    async_staging_slots: [AsyncStagingSlot; 2],
}

impl InteractionSystem {
    /// Create a new GPU interaction system
    pub async fn new(context: &RenderContext) -> GupResult<Self> {
        let device = context.device();
        let queue = context.queue();

        // Create compute pipelines
        let hit_test_pipeline = Self::create_hit_test_pipeline(device).await?;

        // Create explicit bind group layout for spatial index pipelines.
        // Using an explicit layout ensures all 4 bindings are present regardless
        // of which entry point is used. Auto-layout would omit unused bindings.
        let spatial_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("spatial_index_bind_group_layout"),
                entries: &[
                    // binding 0: elements (storage, read-only)
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 1: spatial_cells (storage, read-write)
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 2: element_indices (storage, read-write)
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 3: spatial_index config (uniform)
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let spatial_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("spatial_index_pipeline_layout"),
            bind_group_layouts: &[&spatial_bind_group_layout],
            push_constant_ranges: &[],
        });

        let (spatial_index_pipeline, spatial_populate_pipeline) =
            Self::create_spatial_index_pipelines(device, &spatial_pipeline_layout).await?;

        // Create GPU buffers with reasonable initial capacities
        let max_elements = 1_000_000; // Support up to 1M elements for performance target
        let max_queries = 32; // Process up to 32 queries simultaneously
        let max_results = 100_000; // Store up to 100K results
        let max_spatial_cells = 10_000; // 100x100 grid for spatial indexing
        let max_morton_candidates = 100_000; // Up to 100K candidates from Morton query

        let element_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("interaction_elements"),
            size: (max_elements * std::mem::size_of::<ElementData>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let query_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("interaction_queries"),
            size: (max_queries * std::mem::size_of::<GpuInteractionQuery>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let result_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("interaction_results"),
            size: (max_results * std::mem::size_of::<InteractionResult>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Persistent staging buffer for result readback (GUP-197).
        // Created once and reused across queries to avoid per-query allocation.
        let result_staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("result_staging_persistent"),
            size: (max_results * std::mem::size_of::<InteractionResult>()) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let hit_test_config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("hit_test_config"),
            size: std::mem::size_of::<HitTestConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Spatial indexing buffers
        let spatial_cells_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("spatial_cells"),
            size: (max_spatial_cells * std::mem::size_of::<SpatialCell>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let element_indices_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("element_indices"),
            size: (max_elements * std::mem::size_of::<u32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let spatial_config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("spatial_config"),
            size: std::mem::size_of::<SpatialIndexConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // GPU-side Morton range query resources
        let (morton_query_pipeline, morton_query_bind_group_layout) =
            Self::create_morton_query_pipeline(device).await?;

        let morton_entries_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("morton_entries"),
            size: (max_elements * std::mem::size_of::<MortonEntry>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let morton_query_config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("morton_query_config"),
            size: std::mem::size_of::<MortonQueryConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let morton_candidates_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("morton_candidates"),
            size: (max_morton_candidates * std::mem::size_of::<u32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Candidate count buffer: 4 bytes for an atomic u32 counter.
        let morton_candidate_count_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("morton_candidate_count"),
            size: std::mem::size_of::<u32>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // GPU-resident candidate pipeline resources (GUP-193)
        let (gather_pipeline, gather_bind_group_layout) =
            Self::create_gather_pipeline(device).await?;

        let gathered_element_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gathered_elements"),
            size: (max_morton_candidates * std::mem::size_of::<ElementData>()) as u64,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Indirect dispatch args for the hit test: 3 × u32.
        let hit_test_indirect_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("hit_test_indirect"),
            size: (3 * std::mem::size_of::<u32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Default spatial configuration (will be updated based on data bounds)
        let spatial_config = SpatialIndexConfig {
            grid_size: [100, 100],   // 100x100 grid
            cell_size: [10.0, 10.0], // 10 units per cell
            world_bounds_min: [0.0, 0.0],
            world_bounds_max: [1000.0, 1000.0],
        };

        // Staging buffer pool (GUP-079): reuse MAP_READ buffers across readbacks.
        // Use a small, focused configuration – Morton readbacks are small and
        // frequent, so a few pooled buffers with a short eviction timeout suffice.
        let staging_pool = BufferPool::with_config(
            Arc::new(device.clone()),
            BufferPoolConfig {
                max_buffers_per_pool: 4,
                max_total_memory: Some(4 * 1024 * 1024), // 4 MB for staging
                enable_adaptive_sizing: true,
                ..Default::default()
            },
        );

        // Double-buffered staging for non-blocking queries (GUP-198).
        let async_staging_size = (max_results * std::mem::size_of::<InteractionResult>()) as u64;
        let async_staging_a = AsyncStagingSlot {
            buffer: Arc::new(device.create_buffer(&BufferDescriptor {
                label: Some("async_staging_a"),
                size: async_staging_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })),
            in_use: Arc::new(AtomicBool::new(false)),
        };
        let async_staging_b = AsyncStagingSlot {
            buffer: Arc::new(device.create_buffer(&BufferDescriptor {
                label: Some("async_staging_b"),
                size: async_staging_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })),
            in_use: Arc::new(AtomicBool::new(false)),
        };

        Ok(Self {
            hit_test_pipeline,
            spatial_index_pipeline,
            spatial_populate_pipeline,
            spatial_bind_group_layout,
            element_buffer,
            query_buffer,
            result_buffer,
            result_staging_buffer,
            hit_test_config_buffer,
            spatial_cells_buffer,
            element_indices_buffer,
            spatial_config_buffer,
            morton_query_pipeline,
            morton_query_bind_group_layout,
            morton_entries_buffer,
            morton_query_config_buffer,
            morton_candidates_buffer,
            morton_candidate_count_buffer,
            morton_gpu_index_built: false,
            morton_gpu_entry_count: 0,
            gather_pipeline,
            gather_bind_group_layout,
            gathered_element_buffer,
            hit_test_indirect_buffer,
            event_handlers: HashMap::new(),
            active_queries: Vec::new(),
            next_query_id: 0,
            query_stats: QueryStats::default(),
            spatial_config,
            spatial_index_built: false,
            spatial_element_indices: Vec::new(),
            advanced_spatial_index: None,
            spatial_algorithm: SpatialAlgorithm::Auto,
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            max_elements,
            max_queries,
            max_results,
            max_spatial_cells,
            max_morton_candidates,
            cached_element_version: 0,
            cached_element_count: 0,
            last_dispatch_result_slots: 0,
            staging_pool,
            async_staging_slots: [async_staging_a, async_staging_b],
        })
    }

    /// Create the compute pipeline for GPU hit testing
    async fn create_hit_test_pipeline(device: &Device) -> GupResult<ComputePipeline> {
        let shader_source = include_str!("shaders/hit_test.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("hit_test_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("hit_test_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("hit_test_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(compute_pipeline)
    }

    /// Create compute pipelines for spatial indexing using explicit layout.
    ///
    /// Returns pipelines for the build and populate entry points. The prefix-sum
    /// (offset computation) is performed on the CPU because a correct parallel
    /// prefix-sum in WGSL requires multiple dispatch passes and workgroup
    /// synchronisation that adds complexity without measurable benefit for
    /// typical grid sizes (≤10 000 cells).
    async fn create_spatial_index_pipelines(
        device: &Device,
        layout: &wgpu::PipelineLayout,
    ) -> GupResult<(ComputePipeline, ComputePipeline)> {
        let shader_source = include_str!("shaders/spatial_index.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("spatial_index_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let build_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("spatial_index_build_pipeline"),
            layout: Some(layout),
            module: &shader_module,
            entry_point: Some("build_spatial_index"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let populate_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("spatial_index_populate_pipeline"),
            layout: Some(layout),
            module: &shader_module,
            entry_point: Some("populate_element_indices"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok((build_pipeline, populate_pipeline))
    }

    /// Create the compute pipeline and bind group layout for GPU-side Morton
    /// range queries.
    async fn create_morton_query_pipeline(
        device: &Device,
    ) -> GupResult<(ComputePipeline, BindGroupLayout)> {
        let shader_source = include_str!("shaders/morton_query.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("morton_query_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Explicit bind group layout for the Morton query shader.
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("morton_query_bind_group_layout"),
            entries: &[
                // binding 0: sorted Morton entries (storage, read-only)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: query config (uniform)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: candidate output (storage, read-write)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: candidate count (storage, read-write)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("morton_query_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("morton_query_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("morton_range_query"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok((pipeline, bind_group_layout))
    }

    /// Create the compute pipeline and bind group layout for the GPU-resident
    /// gather pass (GUP-193).
    ///
    /// The gather shader reads candidate indices + the full element buffer and
    /// writes a compacted candidate buffer plus indirect dispatch arguments.
    async fn create_gather_pipeline(
        device: &Device,
    ) -> GupResult<(ComputePipeline, BindGroupLayout)> {
        let shader_source = include_str!("shaders/gather_candidates.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("gather_candidates_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("gather_bind_group_layout"),
            entries: &[
                // binding 0: all elements (storage, read-only)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: candidate indices (storage, read-only)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: candidate count (storage, read-only)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: gathered elements output (storage, read-write)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: indirect dispatch args (storage, read-write)
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("gather_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("gather_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("gather_candidates"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok((pipeline, bind_group_layout))
    }

    /// Query for elements at a specific point
    pub async fn query_point(
        &mut self,
        position: Vec2,
        selections: &[&dyn Renderable],
    ) -> GupResult<Vec<ElementHit>> {
        let query = GpuInteractionQuery::point(position, 1000);
        self.execute_query(query, selections).await
    }

    /// Query for elements within a rectangular region
    pub async fn query_region(
        &mut self,
        region: Rect,
        selections: &[&dyn Renderable],
    ) -> GupResult<Vec<ElementHit>> {
        let query = GpuInteractionQuery::region(region, 10000);
        self.execute_query(query, selections).await
    }

    /// Execute multiple queries in a single GPU dispatch for better performance
    pub async fn query_batch(
        &mut self,
        queries: &[GpuInteractionQuery],
        selections: &[&dyn Renderable],
    ) -> GupResult<Vec<Vec<ElementHit>>> {
        let start_time = std::time::Instant::now();

        // Extract element data from selections
        let elements = self.extract_element_data(selections)?;
        if elements.is_empty() {
            return Ok(vec![Vec::new(); queries.len()]);
        }

        // Build spatial index for large datasets
        if elements.len() > 1000 && !self.spatial_index_built {
            self.build_spatial_index(&elements).await?;
        }

        // Upload data to GPU
        self.upload_element_data(&elements)?;
        self.upload_query_data(queries)?;

        // Execute all queries in parallel
        self.dispatch_hit_test_compute(elements.len(), queries.len())
            .await?;

        // Download and process results
        let results = self.download_results().await?;
        let processed_results = self.process_batch_results(&results, queries.len());

        // Update performance statistics
        let query_time_us = start_time.elapsed().as_micros() as f32;
        let total_hits: usize = processed_results.iter().map(|hits| hits.len()).sum();
        self.query_stats.update(
            elements.len() as u32 * queries.len() as u32,
            total_hits as u32,
            query_time_us,
        );

        Ok(processed_results)
    }

    /// Process batch query results into separate hit lists per query
    fn process_batch_results(
        &self,
        results: &[InteractionResult],
        query_count: usize,
    ) -> Vec<Vec<ElementHit>> {
        let mut batch_results = vec![Vec::new(); query_count];

        for result in results {
            if result.is_hit != 0 {
                // Determine which query this result belongs to based on result index
                // This is a simplified approach - in practice would need more sophisticated mapping
                let query_index = (result.element_id / 1000) as usize % query_count;
                if query_index < batch_results.len() {
                    let hit = ElementHit::new(
                        result.element_id,
                        result.selection_id,
                        result.distance,
                        Vec2::new(result.intersection_point[0], result.intersection_point[1]),
                    );
                    batch_results[query_index].push(hit);
                }
            }
        }

        batch_results
    }

    /// Stream query results for very large datasets to reduce memory usage
    pub async fn query_stream<F>(
        &mut self,
        query: GpuInteractionQuery,
        selections: &[&dyn Renderable],
        mut callback: F,
    ) -> GupResult<()>
    where
        F: FnMut(ElementHit) -> bool, // Return false to stop streaming
    {
        let elements = self.extract_element_data(selections)?;
        if elements.is_empty() {
            return Ok(());
        }

        // Process in chunks to manage memory usage
        const CHUNK_SIZE: usize = 100_000;

        for chunk_start in (0..elements.len()).step_by(CHUNK_SIZE) {
            let chunk_end = (chunk_start + CHUNK_SIZE).min(elements.len());
            let chunk = &elements[chunk_start..chunk_end];

            // Upload chunk to GPU
            self.upload_element_data(chunk)?;
            self.upload_query_data(&[query])?;

            // Execute query on chunk
            self.dispatch_hit_test_compute(chunk.len(), 1).await?;

            // Stream results
            let results = self.download_results().await?;
            for result in &results {
                if result.is_hit != 0 {
                    let hit = ElementHit::new(
                        result.element_id + chunk_start as u32, // Adjust for chunk offset
                        result.selection_id,
                        result.distance,
                        Vec2::new(result.intersection_point[0], result.intersection_point[1]),
                    );

                    if !callback(hit) {
                        return Ok(()); // Early termination requested
                    }
                }
            }
        }

        Ok(())
    }

    // -- Cached element data API (GUP-194) --

    /// Upload element data to the GPU with version-based caching.
    ///
    /// If `version` matches the previously uploaded version and the element
    /// count has not changed, the upload is skipped and the GPU-resident buffer
    /// is reused. When a cache miss occurs the element data is uploaded and the
    /// spatial index is (re-)built for large datasets.
    ///
    /// Returns `true` on a cache miss (data was uploaded), `false` on a hit.
    pub async fn upload_element_data_cached(
        &mut self,
        elements: &[ElementData],
        version: u64,
    ) -> GupResult<bool> {
        // A version of 0 is treated as "never cached" to avoid accidental
        // matches with the default initial value.
        if version != 0
            && version == self.cached_element_version
            && elements.len() == self.cached_element_count
        {
            return Ok(false); // Cache hit — GPU buffer is still valid.
        }

        // Cache miss — upload element data.
        self.upload_element_data(elements)?;

        // Invalidate and rebuild spatial index for large datasets.
        self.spatial_index_built = false;
        self.morton_gpu_index_built = false;
        self.morton_gpu_entry_count = 0;
        self.advanced_spatial_index = None;

        if elements.len() > 1000 {
            self.build_spatial_index(elements).await?;
        }

        self.cached_element_version = version;
        self.cached_element_count = elements.len();

        Ok(true)
    }

    /// Invalidate the element data cache, forcing the next
    /// [`upload_element_data_cached`](Self::upload_element_data_cached) call
    /// to re-upload even if the version matches.
    pub fn invalidate_element_cache(&mut self) {
        self.cached_element_version = 0;
        self.cached_element_count = 0;
        self.spatial_index_built = false;
        self.morton_gpu_index_built = false;
        self.morton_gpu_entry_count = 0;
        self.advanced_spatial_index = None;
    }

    /// Query for elements at a point using previously cached element data.
    ///
    /// The caller must have uploaded element data via
    /// [`upload_element_data_cached`](Self::upload_element_data_cached) before
    /// calling this method. Returns an empty vec if no data is cached.
    pub async fn query_point_cached(&mut self, position: Vec2) -> GupResult<Vec<ElementHit>> {
        let query = GpuInteractionQuery::point(position, 1000);
        self.execute_query_cached(query).await
    }

    /// Query for elements within a rectangular region using cached element data.
    ///
    /// See [`query_point_cached`](Self::query_point_cached) for usage.
    pub async fn query_region_cached(&mut self, region: Rect) -> GupResult<Vec<ElementHit>> {
        let query = GpuInteractionQuery::region(region, 10000);
        self.execute_query_cached(query).await
    }

    // -- Non-blocking query API (GUP-198) --

    /// Submit a point query and return a [`QueryHandle`] without waiting for
    /// the GPU to finish.
    ///
    /// The caller must have uploaded element data via
    /// [`upload_element_data_cached`](Self::upload_element_data_cached) before
    /// calling this method.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let handle = system.query_point_async(Vec2::new(100.0, 200.0)).await?;
    /// // … do CPU work …
    /// let hits = handle.await_result().await?;
    /// ```
    pub async fn query_point_async(&mut self, position: Vec2) -> GupResult<QueryHandle> {
        let query = GpuInteractionQuery::point(position, 1000);
        self.execute_query_cached_async(query).await
    }

    /// Submit a region query and return a [`QueryHandle`] without waiting for
    /// the GPU to finish.
    ///
    /// See [`query_point_async`](Self::query_point_async) for usage.
    pub async fn query_region_async(&mut self, region: Rect) -> GupResult<QueryHandle> {
        let query = GpuInteractionQuery::region(region, 10000);
        self.execute_query_cached_async(query).await
    }

    /// Internal: dispatch a cached query and start an asynchronous readback,
    /// returning a [`QueryHandle`] that can be polled or awaited.
    async fn execute_query_cached_async(
        &mut self,
        query: GpuInteractionQuery,
    ) -> GupResult<QueryHandle> {
        if self.cached_element_count == 0 {
            return Ok(QueryHandle::empty());
        }

        let element_count = self.cached_element_count;

        // Upload query data.
        self.upload_query_data(&[query])?;

        // Dispatch compute shader (same paths as the sync cached API).
        if element_count > 1000 && self.morton_gpu_index_built {
            self.dispatch_gpu_morton_query_cached(query).await?;
        } else {
            self.dispatch_hit_test_compute(element_count, 1).await?;
        }

        // Start non-blocking readback via double-buffered staging.
        self.start_async_download()
    }

    /// Pick a free async staging slot, copy the result buffer into it, and
    /// initiate `map_async`.  Returns a [`QueryHandle`] wrapping the pending
    /// readback.
    fn start_async_download(&mut self) -> GupResult<QueryHandle> {
        let slot_idx = self.pick_async_staging_slot()?;
        let slot = &self.async_staging_slots[slot_idx];

        let result_entry_size = std::mem::size_of::<InteractionResult>() as u64;
        let staging_size = slot.buffer.size();

        // Copy only the portion that was actually written (GUP-197 pattern).
        let copy_size = if self.last_dispatch_result_slots > 0 {
            let needed = self.last_dispatch_result_slots as u64 * result_entry_size;
            needed.min(staging_size)
        } else {
            staging_size
        };

        // Copy result buffer → async staging buffer.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("async_result_copy"),
            });
        encoder.copy_buffer_to_buffer(&self.result_buffer, 0, &slot.buffer, 0, copy_size);
        self.queue.submit([encoder.finish()]);

        // Initiate non-blocking map.
        let buffer_slice = slot.buffer.slice(..copy_size);
        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Mark the slot as in-use.
        slot.in_use.store(true, Ordering::Release);

        Ok(QueryHandle {
            inner: Some(QueryHandleInner {
                map_receiver: receiver,
                staging_buffer: Arc::clone(&slot.buffer),
                copy_size,
                device: Arc::clone(&self.device),
                slot_in_use: Arc::clone(&slot.in_use),
            }),
        })
    }

    /// Find a free async staging slot.  Returns an error if both are busy.
    fn pick_async_staging_slot(&self) -> GupResult<usize> {
        for (i, slot) in self.async_staging_slots.iter().enumerate() {
            if !slot.in_use.load(Ordering::Acquire) {
                return Ok(i);
            }
        }
        Err(GupError::render_error(
            "Both async staging buffers are in use; consume or drop pending QueryHandles first"
                .to_string(),
        ))
    }

    /// Returns the current cached element data version.
    pub fn cached_element_version(&self) -> u64 {
        self.cached_element_version
    }

    /// Returns the number of elements currently cached on the GPU.
    pub fn cached_element_count(&self) -> usize {
        self.cached_element_count
    }

    /// Execute a query using cached GPU-resident element data (GUP-194).
    ///
    /// Unlike [`execute_query`](Self::execute_query), this method does **not**
    /// extract or upload element data — it assumes the data is already on the
    /// GPU from a prior [`upload_element_data_cached`] call.
    async fn execute_query_cached(
        &mut self,
        query: GpuInteractionQuery,
    ) -> GupResult<Vec<ElementHit>> {
        let start_time = std::time::Instant::now();

        if self.cached_element_count == 0 {
            return Ok(Vec::new());
        }

        let element_count = self.cached_element_count;

        // Upload query data.
        self.upload_query_data(&[query])?;

        // Dispatch compute shader.
        // Prefer the fully GPU-resident Morton path when available.
        if element_count > 1000 && self.morton_gpu_index_built {
            // The GPU Morton query pipeline reads element data directly from
            // the GPU buffer, so no CPU-side elements are needed.
            self.dispatch_gpu_morton_query_cached(query).await?;
        } else {
            self.dispatch_hit_test_compute(element_count, 1).await?;
        }

        // Download results.
        let results = self.download_results().await?;
        let hits = self.process_results(&results);

        // Update performance statistics.
        let query_time_us = start_time.elapsed().as_micros() as f32;
        self.query_stats
            .update(element_count as u32, hits.len() as u32, query_time_us);

        Ok(hits)
    }

    /// Dispatch a GPU-side Morton query using cached element data.
    ///
    /// This is a streamlined variant of [`dispatch_gpu_morton_query`] that
    /// does not require a CPU-side `&[ElementData]` slice.
    async fn dispatch_gpu_morton_query_cached(
        &mut self,
        query: GpuInteractionQuery,
    ) -> GupResult<()> {
        if !self.morton_gpu_index_built || self.morton_gpu_entry_count == 0 {
            return self
                .dispatch_hit_test_compute(self.cached_element_count, 1)
                .await;
        }

        let bounds = match &self.advanced_spatial_index {
            Some(SpatialIndex::Morton(idx)) => *idx.bounds(),
            _ => {
                return self
                    .dispatch_hit_test_compute(self.cached_element_count, 1)
                    .await;
            }
        };

        // Build query config.
        let (qtype, half_ext) = if query.query_type == 0 {
            (0u32, [0.0f32, 0.0f32])
        } else {
            (
                1u32,
                [query.region_size[0] * 0.5, query.region_size[1] * 0.5],
            )
        };

        let config = MortonQueryConfig {
            query_type: qtype,
            search_radius: 512,
            entry_count: self.morton_gpu_entry_count,
            max_candidates: self.max_morton_candidates as u32,
            query_position: query.position,
            query_half_extent: half_ext,
            world_bounds_min: bounds.min,
            world_bounds_max: bounds.max,
        };

        self.queue.write_buffer(
            &self.morton_query_config_buffer,
            0,
            bytemuck::bytes_of(&config),
        );
        self.queue
            .write_buffer(&self.morton_candidate_count_buffer, 0, &[0u8; 4]);
        self.queue
            .write_buffer(&self.hit_test_indirect_buffer, 0, &[0u8; 12]);

        let hit_test_config = HitTestConfig {
            query_count: 1,
            _padding: [0; 3],
        };
        self.queue.write_buffer(
            &self.hit_test_config_buffer,
            0,
            bytemuck::bytes_of(&hit_test_config),
        );

        // Encode all three passes in a single command encoder (same as
        // dispatch_gpu_morton_query but without requiring a CPU elements slice).
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("morton_query_cached_encoder"),
            });

        // Pass 1: Morton range query.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("morton_query_cached_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.morton_query_pipeline);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("morton_query_cached_bg"),
                layout: &self.morton_query_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.morton_entries_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.morton_query_config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.morton_candidates_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.morton_candidate_count_buffer.as_entire_binding(),
                    },
                ],
            });
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = (self.morton_gpu_entry_count as usize).div_ceil(256);
            pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        // Pass 2: Gather candidates.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("gather_cached_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.gather_pipeline);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gather_cached_bg"),
                layout: &self.gather_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.morton_candidates_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.morton_candidate_count_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.element_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.gathered_element_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.hit_test_indirect_buffer.as_entire_binding(),
                    },
                ],
            });
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = self.max_morton_candidates.div_ceil(256);
            pass.dispatch_workgroups(workgroups as u32, 1, 1);
        }

        // Pass 3: Hit test (indirect dispatch).
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("hit_test_cached_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hit_test_pipeline);
            let bind_group_layout = self.hit_test_pipeline.get_bind_group_layout(0);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hit_test_cached_bg"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.gathered_element_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.query_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.result_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.hit_test_config_buffer.as_entire_binding(),
                    },
                ],
            });
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups_indirect(&self.hit_test_indirect_buffer, 0);
        }

        self.queue.submit([encoder.finish()]);

        // For the indirect path we don't know the exact candidate count, but
        // it is bounded by max_morton_candidates (GUP-197).
        self.last_dispatch_result_slots = self.max_morton_candidates;

        Ok(())
    }

    /// Execute a GPU query
    async fn execute_query(
        &mut self,
        query: GpuInteractionQuery,
        selections: &[&dyn Renderable],
    ) -> GupResult<Vec<ElementHit>> {
        let start_time = std::time::Instant::now();

        // Extract element data from selections
        let elements = self.extract_element_data(selections)?;
        if elements.is_empty() {
            return Ok(Vec::new());
        }

        // Build spatial index for datasets with enough elements to benefit
        if elements.len() > 1000 && !self.spatial_index_built {
            self.build_spatial_index(&elements).await?;
        }

        // Upload data to GPU
        self.upload_element_data(&elements)?;
        self.upload_query_data(&[query])?;

        // Execute compute shader – when a spatial index is available, use it
        // to narrow candidates before GPU dispatch. Otherwise brute-force.
        if elements.len() > 1000 && self.spatial_index_built {
            // Prefer GPU-side Morton query when the entries are on the GPU.
            if self.morton_gpu_index_built {
                self.dispatch_gpu_morton_query(query, &elements).await?;
            } else {
                self.dispatcher_spatial_query(query, &elements).await?;
            }
        } else {
            self.dispatch_hit_test_compute(elements.len(), 1).await?;
        }

        // Download results
        let results = self.download_results().await?;

        // Process results into ElementHit structs
        let hits = self.process_results(&results);

        // Update performance statistics
        let query_time_us = start_time.elapsed().as_micros() as f32;
        self.query_stats
            .update(elements.len() as u32, hits.len() as u32, query_time_us);

        Ok(hits)
    }

    /// Dispatch spatial-indexed query for better performance on large datasets.
    ///
    /// Uses the advanced spatial index (Morton or Hierarchical) to determine
    /// which elements are candidates for the query, then uploads only those
    /// candidates to the GPU hit test pipeline. For large datasets with
    /// localised queries this dramatically reduces GPU work.
    async fn dispatcher_spatial_query(
        &mut self,
        query: GpuInteractionQuery,
        elements: &[ElementData],
    ) -> GupResult<()> {
        // If no advanced index is available, fall back to brute-force
        let adv_index = match &self.advanced_spatial_index {
            Some(idx) => idx,
            None => return self.dispatch_hit_test_compute(elements.len(), 1).await,
        };

        // Determine candidate elements based on query type
        let candidates = if query.query_type == 0 {
            // Point query
            adv_index.query_point(query.position)
        } else {
            // Region query
            let half_w = query.region_size[0] * 0.5;
            let half_h = query.region_size[1] * 0.5;
            let region = Aabb::new(
                [query.position[0] - half_w, query.position[1] - half_h],
                [query.position[0] + half_w, query.position[1] + half_h],
            );
            adv_index.query_region(&region)
        };

        if candidates.is_empty() {
            return Ok(());
        }

        // Build a reduced element list from candidates
        let candidate_elements: Vec<ElementData> = candidates
            .iter()
            .filter_map(|&idx| elements.get(idx as usize).copied())
            .collect();

        // Upload only the candidate elements and run the hit test
        self.upload_element_data(&candidate_elements)?;
        self.dispatch_hit_test_compute(candidate_elements.len(), 1)
            .await
    }

    /// Dispatch a spatial query entirely on the GPU using Morton range search
    /// and a GPU-resident gather pipeline (GUP-193).
    ///
    /// The sorted Morton entries are already resident on the GPU. This method:
    /// 1. Uploads the query configuration to a uniform buffer.
    /// 2. Resets the candidate count to zero.
    /// 3. Encodes three compute passes in a single command encoder:
    ///    a. Morton range query — binary search on sorted entries.
    ///    b. Gather — compacts candidate elements and writes indirect args.
    ///    c. Hit test — dispatched indirectly using the gather output.
    /// 4. No GPU→CPU→GPU readback occurs in the query hot path.
    async fn dispatch_gpu_morton_query(
        &mut self,
        query: GpuInteractionQuery,
        _elements: &[ElementData],
    ) -> GupResult<()> {
        // Require the GPU Morton index to have been built.
        if !self.morton_gpu_index_built || self.morton_gpu_entry_count == 0 {
            return self.dispatch_hit_test_compute(_elements.len(), 1).await;
        }

        // Look up the Morton index bounds from the advanced spatial index.
        let bounds = match &self.advanced_spatial_index {
            Some(SpatialIndex::Morton(idx)) => *idx.bounds(),
            _ => return self.dispatch_hit_test_compute(_elements.len(), 1).await,
        };

        // Build query config.
        let (qtype, half_ext) = if query.query_type == 0 {
            (0u32, [0.0f32, 0.0f32])
        } else {
            (
                1u32,
                [query.region_size[0] * 0.5, query.region_size[1] * 0.5],
            )
        };

        let config = MortonQueryConfig {
            query_type: qtype,
            search_radius: 512, // matches CPU-side radius
            entry_count: self.morton_gpu_entry_count,
            max_candidates: self.max_morton_candidates as u32,
            query_position: query.position,
            query_half_extent: half_ext,
            world_bounds_min: bounds.min,
            world_bounds_max: bounds.max,
        };

        // Upload config.
        self.queue.write_buffer(
            &self.morton_query_config_buffer,
            0,
            bytemuck::bytes_of(&config),
        );

        // Reset candidate count to zero.
        self.queue
            .write_buffer(&self.morton_candidate_count_buffer, 0, &[0u8; 4]);

        // Also zero the indirect dispatch buffer so a zero-candidate query
        // dispatches zero workgroups for the hit test.
        self.queue
            .write_buffer(&self.hit_test_indirect_buffer, 0, &[0u8; 12]);

        // Upload hit test config so the shader knows the actual query count
        // (always 1 for the GPU-resident Morton query path).
        let hit_test_config = HitTestConfig {
            query_count: 1,
            _padding: [0; 3],
        };
        self.queue.write_buffer(
            &self.hit_test_config_buffer,
            0,
            bytemuck::bytes_of(&hit_test_config),
        );

        // --- Encode all three passes in a single command encoder ---
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_resident_query_encoder"),
            });

        // Pass 1: Morton range query — binary search on sorted entries.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("morton_query_pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.morton_query_pipeline);

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("morton_query_bind_group"),
                layout: &self.morton_query_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.morton_entries_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.morton_query_config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.morton_candidates_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.morton_candidate_count_buffer.as_entire_binding(),
                    },
                ],
            });
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        // Pass 2: Gather — compact candidate elements and write indirect args.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("gather_candidates_pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.gather_pipeline);

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gather_bind_group"),
                layout: &self.gather_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.element_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.morton_candidates_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.morton_candidate_count_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.gathered_element_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: self.hit_test_indirect_buffer.as_entire_binding(),
                    },
                ],
            });
            pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch enough workgroups to cover all possible candidates.
            let gather_workgroup_size = 256;
            let gather_dispatch_x =
                (self.max_morton_candidates as u32).div_ceil(gather_workgroup_size);
            pass.dispatch_workgroups(gather_dispatch_x, 1, 1);
        }

        // Pass 3: Hit test — dispatched indirectly using gather output.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("hit_test_indirect_pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.hit_test_pipeline);

            // Bind the gathered (compacted) element buffer instead of the
            // full element buffer so the hit test operates on candidates only.
            let bind_group = self.create_gathered_hit_test_bind_group()?;
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups_indirect(&self.hit_test_indirect_buffer, 0);
        }

        self.queue.submit([encoder.finish()]);

        // For the indirect path we don't know the exact candidate count, but
        // it is bounded by max_morton_candidates (GUP-197).
        self.last_dispatch_result_slots = self.max_morton_candidates;

        Ok(())
    }

    /// Create a hit test bind group that reads from the gathered (compacted)
    /// element buffer instead of the full element buffer.
    fn create_gathered_hit_test_bind_group(&self) -> GupResult<wgpu::BindGroup> {
        let bind_group_layout = self.hit_test_pipeline.get_bind_group_layout(0);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gathered_hit_test_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.gathered_element_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.result_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.hit_test_config_buffer.as_entire_binding(),
                },
            ],
        });

        Ok(bind_group)
    }

    /// Read the Morton candidate count back from the GPU.
    ///
    /// Uses a pooled staging buffer (GUP-079) to avoid per-call allocation.
    async fn read_morton_candidate_count(&mut self) -> GupResult<u32> {
        let byte_size = std::mem::size_of::<u32>();
        let (staging, size_class) = self
            .staging_pool
            .allocate_raw(PoolBufferType::Staging, byte_size);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("morton_count_copy_encoder"),
            });
        encoder.copy_buffer_to_buffer(
            &self.morton_candidate_count_buffer,
            0,
            &staging,
            0,
            byte_size as u64,
        );
        let sub_idx = self.queue.submit([encoder.finish()]);
        let _ = self.device.poll(PollType::Wait {
            submission_index: Some(sub_idx),
            timeout: None,
        });

        let slice = staging.slice(..byte_size as u64);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let map_result = receiver
            .await
            .map_err(|_| GupError::render_error("Morton count readback cancelled".to_string()))?
            .map_err(|e| GupError::render_error(format!("Morton count map failed: {e:?}")));

        if let Err(e) = map_result {
            self.staging_pool
                .deallocate_raw(staging, PoolBufferType::Staging, size_class);
            return Err(e);
        }

        let data = slice.get_mapped_range();
        let count = *bytemuck::from_bytes::<u32>(&data);
        drop(data);
        staging.unmap();
        self.staging_pool
            .deallocate_raw(staging, PoolBufferType::Staging, size_class);

        // Clamp to max to avoid OOB reads.
        Ok(count.min(self.max_morton_candidates as u32))
    }

    /// Read Morton candidate indices from the GPU.
    ///
    /// Uses a pooled staging buffer (GUP-079) to avoid per-call allocation.
    async fn read_morton_candidates(&mut self, count: u32) -> GupResult<Vec<u32>> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let byte_size = (count as usize * std::mem::size_of::<u32>()) as u64;
        let (staging, size_class) = self
            .staging_pool
            .allocate_raw(PoolBufferType::Staging, byte_size as usize);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("morton_candidates_copy_encoder"),
            });
        encoder.copy_buffer_to_buffer(&self.morton_candidates_buffer, 0, &staging, 0, byte_size);
        let sub_idx = self.queue.submit([encoder.finish()]);
        let _ = self.device.poll(PollType::Wait {
            submission_index: Some(sub_idx),
            timeout: None,
        });

        let slice = staging.slice(..byte_size);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let map_result = receiver
            .await
            .map_err(|_| {
                GupError::render_error("Morton candidates readback cancelled".to_string())
            })?
            .map_err(|e| GupError::render_error(format!("Morton candidates map failed: {e:?}")));

        if let Err(e) = map_result {
            self.staging_pool
                .deallocate_raw(staging, PoolBufferType::Staging, size_class);
            return Err(e);
        }

        let data = slice.get_mapped_range();
        let indices: Vec<u32> = bytemuck::cast_slice::<u8, u32>(&data).to_vec();
        drop(data);
        staging.unmap();
        self.staging_pool
            .deallocate_raw(staging, PoolBufferType::Staging, size_class);

        Ok(indices)
    }

    /// Extract element data from selections for GPU processing
    fn extract_element_data(&self, selections: &[&dyn Renderable]) -> GupResult<Vec<ElementData>> {
        let mut elements = Vec::new();

        for (selection_id, selection) in selections.iter().enumerate() {
            // Extract elements from each selection
            let selection_elements = selection.get_elements_for_interaction()?;

            for (element_id, element) in selection_elements.iter().enumerate() {
                let element_data = ElementData {
                    position: element.position,
                    size: element.size,
                    mark_type: element.mark_type,
                    element_id: element_id as u32,
                    selection_id: selection_id as u32,
                    _padding: 0,
                };

                elements.push(element_data);
            }
        }

        Ok(elements)
    }

    /// Upload element data to GPU
    fn upload_element_data(&self, elements: &[ElementData]) -> GupResult<()> {
        if elements.len() > self.max_elements {
            return Err(GupError::render_error(format!(
                "Too many elements: {} > {}",
                elements.len(),
                self.max_elements
            )));
        }

        let data = bytemuck::cast_slice(elements);
        self.queue.write_buffer(&self.element_buffer, 0, data);

        Ok(())
    }

    /// Upload query data to GPU
    fn upload_query_data(&self, queries: &[GpuInteractionQuery]) -> GupResult<()> {
        if queries.len() > self.max_queries {
            return Err(GupError::render_error(format!(
                "Too many queries: {} > {}",
                queries.len(),
                self.max_queries
            )));
        }

        let data = bytemuck::cast_slice(queries);
        self.queue.write_buffer(&self.query_buffer, 0, data);

        Ok(())
    }

    /// Dispatch the hit test compute shader
    async fn dispatch_hit_test_compute(
        &mut self,
        element_count: usize,
        query_count: usize,
    ) -> GupResult<()> {
        // Track result slots for optimised readback copy size (GUP-197).
        self.last_dispatch_result_slots = element_count * query_count;

        // Upload the hit test config so the shader knows the actual query count.
        let config = HitTestConfig {
            query_count: query_count as u32,
            _padding: [0; 3],
        };
        self.queue
            .write_buffer(&self.hit_test_config_buffer, 0, bytemuck::bytes_of(&config));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hit_test_encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("hit_test_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.hit_test_pipeline);

            // Create bind group for buffers
            let bind_group = self.create_compute_bind_group()?;
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch with workgroup size 256 (defined in shader)
            let workgroup_size = 256;
            let dispatch_x = element_count.div_ceil(workgroup_size);
            let dispatch_y = query_count;

            compute_pass.dispatch_workgroups(dispatch_x as u32, dispatch_y as u32, 1);
        }

        self.queue.submit([encoder.finish()]);

        Ok(())
    }

    /// Create bind group for compute shader
    fn create_compute_bind_group(&self) -> GupResult<wgpu::BindGroup> {
        let bind_group_layout = self.hit_test_pipeline.get_bind_group_layout(0);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hit_test_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.element_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.result_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.hit_test_config_buffer.as_entire_binding(),
                },
            ],
        });

        Ok(bind_group)
    }

    /// Download results from GPU using the persistent staging buffer (GUP-197).
    ///
    /// Reuses a pre-allocated staging buffer instead of creating a new one per
    /// query, eliminating the per-query buffer allocation overhead. Only the
    /// portion of the result buffer that was actually written is copied, and
    /// the copy and map are combined into a single poll cycle.
    async fn download_results(&mut self) -> GupResult<Vec<InteractionResult>> {
        let result_entry_size = std::mem::size_of::<InteractionResult>() as u64;
        let staging_size = self.result_staging_buffer.size();

        // Only copy the portion of the result buffer that was actually written.
        // `last_dispatch_result_slots` is set by dispatch methods; 0 means
        // "copy everything" as a conservative fallback.
        let copy_size = if self.last_dispatch_result_slots > 0 {
            let needed = self.last_dispatch_result_slots as u64 * result_entry_size;
            needed.min(staging_size)
        } else {
            staging_size
        };

        // Copy from result buffer to the persistent staging buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("result_copy_encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &self.result_buffer,
            0,
            &self.result_staging_buffer,
            0,
            copy_size,
        );

        self.queue.submit([encoder.finish()]);

        // Request the map immediately after the copy submission.
        // The map will not complete until the copy is finished.
        // Only map the region we actually copied.
        let buffer_slice = self.result_staging_buffer.slice(..copy_size);

        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Single poll to wait for both copy and map to complete.
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        receiver
            .await
            .map_err(|_| {
                GupError::render_error("Failed to receive buffer mapping result".to_string())
            })?
            .map_err(|e| GupError::render_error(format!("Buffer mapping failed: {e:?}")))?;

        let data = buffer_slice.get_mapped_range();
        let results: &[InteractionResult] = bytemuck::cast_slice(&data);

        // Filter out non-hits and convert to Vec
        let hits: Vec<InteractionResult> = results
            .iter()
            .filter(|result| result.is_hit != 0)
            .copied()
            .collect();

        drop(data);
        self.result_staging_buffer.unmap();

        Ok(hits)
    }

    /// Process GPU results into ElementHit structs
    fn process_results(&self, results: &[InteractionResult]) -> Vec<ElementHit> {
        results
            .iter()
            .map(|result| {
                ElementHit::new(
                    result.element_id,
                    result.selection_id,
                    result.distance,
                    Vec2::new(result.intersection_point[0], result.intersection_point[1]),
                )
            })
            .collect()
    }

    /// Build spatial index for the current element data.
    ///
    /// Constructs a grid-based spatial index on the CPU and uploads the results
    /// to GPU buffers. The CPU approach avoids race conditions inherent in
    /// parallel GPU counting without atomics, and the prefix-sum over cells is
    /// trivially fast on CPU (≤10 000 cells).
    async fn build_spatial_index(&mut self, elements: &[ElementData]) -> GupResult<()> {
        if elements.is_empty() {
            return Ok(());
        }

        // Calculate optimal spatial configuration based on data bounds
        let (min_bounds, max_bounds) = self.calculate_data_bounds(elements);
        self.spatial_config.world_bounds_min = [min_bounds.x, min_bounds.y];
        self.spatial_config.world_bounds_max = [max_bounds.x, max_bounds.y];

        // Adapt grid resolution based on element count (GUP-176).
        // Heuristic: side = clamp(√N, MIN_GRID_SIDE, max_side) so that
        // small datasets don't waste memory on empty cells and large
        // datasets get finer resolution up to the buffer limit.
        let adaptive_side = Self::adaptive_grid_side(elements.len(), self.max_spatial_cells);
        self.spatial_config.grid_size = [adaptive_side as u32, adaptive_side as u32];

        let grid_w = self.spatial_config.grid_size[0] as usize;
        let grid_h = self.spatial_config.grid_size[1] as usize;
        let total_cells = grid_w * grid_h;

        let world_size = Vec2::new(max_bounds.x - min_bounds.x, max_bounds.y - min_bounds.y);
        // Avoid division by zero for degenerate bounds
        let cell_size_x = if world_size.x > 0.0 {
            world_size.x / grid_w as f32
        } else {
            1.0
        };
        let cell_size_y = if world_size.y > 0.0 {
            world_size.y / grid_h as f32
        } else {
            1.0
        };
        self.spatial_config.cell_size = [cell_size_x, cell_size_y];

        // --- Phase 1: Count elements per cell ---
        let mut cell_counts = vec![0u32; total_cells];
        for element in elements {
            let cell_idx = self.world_to_cell_index(element.position);
            if cell_idx < total_cells {
                cell_counts[cell_idx] += 1;
            }
        }

        // --- Phase 2: Prefix sum to compute start offsets ---
        let mut cell_offsets = vec![0u32; total_cells];
        let mut running = 0u32;
        for i in 0..total_cells {
            cell_offsets[i] = running;
            running += cell_counts[i];
        }

        // --- Phase 3: Populate element indices ---
        let total_indexed = running as usize;
        self.spatial_element_indices = vec![0u32; total_indexed];
        // Use a temporary write-cursor per cell so we can fill in order
        let mut cursors = cell_offsets.clone();
        for (elem_idx, element) in elements.iter().enumerate() {
            let cell_idx = self.world_to_cell_index(element.position);
            if cell_idx < total_cells {
                let pos = cursors[cell_idx] as usize;
                if pos < total_indexed {
                    self.spatial_element_indices[pos] = elem_idx as u32;
                    cursors[cell_idx] += 1;
                }
            }
        }

        // --- Phase 4: Build SpatialCell array and upload ---
        let mut cells = vec![
            SpatialCell {
                element_count: 0,
                element_start_index: 0,
                bounds_min: [0.0, 0.0],
                bounds_max: [0.0, 0.0],
            };
            total_cells.min(self.max_spatial_cells)
        ];
        for i in 0..cells.len() {
            let cx = (i % grid_w) as f32;
            let cy = (i / grid_w) as f32;
            cells[i] = SpatialCell {
                element_count: cell_counts[i],
                element_start_index: cell_offsets[i],
                bounds_min: [
                    min_bounds.x + cx * cell_size_x,
                    min_bounds.y + cy * cell_size_y,
                ],
                bounds_max: [
                    min_bounds.x + (cx + 1.0) * cell_size_x,
                    min_bounds.y + (cy + 1.0) * cell_size_y,
                ],
            };
        }

        // Upload spatial configuration
        let config_data = bytemuck::bytes_of(&self.spatial_config);
        self.queue
            .write_buffer(&self.spatial_config_buffer, 0, config_data);

        // Upload spatial cells
        let cells_data = bytemuck::cast_slice(&cells);
        self.queue
            .write_buffer(&self.spatial_cells_buffer, 0, cells_data);

        // Upload element indices
        if !self.spatial_element_indices.is_empty() {
            let indices_data = bytemuck::cast_slice(&self.spatial_element_indices);
            self.queue
                .write_buffer(&self.element_indices_buffer, 0, indices_data);
        }

        self.spatial_index_built = true;

        // Build advanced spatial index for narrowing candidates
        let positions: Vec<ElementPosition> = elements
            .iter()
            .enumerate()
            .map(|(i, e)| ElementPosition {
                position: e.position,
                size: e.size,
                element_index: i as u32,
            })
            .collect();
        let adv_bounds = Aabb::new([min_bounds.x, min_bounds.y], [max_bounds.x, max_bounds.y]);
        self.advanced_spatial_index = Some(SpatialIndex::build(
            self.spatial_algorithm,
            &positions,
            adv_bounds,
        ));

        // If the selected algorithm is Morton, upload the sorted entries to GPU
        // so that GPU-side range queries can bypass the CPU.
        self.morton_gpu_index_built = false;
        self.morton_gpu_entry_count = 0;
        if let Some(SpatialIndex::Morton(ref morton_idx)) = self.advanced_spatial_index {
            let entries = morton_idx.entries();
            if !entries.is_empty() && entries.len() <= self.max_elements {
                let data = bytemuck::cast_slice(entries);
                self.queue
                    .write_buffer(&self.morton_entries_buffer, 0, data);
                self.morton_gpu_entry_count = entries.len() as u32;
                self.morton_gpu_index_built = true;
            }
        }

        Ok(())
    }

    /// Convert a world-space position to a flat cell index in the spatial grid.
    fn world_to_cell_index(&self, position: [f32; 2]) -> usize {
        let grid_w = self.spatial_config.grid_size[0] as usize;
        let grid_h = self.spatial_config.grid_size[1] as usize;

        let min = self.spatial_config.world_bounds_min;
        let max = self.spatial_config.world_bounds_max;
        let range_x = max[0] - min[0];
        let range_y = max[1] - min[1];

        if range_x <= 0.0 || range_y <= 0.0 {
            return 0;
        }

        let nx = ((position[0] - min[0]) / range_x).clamp(0.0, 1.0 - f32::EPSILON);
        let ny = ((position[1] - min[1]) / range_y).clamp(0.0, 1.0 - f32::EPSILON);

        let cx = (nx * grid_w as f32) as usize;
        let cy = (ny * grid_h as f32) as usize;

        cy * grid_w + cx
    }

    /// Calculate data bounds for spatial indexing
    fn calculate_data_bounds(&self, elements: &[ElementData]) -> (Vec2, Vec2) {
        if elements.is_empty() {
            return (Vec2::new(0.0, 0.0), Vec2::new(1000.0, 1000.0));
        }

        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for element in elements {
            let pos = element.position;
            let size = element.size;

            // Account for element size in bounds calculation
            let element_min_x = pos[0] - size[0] * 0.5;
            let element_max_x = pos[0] + size[0] * 0.5;
            let element_min_y = pos[1] - size[1] * 0.5;
            let element_max_y = pos[1] + size[1] * 0.5;

            min_x = min_x.min(element_min_x);
            max_x = max_x.max(element_max_x);
            min_y = min_y.min(element_min_y);
            max_y = max_y.max(element_max_y);
        }

        // Add some padding to bounds
        let padding = 10.0;
        (
            Vec2::new(min_x - padding, min_y - padding),
            Vec2::new(max_x + padding, max_y + padding),
        )
    }

    /// Compute the adaptive grid side length for the spatial index (GUP-176).
    ///
    /// The heuristic uses `√N` (square-root of element count) so that each cell
    /// contains roughly one element on average for a uniform distribution.
    /// The result is clamped between [`Self::MIN_GRID_SIDE`] and the largest
    /// side that still fits within `max_cells` (the pre-allocated buffer).
    fn adaptive_grid_side(element_count: usize, max_cells: usize) -> usize {
        /// Minimum grid side length – avoids degenerate grids for tiny datasets.
        const MIN_GRID_SIDE: usize = 4;

        let sqrt = (element_count as f64).sqrt().ceil() as usize;
        let side = sqrt.max(MIN_GRID_SIDE);

        // Cap so that side² ≤ max_cells.
        let max_side = (max_cells as f64).sqrt() as usize;
        side.min(max_side.max(MIN_GRID_SIDE))
    }

    /// Create bind group for spatial indexing compute shader.
    ///
    /// Used by the GPU-side spatial query dispatch (GUP-078).
    #[allow(dead_code)]
    fn create_spatial_index_bind_group(&self) -> GupResult<wgpu::BindGroup> {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("spatial_index_bind_group"),
            layout: &self.spatial_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.element_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.spatial_cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.element_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.spatial_config_buffer.as_entire_binding(),
                },
            ],
        });

        Ok(bind_group)
    }

    /// Returns `true` if the spatial index has been built for the current data.
    pub fn is_spatial_index_built(&self) -> bool {
        self.spatial_index_built
    }

    /// Returns the current spatial index configuration.
    pub fn spatial_config(&self) -> &SpatialIndexConfig {
        &self.spatial_config
    }

    /// Invalidate the spatial index so it will be rebuilt on the next query.
    pub fn invalidate_spatial_index(&mut self) {
        self.spatial_index_built = false;
        self.spatial_element_indices.clear();
        self.advanced_spatial_index = None;
        self.morton_gpu_index_built = false;
        self.morton_gpu_entry_count = 0;
    }

    /// Build the spatial index from raw element data (public for testing).
    pub async fn build_spatial_index_from_elements(
        &mut self,
        elements: &[ElementData],
    ) -> GupResult<()> {
        self.build_spatial_index(elements).await
    }

    /// Dispatch a GPU-side Morton query and return the candidate element
    /// indices. Public for testing and benchmarking.
    ///
    /// Requires the GPU Morton index to have been built first via
    /// `build_spatial_index_from_elements` with a Morton-compatible algorithm.
    pub async fn gpu_morton_query(&mut self, query: GpuInteractionQuery) -> GupResult<Vec<u32>> {
        if !self.morton_gpu_index_built || self.morton_gpu_entry_count == 0 {
            return Err(GupError::render_error(
                "GPU Morton index not built".to_string(),
            ));
        }

        let bounds = match &self.advanced_spatial_index {
            Some(SpatialIndex::Morton(idx)) => *idx.bounds(),
            _ => {
                return Err(GupError::render_error(
                    "Advanced spatial index is not Morton".to_string(),
                ));
            }
        };

        let (qtype, half_ext) = if query.query_type == 0 {
            (0u32, [0.0f32, 0.0f32])
        } else {
            (
                1u32,
                [query.region_size[0] * 0.5, query.region_size[1] * 0.5],
            )
        };

        let config = MortonQueryConfig {
            query_type: qtype,
            search_radius: 512,
            entry_count: self.morton_gpu_entry_count,
            max_candidates: self.max_morton_candidates as u32,
            query_position: query.position,
            query_half_extent: half_ext,
            world_bounds_min: bounds.min,
            world_bounds_max: bounds.max,
        };

        self.queue.write_buffer(
            &self.morton_query_config_buffer,
            0,
            bytemuck::bytes_of(&config),
        );
        self.queue
            .write_buffer(&self.morton_candidate_count_buffer, 0, &[0u8; 4]);

        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("morton_query_test_encoder"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("morton_query_test_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.morton_query_pipeline);
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("morton_query_test_bind_group"),
                    layout: &self.morton_query_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.morton_entries_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.morton_query_config_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.morton_candidates_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.morton_candidate_count_buffer.as_entire_binding(),
                        },
                    ],
                });
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            self.queue.submit([encoder.finish()]);
        }

        let count = self.read_morton_candidate_count().await?;
        if count == 0 {
            return Ok(Vec::new());
        }
        self.read_morton_candidates(count).await
    }

    /// Set the spatial algorithm to use. Invalidates any existing index.
    pub fn set_spatial_algorithm(&mut self, algorithm: SpatialAlgorithm) {
        if self.spatial_algorithm != algorithm {
            self.spatial_algorithm = algorithm;
            self.invalidate_spatial_index();
        }
    }

    /// Returns the current spatial algorithm setting.
    pub fn spatial_algorithm(&self) -> SpatialAlgorithm {
        self.spatial_algorithm
    }

    /// Returns the algorithm that was actually selected (relevant when using `Auto`).
    pub fn active_spatial_algorithm(&self) -> Option<SpatialAlgorithm> {
        self.advanced_spatial_index
            .as_ref()
            .map(|idx| idx.algorithm())
    }

    /// Returns memory usage of the advanced spatial index in bytes, or 0 if none.
    pub fn advanced_index_memory_bytes(&self) -> usize {
        self.advanced_spatial_index
            .as_ref()
            .map_or(0, |idx| idx.memory_usage_bytes())
    }

    /// Returns `true` if the GPU-side Morton entries buffer has been populated.
    pub fn is_gpu_morton_index_built(&self) -> bool {
        self.morton_gpu_index_built
    }

    /// Returns the number of Morton entries currently on the GPU.
    pub fn gpu_morton_entry_count(&self) -> u32 {
        self.morton_gpu_entry_count
    }

    /// Register an event handler for a specific event type
    pub fn register_event_handler<F>(&mut self, event_type: &str, handler: F)
    where
        F: Fn(&InteractionEvent) + MaybeSend + MaybeSync + 'static,
    {
        struct FnHandler<F> {
            f: F,
        }

        impl<F> EventHandler for FnHandler<F>
        where
            F: Fn(&InteractionEvent) + MaybeSend + MaybeSync + 'static,
        {
            fn handle_event(&self, event: &InteractionEvent) {
                (self.f)(event);
            }
        }

        let boxed_handler = Box::new(FnHandler { f: handler });
        self.event_handlers
            .entry(event_type.to_string())
            .or_default()
            .push(boxed_handler);
    }

    /// Process an interaction event and trigger appropriate handlers
    pub async fn process_interaction_event(&mut self, event: InteractionEvent) -> GupResult<()> {
        // Fire event handlers
        if let Some(handlers) = self.event_handlers.get(&event.interaction_type) {
            for handler in handlers {
                handler.handle_event(&event);
            }
        }

        Ok(())
    }

    /// Get performance statistics
    pub fn query_stats(&self) -> &QueryStats {
        &self.query_stats
    }

    /// Get buffer pool statistics for staging buffers (GUP-079).
    ///
    /// Returns allocation statistics including pool hit rate, active/pooled
    /// buffer counts, and total bytes allocated through the staging pool.
    pub fn staging_pool_stats(&self) -> &crate::buffer::AllocationStats {
        self.staging_pool.get_stats()
    }

    /// Clean up unused staging buffers to free GPU memory (GUP-079).
    ///
    /// Removes staging buffers that haven't been used within the configured
    /// eviction timeout and enforces pool size limits.
    pub fn cleanup_staging_pool(&mut self) {
        self.staging_pool.cleanup_unused();
    }

    /// Reset performance statistics
    pub fn reset_stats(&mut self) {
        self.query_stats = QueryStats::default();
    }
}

/// Trait for objects that can be queried for interactions
pub trait Renderable: MaybeSend + MaybeSync {
    /// Get elements for interaction processing
    fn get_elements_for_interaction(&self) -> GupResult<Vec<InteractionElement>>;

    /// Get the selection ID for this renderable
    fn selection_id(&self) -> u32;
}

/// Element data for interaction processing
#[derive(Debug, Clone)]
pub struct InteractionElement {
    /// Position in world coordinates.
    pub position: [f32; 2],
    /// Width and height of the element.
    pub size: [f32; 2],
    /// Mark type identifier for the element.
    pub mark_type: u32,
}

// -- Non-blocking query API (GUP-198) --

/// Internal state for a double-buffered async staging slot.
struct AsyncStagingSlot {
    buffer: Arc<Buffer>,
    in_use: Arc<AtomicBool>,
}

/// Handle for a pending non-blocking GPU hit test query (GUP-198).
///
/// Created by [`InteractionSystem::query_point_async`] or
/// [`InteractionSystem::query_region_async`]. The result can be polled
/// non-blockingly via [`poll_result`](Self::poll_result) or consumed
/// via [`await_result`](Self::await_result).
///
/// # Frame-aligned usage
///
/// In a render loop the typical pattern is:
///
/// ```rust,ignore
/// // Frame N: submit query
/// let handle = system.query_point_async(position).await?;
///
/// // Frame N+1: consume result (GPU has already finished)
/// if let Some(hits) = handle.poll_result()? {
///     process(hits);
/// }
/// ```
///
/// Because the GPU completes the work between frames, `poll_result`
/// returns the answer with effectively zero perceived latency.
pub struct QueryHandle {
    inner: Option<QueryHandleInner>,
}

struct QueryHandleInner {
    /// Receives the `map_async` completion signal.
    map_receiver: futures_channel::oneshot::Receiver<Result<(), wgpu::BufferAsyncError>>,
    /// Staging buffer holding the mapped result data.
    staging_buffer: Arc<Buffer>,
    /// Byte count of the valid region in the staging buffer.
    copy_size: u64,
    /// Device handle for driving non-blocking polls.
    device: Arc<Device>,
    /// Shared flag — cleared when this handle releases the staging slot.
    slot_in_use: Arc<AtomicBool>,
}

impl QueryHandle {
    /// Create a handle that immediately resolves to an empty result set.
    fn empty() -> Self {
        Self { inner: None }
    }

    /// Returns `true` if the result has already been consumed or was empty.
    pub fn is_consumed(&self) -> bool {
        self.inner.is_none()
    }

    /// Poll for the query result without blocking.
    ///
    /// Returns `Ok(None)` if the GPU has not yet finished, `Ok(Some(hits))`
    /// once the result is available, or `Err` on failure.
    ///
    /// Each call performs a non-blocking `device.poll(PollType::Poll)` to
    /// drive any pending callbacks, so results may become available on
    /// successive calls even without external device polling.
    pub fn poll_result(&mut self) -> GupResult<Option<Vec<ElementHit>>> {
        let inner = match self.inner.as_mut() {
            None => return Ok(Some(Vec::new())),
            Some(inner) => inner,
        };

        // Drive the device without blocking.
        let _ = inner.device.poll(PollType::Poll);

        match inner.map_receiver.try_recv() {
            Ok(Some(Ok(()))) => {
                // Mapping complete — read data and release the slot.
                let results = Self::read_and_unmap(&inner.staging_buffer, inner.copy_size)?;
                inner.slot_in_use.store(false, Ordering::Release);
                self.inner = None;
                Ok(Some(results))
            }
            Ok(Some(Err(e))) => {
                inner.slot_in_use.store(false, Ordering::Release);
                self.inner = None;
                Err(GupError::render_error(format!(
                    "Buffer mapping failed: {e:?}"
                )))
            }
            Ok(None) => Ok(None), // Not ready yet.
            Err(futures_channel::oneshot::Canceled) => {
                inner.slot_in_use.store(false, Ordering::Release);
                self.inner = None;
                Err(GupError::render_error(
                    "Query handle channel closed unexpectedly".to_string(),
                ))
            }
        }
    }

    /// Await the query result.
    ///
    /// Performs a blocking `device.poll(PollType::Wait { submission_index: None, timeout: None })` so the map callback
    /// fires, then reads and returns the hit test results. In frame-aligned
    /// usage where the GPU has already finished, this returns almost
    /// immediately.
    pub async fn await_result(mut self) -> GupResult<Vec<ElementHit>> {
        let inner = match self.inner.take() {
            None => return Ok(Vec::new()),
            Some(inner) => inner,
        };

        let QueryHandleInner {
            map_receiver,
            staging_buffer,
            copy_size,
            device,
            slot_in_use,
        } = inner;

        // Block until GPU work completes and the map callback fires.
        let _ = device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        let map_result = map_receiver.await.map_err(|_| {
            slot_in_use.store(false, Ordering::Release);
            GupError::render_error("Query channel closed".to_string())
        })?;

        map_result.map_err(|e| {
            slot_in_use.store(false, Ordering::Release);
            staging_buffer.unmap();
            GupError::render_error(format!("Buffer mapping failed: {e:?}"))
        })?;

        let results = Self::read_and_unmap(&staging_buffer, copy_size)?;
        slot_in_use.store(false, Ordering::Release);
        Ok(results)
    }

    /// Read interaction results from a mapped staging buffer and unmap it.
    fn read_and_unmap(staging_buffer: &Buffer, copy_size: u64) -> GupResult<Vec<ElementHit>> {
        let buffer_slice = staging_buffer.slice(..copy_size);
        let data = buffer_slice.get_mapped_range();
        let results: &[InteractionResult] = bytemuck::cast_slice(&data);

        let hits: Vec<ElementHit> = results
            .iter()
            .filter(|r| r.is_hit != 0)
            .map(|r| {
                ElementHit::new(
                    r.element_id,
                    r.selection_id,
                    r.distance,
                    Vec2::new(r.intersection_point[0], r.intersection_point[1]),
                )
            })
            .collect();

        drop(data);
        staging_buffer.unmap();
        Ok(hits)
    }
}

impl Drop for QueryHandle {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            // Release the staging slot so it can be reused.
            // Unmapping an unmapped buffer is a safe no-op in wgpu.
            inner.staging_buffer.unmap();
            inner.slot_in_use.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_creation() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);

        let v2: Vec2 = [3.0, 4.0].into();
        assert_eq!(v2.x, 3.0);
        assert_eq!(v2.y, 4.0);

        let array: [f32; 2] = v.into();
        assert_eq!(array, [1.0, 2.0]);
    }

    #[test]
    fn test_rect_creation() {
        let rect = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 20.0));
        assert_eq!(rect.width(), 10.0);
        assert_eq!(rect.height(), 20.0);

        let center = rect.center();
        assert_eq!(center.x, 5.0);
        assert_eq!(center.y, 10.0);

        let rect2 = Rect::from_center_size(Vec2::new(5.0, 10.0), Vec2::new(10.0, 20.0));
        assert_eq!(rect2.min.x, 0.0);
        assert_eq!(rect2.min.y, 0.0);
        assert_eq!(rect2.max.x, 10.0);
        assert_eq!(rect2.max.y, 20.0);
    }

    #[test]
    fn test_interaction_query() {
        let point_query = GpuInteractionQuery::point(Vec2::new(100.0, 200.0), 500);
        assert_eq!(point_query.query_type, 0);
        assert_eq!(point_query.position, [100.0, 200.0]);
        assert_eq!(point_query.max_results, 500);

        let rect = Rect::new(Vec2::new(50.0, 60.0), Vec2::new(150.0, 180.0));
        let region_query = GpuInteractionQuery::region(rect, 1000);
        assert_eq!(region_query.query_type, 1);
        assert_eq!(region_query.position, [100.0, 120.0]); // center
        assert_eq!(region_query.region_size, [100.0, 120.0]); // width, height
        assert_eq!(region_query.max_results, 1000);
    }

    #[test]
    fn test_element_hit() {
        let hit = ElementHit::new(42, 1, 15.5, Vec2::new(100.0, 200.0));
        assert_eq!(hit.element_id, 42);
        assert_eq!(hit.selection_id, 1);
        assert_eq!(hit.distance, 15.5);
        assert_eq!(hit.intersection_point.x, 100.0);
        assert_eq!(hit.intersection_point.y, 200.0);

        let hit_with_meta = hit.with_metadata("type", "circle");
        assert_eq!(
            hit_with_meta.metadata.get("type"),
            Some(&"circle".to_string())
        );
    }

    #[test]
    fn test_interaction_event() {
        let event = InteractionEvent::new("click", Vec2::new(50.0, 75.0));
        assert_eq!(event.interaction_type, "click");
        assert_eq!(event.screen_position.x, 50.0);
        assert_eq!(event.screen_position.y, 75.0);
        assert!(event.world_position.is_none());
        assert!(event.hit.is_none());

        let hit = ElementHit::new(1, 0, 10.0, Vec2::new(50.0, 75.0));
        let event_with_hit = event.with_hit(hit.clone());
        assert!(event_with_hit.hit.is_some());
        assert_eq!(event_with_hit.hit.as_ref().unwrap().element_id, 1);
    }

    #[test]
    fn test_query_stats() {
        let mut stats = QueryStats::default();
        assert_eq!(stats.total_queries, 0);
        assert_eq!(stats.hit_rate(), 0.0);

        stats.update(1000, 50, 500.0);
        assert_eq!(stats.total_queries, 1);
        assert_eq!(stats.total_elements_tested, 1000);
        assert_eq!(stats.total_hits, 50);
        assert_eq!(stats.average_query_time_us, 500.0);
        assert_eq!(stats.max_query_time_us, 500.0);
        assert_eq!(stats.hit_rate(), 0.05);

        stats.update(2000, 100, 300.0);
        assert_eq!(stats.total_queries, 2);
        assert_eq!(stats.total_elements_tested, 3000);
        assert_eq!(stats.total_hits, 150);
        assert_eq!(stats.average_query_time_us, 400.0); // (500 + 300) / 2
        assert_eq!(stats.max_query_time_us, 500.0);
        assert_eq!(stats.hit_rate(), 0.05); // 150 / 3000
    }

    #[test]
    fn test_bytemuck_compatibility() {
        // Test that our GPU structs are properly aligned and can be cast to bytes
        let query = GpuInteractionQuery::point(Vec2::new(10.0, 20.0), 100);
        let bytes = bytemuck::bytes_of(&query);
        assert_eq!(bytes.len(), std::mem::size_of::<GpuInteractionQuery>());

        let element = ElementData {
            position: [1.0, 2.0],
            size: [10.0, 10.0],
            mark_type: 0,
            element_id: 42,
            selection_id: 1,
            _padding: 0,
        };
        let bytes = bytemuck::bytes_of(&element);
        assert_eq!(bytes.len(), std::mem::size_of::<ElementData>());

        let result = InteractionResult {
            element_id: 42,
            selection_id: 1,
            distance: 15.5,
            is_hit: 1,
            intersection_point: [10.0, 20.0],
            _padding: [0, 0],
        };
        let bytes = bytemuck::bytes_of(&result);
        assert_eq!(bytes.len(), std::mem::size_of::<InteractionResult>());

        // Debug: Print struct sizes and layouts
        println!(
            "GpuInteractionQuery size: {}",
            std::mem::size_of::<GpuInteractionQuery>()
        );
        println!("ElementData size: {}", std::mem::size_of::<ElementData>());
        println!(
            "InteractionResult size: {}",
            std::mem::size_of::<InteractionResult>()
        );

        // Debug: Print struct field offsets
        use std::mem::offset_of;
        println!("GpuInteractionQuery field offsets:");
        println!(
            "  query_type: {}",
            offset_of!(GpuInteractionQuery, query_type)
        );
        println!(
            "  max_results: {}",
            offset_of!(GpuInteractionQuery, max_results)
        );
        println!("  position: {}", offset_of!(GpuInteractionQuery, position));
        println!(
            "  region_size: {}",
            offset_of!(GpuInteractionQuery, region_size)
        );
        println!("  _padding: {}", offset_of!(GpuInteractionQuery, _padding));

        println!("InteractionResult field offsets:");
        println!(
            "  element_id: {}",
            offset_of!(InteractionResult, element_id)
        );
        println!(
            "  selection_id: {}",
            offset_of!(InteractionResult, selection_id)
        );
        println!("  distance: {}", offset_of!(InteractionResult, distance));
        println!("  is_hit: {}", offset_of!(InteractionResult, is_hit));
        println!(
            "  intersection_point: {}",
            offset_of!(InteractionResult, intersection_point)
        );
        println!("  _padding: {}", offset_of!(InteractionResult, _padding));
    }

    #[test]
    fn test_spatial_index_config_alignment() {
        // Verify SpatialIndexConfig alignment matches WGSL SpatialIndex struct
        use std::mem::offset_of;
        assert_eq!(
            std::mem::size_of::<SpatialIndexConfig>(),
            32,
            "SpatialIndexConfig must be 32 bytes to match WGSL"
        );
        assert_eq!(offset_of!(SpatialIndexConfig, grid_size), 0);
        assert_eq!(offset_of!(SpatialIndexConfig, cell_size), 8);
        assert_eq!(offset_of!(SpatialIndexConfig, world_bounds_min), 16);
        assert_eq!(offset_of!(SpatialIndexConfig, world_bounds_max), 24);
    }

    #[test]
    fn test_spatial_cell_alignment() {
        // Verify SpatialCell alignment matches WGSL SpatialCell struct
        use std::mem::offset_of;
        assert_eq!(
            std::mem::size_of::<SpatialCell>(),
            24,
            "SpatialCell must be 24 bytes to match WGSL"
        );
        assert_eq!(offset_of!(SpatialCell, element_count), 0);
        assert_eq!(offset_of!(SpatialCell, element_start_index), 4);
        assert_eq!(offset_of!(SpatialCell, bounds_min), 8);
        assert_eq!(offset_of!(SpatialCell, bounds_max), 16);
    }

    #[test]
    fn test_spatial_index_world_to_cell() {
        // Test the cell index computation directly using a helper
        let config = SpatialIndexConfig {
            grid_size: [10, 10],
            cell_size: [10.0, 10.0],
            world_bounds_min: [0.0, 0.0],
            world_bounds_max: [100.0, 100.0],
        };

        // Helper mimicking InteractionSystem::world_to_cell_index
        let world_to_cell = |pos: [f32; 2]| -> usize {
            let grid_w = config.grid_size[0] as usize;
            let min = config.world_bounds_min;
            let max = config.world_bounds_max;
            let range_x = max[0] - min[0];
            let range_y = max[1] - min[1];
            let nx = ((pos[0] - min[0]) / range_x).clamp(0.0, 1.0 - f32::EPSILON);
            let ny = ((pos[1] - min[1]) / range_y).clamp(0.0, 1.0 - f32::EPSILON);
            let cx = (nx * grid_w as f32) as usize;
            let cy = (ny * grid_w as f32) as usize;
            cy * grid_w + cx
        };

        // Origin → cell (0,0) = index 0
        assert_eq!(world_to_cell([0.0, 0.0]), 0);
        // Position in cell (5,5) → index 55
        assert_eq!(world_to_cell([55.0, 55.0]), 55);
        // Edge of grid → last cell (9,9) = index 99
        assert_eq!(world_to_cell([99.0, 99.0]), 99);
        // Out of bounds clamped to valid range
        assert_eq!(world_to_cell([-10.0, -10.0]), 0);
    }

    #[test]
    fn test_spatial_index_cpu_build() {
        // Test the CPU spatial index building logic
        let elements = vec![
            ElementData {
                position: [10.0, 10.0],
                size: [5.0, 5.0],
                mark_type: 0,
                element_id: 0,
                selection_id: 0,
                _padding: 0,
            },
            ElementData {
                position: [10.0, 10.0],
                size: [5.0, 5.0],
                mark_type: 0,
                element_id: 1,
                selection_id: 0,
                _padding: 0,
            },
            ElementData {
                position: [90.0, 90.0],
                size: [5.0, 5.0],
                mark_type: 0,
                element_id: 2,
                selection_id: 0,
                _padding: 0,
            },
        ];

        // Simple CPU-side index build matching InteractionSystem::build_spatial_index
        let grid_w = 10usize;
        let grid_h = 10usize;
        let total_cells = grid_w * grid_h;
        let min_x = 0.0f32;
        let min_y = 0.0f32;
        let max_x = 100.0f32;
        let max_y = 100.0f32;
        let range_x = max_x - min_x;
        let range_y = max_y - min_y;

        // Phase 1: count
        let mut counts = vec![0u32; total_cells];
        for e in &elements {
            let nx = ((e.position[0] - min_x) / range_x).clamp(0.0, 1.0 - f32::EPSILON);
            let ny = ((e.position[1] - min_y) / range_y).clamp(0.0, 1.0 - f32::EPSILON);
            let cx = (nx * grid_w as f32) as usize;
            let cy = (ny * grid_h as f32) as usize;
            let idx = cy * grid_w + cx;
            counts[idx] += 1;
        }

        // Elements 0 and 1 are at (10,10) → cell (1,1) = index 11
        assert_eq!(counts[11], 2);
        // Element 2 is at (90,90) → cell (9,9) = index 99
        assert_eq!(counts[99], 1);
        // Other cells should be empty
        assert_eq!(counts.iter().sum::<u32>(), 3);

        // Phase 2: prefix sum
        let mut offsets = vec![0u32; total_cells];
        let mut running = 0u32;
        for i in 0..total_cells {
            offsets[i] = running;
            running += counts[i];
        }
        assert_eq!(offsets[11], 0); // first non-empty cell starts at 0
        assert_eq!(offsets[99], 2); // after two elements

        // Phase 3: populate
        let total_indexed = running as usize;
        assert_eq!(total_indexed, 3);
        let mut indices = vec![0u32; total_indexed];
        let mut cursors = offsets.clone();
        for (i, e) in elements.iter().enumerate() {
            let nx = ((e.position[0] - min_x) / range_x).clamp(0.0, 1.0 - f32::EPSILON);
            let ny = ((e.position[1] - min_y) / range_y).clamp(0.0, 1.0 - f32::EPSILON);
            let cx = (nx * grid_w as f32) as usize;
            let cy = (ny * grid_h as f32) as usize;
            let cell_idx = cy * grid_w + cx;
            let pos = cursors[cell_idx] as usize;
            indices[pos] = i as u32;
            cursors[cell_idx] += 1;
        }
        // Element indices should be stored in order within each cell
        assert_eq!(indices[0], 0); // first element in cell 11
        assert_eq!(indices[1], 1); // second element in cell 11
        assert_eq!(indices[2], 2); // element in cell 99
    }

    #[tokio::test]
    async fn test_interaction_system_staging_pool_creation() {
        let context = crate::render::RenderContext::new().await.unwrap();
        let system = InteractionSystem::new(&context).await.unwrap();

        // Staging pool should be initialized with zero active buffers
        let stats = system.staging_pool_stats();
        assert_eq!(stats.active_buffers, 0);
        assert_eq!(stats.pooled_buffers, 0);
        assert_eq!(stats.total_allocated, 0);
    }

    #[tokio::test]
    async fn test_interaction_system_staging_pool_cleanup() {
        let context = crate::render::RenderContext::new().await.unwrap();
        let mut system = InteractionSystem::new(&context).await.unwrap();

        // Cleanup should be safe to call even with empty pool
        system.cleanup_staging_pool();

        let stats = system.staging_pool_stats();
        assert_eq!(stats.pooled_buffers, 0);
    }

    // --- Adaptive grid size tests (GUP-176) ---

    #[test]
    fn test_adaptive_grid_side_tiny_dataset() {
        // Datasets smaller than MIN_GRID_SIDE² should still get MIN_GRID_SIDE.
        assert_eq!(InteractionSystem::adaptive_grid_side(1, 10_000), 4);
        assert_eq!(InteractionSystem::adaptive_grid_side(9, 10_000), 4);
        assert_eq!(InteractionSystem::adaptive_grid_side(15, 10_000), 4);
    }

    #[test]
    fn test_adaptive_grid_side_uses_sqrt() {
        // √100 = 10
        assert_eq!(InteractionSystem::adaptive_grid_side(100, 10_000), 10);
        // √10000 = 100
        assert_eq!(InteractionSystem::adaptive_grid_side(10_000, 10_000), 100);
        // √2500 = 50
        assert_eq!(InteractionSystem::adaptive_grid_side(2_500, 10_000), 50);
    }

    #[test]
    fn test_adaptive_grid_side_caps_at_max() {
        // √1_000_000 = 1000, but max_cells = 10_000 → max_side = 100
        assert_eq!(
            InteractionSystem::adaptive_grid_side(1_000_000, 10_000),
            100
        );
    }

    #[test]
    fn test_adaptive_grid_side_non_perfect_sqrt() {
        // √50 ≈ 7.07, ceil → 8
        assert_eq!(InteractionSystem::adaptive_grid_side(50, 10_000), 8);
        // √200 ≈ 14.14, ceil → 15
        assert_eq!(InteractionSystem::adaptive_grid_side(200, 10_000), 15);
    }

    #[test]
    fn test_adaptive_grid_side_zero_elements() {
        // build_spatial_index returns early for 0 elements, but the
        // function itself should still return MIN_GRID_SIDE.
        assert_eq!(InteractionSystem::adaptive_grid_side(0, 10_000), 4);
    }

    #[test]
    fn test_adaptive_grid_side_small_max_cells() {
        // max_cells = 16 → max_side = 4
        assert_eq!(InteractionSystem::adaptive_grid_side(10_000, 16), 4);
    }
}

/// Multi-touch gesture recognizer that processes touch events to detect gestures.
///
/// This recognizer tracks active touches and applies heuristics to identify
/// common gestures like pinch, rotate, swipe, and pan.
#[allow(clippy::items_after_test_module)]
pub struct GestureRecognizer {
    /// Active touches being tracked
    active_touches: HashMap<u64, TouchPoint>,
    /// Previous touch positions for delta calculations
    previous_touches: HashMap<u64, TouchPoint>,
    /// Minimum distance for swipe recognition (pixels)
    #[allow(dead_code)]
    swipe_threshold: f32,
    /// Minimum velocity for swipe recognition (pixels/second)
    #[allow(dead_code)]
    swipe_velocity_threshold: f32,
}

impl GestureRecognizer {
    /// Create a new gesture recognizer with default thresholds.
    pub fn new() -> Self {
        Self {
            active_touches: HashMap::new(),
            previous_touches: HashMap::new(),
            swipe_threshold: 50.0,           // 50 pixels minimum
            swipe_velocity_threshold: 500.0, // 500 px/s minimum
        }
    }

    /// Update with new touch points and recognize gestures.
    ///
    /// Returns the recognized gesture, if any.
    pub fn update(&mut self, touches: Vec<TouchPoint>) -> Option<GestureType> {
        // Store previous state
        self.previous_touches = self.active_touches.clone();

        // Update active touches
        self.active_touches.clear();
        for touch in touches {
            self.active_touches.insert(touch.id, touch);
        }

        // Recognize gestures based on touch count
        match self.active_touches.len() {
            0 => {
                // Check for completed swipe
                if self.previous_touches.len() == 1 {
                    self.recognize_swipe()
                } else {
                    None
                }
            }
            1 => {
                // Single touch - could be start of swipe or pan
                self.recognize_pan()
            }
            2 => {
                // Two touches - could be pinch or rotate
                self.recognize_two_finger_gesture()
            }
            _ => {
                // Three or more touches - generic multi-touch pan
                self.recognize_pan()
            }
        }
    }

    /// Recognize swipe gesture from touch release.
    fn recognize_swipe(&self) -> Option<GestureType> {
        if let Some((_, _prev_touch)) = self.previous_touches.iter().next() {
            // For swipe recognition, we'd need start position stored separately
            // This is a simplified version
            None // Swipe detection requires more state tracking
        } else {
            None
        }
    }

    /// Recognize pan gesture from single or multi-touch movement.
    fn recognize_pan(&self) -> Option<GestureType> {
        if self.active_touches.is_empty() || self.previous_touches.is_empty() {
            return None;
        }

        // Calculate average position and delta
        let (current_pos, prev_pos) = self.calculate_average_positions();

        let delta = current_pos - prev_pos;

        // Only return pan if there's meaningful movement
        let delta_magnitude = (delta.x * delta.x + delta.y * delta.y).sqrt();
        if delta_magnitude > 1.0 {
            Some(GestureType::Pan {
                start: prev_pos,
                current: current_pos,
                delta,
            })
        } else {
            None
        }
    }

    /// Recognize two-finger gestures (pinch and rotate).
    fn recognize_two_finger_gesture(&self) -> Option<GestureType> {
        if self.active_touches.len() != 2 || self.previous_touches.len() != 2 {
            return None;
        }

        let touches: Vec<&TouchPoint> = self.active_touches.values().collect();
        let prev_touches: Vec<&TouchPoint> = self.previous_touches.values().collect();

        // Calculate distances and angles
        let current_distance = self.distance_between(touches[0].position, touches[1].position);
        let previous_distance =
            self.distance_between(prev_touches[0].position, prev_touches[1].position);

        let current_angle = self.angle_between(touches[0].position, touches[1].position);
        let previous_angle = self.angle_between(prev_touches[0].position, prev_touches[1].position);

        let center = (touches[0].position + touches[1].position) * 0.5;

        // Calculate deltas
        let scale = if previous_distance > 0.0 {
            current_distance / previous_distance
        } else {
            1.0
        };
        let delta_scale = scale - 1.0;

        let delta_angle = current_angle - previous_angle;

        // Determine dominant gesture
        // Prefer pinch if scale change is significant
        if delta_scale.abs() > 0.01 {
            Some(GestureType::Pinch {
                center,
                scale,
                delta_scale,
            })
        } else if delta_angle.abs() > 0.02 {
            // 0.02 radians ≈ 1 degree
            Some(GestureType::Rotate {
                center,
                angle: current_angle,
                delta_angle,
            })
        } else {
            None
        }
    }

    /// Calculate distance between two points.
    fn distance_between(&self, p1: Vec2, p2: Vec2) -> f32 {
        let d = p2 - p1;
        (d.x * d.x + d.y * d.y).sqrt()
    }

    /// Calculate angle between two points (in radians).
    fn angle_between(&self, p1: Vec2, p2: Vec2) -> f32 {
        let d = p2 - p1;
        d.y.atan2(d.x)
    }

    /// Calculate average position of current and previous touches.
    fn calculate_average_positions(&self) -> (Vec2, Vec2) {
        let current_avg = if !self.active_touches.is_empty() {
            let sum = self
                .active_touches
                .values()
                .fold(Vec2::zero(), |acc, t| acc + t.position);
            sum / self.active_touches.len() as f32
        } else {
            Vec2::zero()
        };

        let prev_avg = if !self.previous_touches.is_empty() {
            let sum = self
                .previous_touches
                .values()
                .fold(Vec2::zero(), |acc, t| acc + t.position);
            sum / self.previous_touches.len() as f32
        } else {
            Vec2::zero()
        };

        (current_avg, prev_avg)
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}
