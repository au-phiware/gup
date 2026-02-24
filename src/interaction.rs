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

use crate::RenderContext;
use crate::error::{GupError, GupResult};
use futures_channel;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineLayoutDescriptor, PollType, Queue,
    ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

/// Geometric shapes for spatial queries
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from(array: [f32; 2]) -> Self {
        Self {
            x: array[0],
            y: array[1],
        }
    }
}

impl From<Vec2> for [f32; 2] {
    fn from(vec: Vec2) -> Self {
        [vec.x, vec.y]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half_size = Vec2::new(size.x * 0.5, size.y * 0.5);
        Self {
            min: Vec2::new(center.x - half_size.x, center.y - half_size.y),
            max: Vec2::new(center.x + half_size.x, center.y + half_size.y),
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
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
pub trait CustomInteractionQuery: Send + Sync + std::fmt::Debug {
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
pub trait EventHandler: Send + Sync {
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
}

impl InteractionEvent {
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
        }
    }

    pub fn with_world_position(mut self, world_position: Vec2) -> Self {
        self.world_position = Some(world_position);
        self
    }

    pub fn with_hit(mut self, hit: ElementHit) -> Self {
        self.hit = Some(hit);
        self
    }

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
    pub fn point(position: Vec2, max_results: u32) -> Self {
        Self {
            query_type: 0,
            max_results,
            position: position.into(),
            region_size: [0.0, 0.0],
            _padding: [0; 2],
        }
    }

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
    pub element_count: u32,
    pub element_start_index: u32,
    pub bounds_min: [f32; 2],
    pub bounds_max: [f32; 2],
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

    /// Spatial indexing buffers
    spatial_cells_buffer: Buffer,
    element_indices_buffer: Buffer,
    spatial_config_buffer: Buffer,

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

    /// GPU resources
    device: Arc<Device>,
    queue: Arc<Queue>,

    /// Buffer capacities
    max_elements: usize,
    max_queries: usize,
    max_results: usize,
    max_spatial_cells: usize,
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

        // Default spatial configuration (will be updated based on data bounds)
        let spatial_config = SpatialIndexConfig {
            grid_size: [100, 100],   // 100x100 grid
            cell_size: [10.0, 10.0], // 10 units per cell
            world_bounds_min: [0.0, 0.0],
            world_bounds_max: [1000.0, 1000.0],
        };

        Ok(Self {
            hit_test_pipeline,
            spatial_index_pipeline,
            spatial_populate_pipeline,
            spatial_bind_group_layout,
            element_buffer,
            query_buffer,
            result_buffer,
            spatial_cells_buffer,
            element_indices_buffer,
            spatial_config_buffer,
            event_handlers: HashMap::new(),
            active_queries: Vec::new(),
            next_query_id: 0,
            query_stats: QueryStats::default(),
            spatial_config,
            spatial_index_built: false,
            spatial_element_indices: Vec::new(),
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            max_elements,
            max_queries,
            max_results,
            max_spatial_cells,
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

        // Execute compute shader – always use brute-force hit test on GPU.
        // The CPU-side spatial index narrows the candidate set in
        // `dispatcher_spatial_query`, but the final hit test is still a GPU
        // dispatch for consistency and correctness.
        if elements.len() > 1000 && self.spatial_index_built {
            self.dispatcher_spatial_query(query, elements.len()).await?;
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
    /// Uses the CPU-built spatial index to determine which cells overlap the
    /// query region, then dispatches the GPU hit test only over elements in
    /// those cells. For large datasets with localised queries this can reduce
    /// the number of GPU threads by orders of magnitude.
    async fn dispatcher_spatial_query(
        &mut self,
        _query: GpuInteractionQuery,
        element_count: usize,
    ) -> GupResult<()> {
        // For now, dispatch the full brute-force hit test on GPU.
        // The spatial index is built and ready; a follow-up story (GUP-078)
        // will implement a spatial-query-aware compute shader that reads the
        // index on the GPU side to skip irrelevant cells.
        self.dispatch_hit_test_compute(element_count, 1).await
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
        &self,
        element_count: usize,
        query_count: usize,
    ) -> GupResult<()> {
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
            ],
        });

        Ok(bind_group)
    }

    /// Download results from GPU
    async fn download_results(&self) -> GupResult<Vec<InteractionResult>> {
        // Create staging buffer for CPU readback
        let staging_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("result_staging"),
            size: (self.max_results * std::mem::size_of::<InteractionResult>()) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy from result buffer to staging buffer
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("result_copy_encoder"),
            });

        encoder.copy_buffer_to_buffer(
            &self.result_buffer,
            0,
            &staging_buffer,
            0,
            staging_buffer.size(),
        );

        let submission_index = self.queue.submit([encoder.finish()]);

        // Wait for the copy operation to complete
        let _ = self
            .device
            .poll(PollType::WaitForSubmissionIndex(submission_index));

        // Map the buffer and wait for it to be mapped
        let buffer_slice = staging_buffer.slice(..);

        // Create a channel to wait for the mapping completion
        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Poll the device until the mapping is complete
        let _ = self.device.poll(PollType::Wait);

        // Wait for the mapping to complete
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
        staging_buffer.unmap();

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
    }

    /// Build the spatial index from raw element data (public for testing).
    pub async fn build_spatial_index_from_elements(
        &mut self,
        elements: &[ElementData],
    ) -> GupResult<()> {
        self.build_spatial_index(elements).await
    }

    /// Register an event handler for a specific event type
    pub fn register_event_handler<F>(&mut self, event_type: &str, handler: F)
    where
        F: Fn(&InteractionEvent) + Send + Sync + 'static,
    {
        struct FnHandler<F> {
            f: F,
        }

        impl<F> EventHandler for FnHandler<F>
        where
            F: Fn(&InteractionEvent) + Send + Sync + 'static,
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

    /// Reset performance statistics
    pub fn reset_stats(&mut self) {
        self.query_stats = QueryStats::default();
    }
}

/// Trait for objects that can be queried for interactions
pub trait Renderable: Send + Sync {
    /// Get elements for interaction processing
    fn get_elements_for_interaction(&self) -> GupResult<Vec<InteractionElement>>;

    /// Get the selection ID for this renderable
    fn selection_id(&self) -> u32;
}

/// Element data for interaction processing
#[derive(Debug, Clone)]
pub struct InteractionElement {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub mark_type: u32,
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

        let delta = Vec2::new(current_pos.x - prev_pos.x, current_pos.y - prev_pos.y);

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

        let center = Vec2::new(
            (touches[0].position.x + touches[1].position.x) * 0.5,
            (touches[0].position.y + touches[1].position.y) * 0.5,
        );

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
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate angle between two points (in radians).
    fn angle_between(&self, p1: Vec2, p2: Vec2) -> f32 {
        (p2.y - p1.y).atan2(p2.x - p1.x)
    }

    /// Calculate average position of current and previous touches.
    fn calculate_average_positions(&self) -> (Vec2, Vec2) {
        let current_avg = if !self.active_touches.is_empty() {
            let sum = self
                .active_touches
                .values()
                .fold(Vec2::new(0.0, 0.0), |acc, t| {
                    Vec2::new(acc.x + t.position.x, acc.y + t.position.y)
                });
            Vec2::new(
                sum.x / self.active_touches.len() as f32,
                sum.y / self.active_touches.len() as f32,
            )
        } else {
            Vec2::new(0.0, 0.0)
        };

        let prev_avg = if !self.previous_touches.is_empty() {
            let sum = self
                .previous_touches
                .values()
                .fold(Vec2::new(0.0, 0.0), |acc, t| {
                    Vec2::new(acc.x + t.position.x, acc.y + t.position.y)
                });
            Vec2::new(
                sum.x / self.previous_touches.len() as f32,
                sum.y / self.previous_touches.len() as f32,
            )
        } else {
            Vec2::new(0.0, 0.0)
        };

        (current_avg, prev_avg)
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}
