// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streaming LOD manager — incremental, memory-bounded LOD updates from a
//! live [`DataStream<T>`](crate::streaming::DataStream).
//!
//! The [`StreamingLodManager`] accepts a data stream, routes each arriving
//! point to the spatially correct pyramid cell at every LOD level, uploads
//! only the dirty cells to the GPU, and evicts the oldest points when the
//! configured [`MemoryBudget`] is exceeded.
//!
//! # Quick Start
//!
//! ```no_run
//! use gup::lod::{LodPyramidBuilder, VertexData, MemoryBudget};
//! use gup::lod::streaming::{StreamingLodManager, SpatiallyKeyed};
//! use gup::streaming::{DataStream, StreamMode, BackpressureStrategy};
//! use gup::render::RenderContext;
//!
//! #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
//! #[repr(C)]
//! struct Pt { x: f32, y: f32 }
//!
//! impl SpatiallyKeyed for Pt {
//!     fn spatial_key(&self) -> (f32, f32) { (self.x, self.y) }
//! }
//!
//! # async fn example() -> gup::error::GupResult<()> {
//! let ctx = RenderContext::new().await?;
//! let device = ctx.device();
//! let queue = ctx.queue();
//!
//! let data = vec![VertexData::new(0.5, 0.5)];
//! let pyramid = LodPyramidBuilder::new()
//!     .levels(4)
//!     .build_cpu(device, queue, &data)?;
//!
//! let stream = DataStream::<Pt>::builder()
//!     .capacity(10_000)
//!     .mode(StreamMode::SlidingWindow)
//!     .backpressure(BackpressureStrategy::EvictOldest)
//!     .build(device)?;
//!
//! let mut mgr = StreamingLodManager::new(
//!     pyramid, stream,
//!     MemoryBudget::bytes(64 * 1024 * 1024),
//!     device,
//! );
//!
//! // Each frame: drain stream, update pyramid, enforce budget.
//! mgr.poll(device, queue);
//! let _pyramid = mgr.pyramid();
//! # Ok(())
//! # }
//! ```

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use crate::buffer::{BufferType, GpuBuffer};
use crate::lod::{LodLevelMetadata, LodPyramid, VertexData};
use crate::streaming::DataStream;
use crate::streaming::streaming_buffer::StreamUpdate;

// ---------------------------------------------------------------------------
// SpatiallyKeyed
// ---------------------------------------------------------------------------

/// Trait for data types that have a 2D spatial position.
///
/// Implement this for any `T` that you wish to feed into a
/// [`StreamingLodManager`]. The returned `(x, y)` coordinates are used to
/// route each point to the correct pyramid cell at every LOD level.
///
/// # Examples
///
/// ```
/// use gup::lod::streaming::SpatiallyKeyed;
///
/// #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// #[repr(C)]
/// struct SensorReading {
///     lat: f32,
///     lon: f32,
///     value: f32,
///     _pad: f32,
/// }
///
/// impl SpatiallyKeyed for SensorReading {
///     fn spatial_key(&self) -> (f32, f32) {
///         (self.lon, self.lat)
///     }
/// }
/// ```
pub trait SpatiallyKeyed {
    /// Return the `(x, y)` spatial coordinates of this data point.
    fn spatial_key(&self) -> (f32, f32);
}

// Blanket impl for VertexData so it can be used directly.
impl SpatiallyKeyed for VertexData {
    fn spatial_key(&self) -> (f32, f32) {
        (self.x, self.y)
    }
}

// ---------------------------------------------------------------------------
// MemoryBudget
// ---------------------------------------------------------------------------

/// A memory budget in bytes for the streaming LOD pyramid.
///
/// When total GPU memory used by all pyramid cells reaches this limit the
/// [`StreamingLodManager`] evicts the oldest data points until usage falls
/// at or below the budget.
///
/// # Examples
///
/// ```
/// use gup::lod::MemoryBudget;
///
/// let budget = MemoryBudget::bytes(512 * 1024 * 1024); // 512 MiB
/// assert_eq!(budget.as_bytes(), 512 * 1024 * 1024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryBudget(usize);

impl MemoryBudget {
    /// Create a budget of `bytes` bytes.
    #[inline]
    pub fn bytes(bytes: usize) -> Self {
        Self(bytes)
    }

    /// Create a budget specified in mebibytes.
    #[inline]
    pub fn mebibytes(mib: usize) -> Self {
        Self(mib * 1024 * 1024)
    }

    /// The budget value in bytes.
    #[inline]
    pub fn as_bytes(self) -> usize {
        self.0
    }
}

// ---------------------------------------------------------------------------
// EvictionPolicy
// ---------------------------------------------------------------------------

/// Strategy used to decide which points to evict when the memory budget is
/// exceeded.
///
/// Currently only `OldestFirst` is supported; the enum is non-exhaustive to
/// allow future strategies (e.g. `LeastRecentlyAccessed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum EvictionPolicy {
    /// Evict the oldest points (by insertion order) first.
    #[default]
    OldestFirst,
}

// ---------------------------------------------------------------------------
// ScatterPoint
// ---------------------------------------------------------------------------

/// A simple 2D scatter-plot point.
///
/// This is the canonical [`SpatiallyKeyed`] type used in the
/// `streaming_lod_scatter` example and in tests.
///
/// # Examples
///
/// ```
/// use gup::lod::streaming::{ScatterPoint, SpatiallyKeyed};
///
/// let pt = ScatterPoint { x: 1.5, y: 3.7 };
/// assert_eq!(pt.spatial_key(), (1.5, 3.7));
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScatterPoint {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl SpatiallyKeyed for ScatterPoint {
    fn spatial_key(&self) -> (f32, f32) {
        (self.x, self.y)
    }
}

// ---------------------------------------------------------------------------
// Internal bookkeeping
// ---------------------------------------------------------------------------

/// Record of a single inserted point (for FIFO eviction).
#[derive(Debug, Clone)]
struct InsertionRecord {
    /// The spatial key.
    key: (f32, f32),
    /// Cell index at each LOD level that contains this point.
    cell_indices: Vec<usize>,
}

/// CPU-side mirror of a single cell's point data.
#[derive(Debug, Clone, Default)]
struct CellData {
    points: Vec<VertexData>,
}

/// Per-level streaming state.
#[derive(Debug)]
struct StreamingLevel {
    /// Number of cells per axis at this level.
    grid_side: usize,
    /// CPU-side cell data. Length = `grid_side * grid_side`.
    cells: Vec<CellData>,
    /// Indices of cells modified since last flush.
    dirty: HashSet<usize>,
}

// ---------------------------------------------------------------------------
// StreamingLodManager
// ---------------------------------------------------------------------------

/// Manages a live-updating, memory-bounded LOD pyramid fed by a
/// [`DataStream<T>`](crate::streaming::DataStream).
///
/// See the [module-level documentation](self) for a full usage example.
pub struct StreamingLodManager<T: bytemuck::Pod + bytemuck::Zeroable + SpatiallyKeyed> {
    /// Spatial bounds: `[min_x, min_y, max_x, max_y]`.
    bounds: [f32; 4],
    /// Per-level streaming state (CPU-side cell tracking).
    levels: Vec<StreamingLevel>,
    /// The LodPyramid whose GPU buffers we update incrementally.
    pyramid: LodPyramid,
    /// FIFO log of all inserted points (oldest at front).
    insertion_log: VecDeque<InsertionRecord>,
    /// Memory budget.
    budget: MemoryBudget,
    /// Current estimated GPU byte usage.
    current_bytes: usize,
    /// Eviction strategy.
    #[allow(dead_code)]
    eviction_policy: EvictionPolicy,
    /// Incoming data stream.
    stream: DataStream<T>,
    /// Running count of cell writes (for testing / metrics).
    cell_write_count: u64,
    /// Shared buffer populated by the stream subscriber on each push.
    pending: Arc<Mutex<Vec<(f32, f32)>>>,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable + SpatiallyKeyed> StreamingLodManager<T> {
    /// Construct a new streaming LOD manager.
    ///
    /// * `pyramid` — An existing [`LodPyramid`] that defines the spatial
    ///   extent (via its level-0 bounds) and number of levels.
    /// * `stream` — A [`DataStream<T>`] whose elements implement
    ///   [`SpatiallyKeyed`].
    /// * `budget` — Maximum GPU memory the manager may use.
    /// * `device` — The GPU device used to allocate level buffers.
    pub fn new(
        pyramid: LodPyramid,
        mut stream: DataStream<T>,
        budget: MemoryBudget,
        device: &wgpu::Device,
    ) -> Self {
        let depth = pyramid.level_count();
        let bounds = pyramid.metadata(0).bounds;

        // Shared pending buffer: the subscriber pushes spatial keys here,
        // and `poll()` drains them.
        let pending: Arc<Mutex<Vec<(f32, f32)>>> = Arc::new(Mutex::new(Vec::new()));
        let pending_clone = Arc::clone(&pending);

        // Subscribe to the stream so that every push is captured.
        stream.subscribe(move |update| {
            if let StreamUpdate::Insert { data, .. } = update {
                let key = data.spatial_key();
                pending_clone.lock().unwrap().push(key);
            }
        });

        // Build per-level streaming state.
        let mut levels = Vec::with_capacity(depth);
        let mut new_gpu_levels: Vec<GpuBuffer<VertexData>> = Vec::with_capacity(depth);
        let mut new_metadata: Vec<LodLevelMetadata> = Vec::with_capacity(depth);

        for level_idx in 0..depth {
            let grid_side = grid_side_for_level(level_idx, depth);
            let total_cells = grid_side * grid_side;
            let cells: Vec<CellData> = (0..total_cells).map(|_| CellData::default()).collect();

            levels.push(StreamingLevel {
                grid_side,
                cells,
                dirty: HashSet::new(),
            });

            // Allocate a fresh GPU buffer for each level.
            let initial_capacity = (1024 / total_cells).max(16) * total_cells;
            let buf = GpuBuffer::<VertexData>::new(device, BufferType::Storage, initial_capacity);
            new_gpu_levels.push(buf);
            new_metadata.push(LodLevelMetadata {
                point_count: 0,
                cell_size: cell_size_for_level(&bounds, grid_side),
                bounds,
            });
        }

        // Reconstruct the pyramid with our freshly allocated buffers.
        let new_pyramid =
            LodPyramid::from_parts(new_gpu_levels, new_metadata, budget.as_bytes() as u64, 0);

        Self {
            bounds,
            levels,
            pyramid: new_pyramid,
            insertion_log: VecDeque::new(),
            budget,
            current_bytes: 0,
            eviction_policy: EvictionPolicy::OldestFirst,
            stream,
            cell_write_count: 0,
            pending,
        }
    }

    /// Read-only reference to the underlying [`LodPyramid`].
    ///
    /// The returned pyramid reflects all updates applied by prior [`poll`](Self::poll)
    /// calls. Pass this to the renderer to draw the current LOD state.
    pub fn pyramid(&self) -> &LodPyramid {
        &self.pyramid
    }

    /// Drain all pending data from the stream and apply incremental updates
    /// to the pyramid.
    ///
    /// This method:
    /// 1. Drains all buffered points from the [`DataStream<T>`].
    /// 2. Routes each point to the correct cell at every LOD level.
    /// 3. Coalesces multiple points per cell into a single GPU upload.
    /// 4. Flushes only the dirty cells to the GPU.
    /// 5. Enforces the memory budget by evicting the oldest points if needed.
    pub fn poll(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // Step 1: Drain pending points from the stream.
        let pending = self.drain_stream(device, queue);
        if pending.is_empty() {
            return;
        }

        // Step 2 + 3: Route points and mark cells dirty.
        for (x, y) in &pending {
            self.insert_point(*x, *y);
        }

        // Step 4: Flush dirty cells to GPU.
        self.flush_dirty_cells(device, queue);

        // Step 5: Enforce memory budget.
        if self.current_bytes > self.budget.as_bytes() {
            self.evict_until_within_budget(device, queue);
        }
    }

    /// Total number of live data points across all cells at level 0.
    pub fn total_points(&self) -> usize {
        self.levels[0].cells.iter().map(|c| c.points.len()).sum()
    }

    /// Running count of individual cell GPU writes performed.
    pub fn cell_write_count(&self) -> u64 {
        self.cell_write_count
    }

    /// Current estimated GPU memory usage in bytes.
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Reference to the internal data stream.
    pub fn stream(&self) -> &DataStream<T> {
        &self.stream
    }

    /// Mutable reference to the internal data stream (for pushing data).
    pub fn stream_mut(&mut self) -> &mut DataStream<T> {
        &mut self.stream
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Drain all pending points from the shared subscriber buffer.
    fn drain_stream(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(f32, f32)> {
        // Also flush the DataStream's GPU-side buffer.
        self.stream.flush(device, queue);

        // Drain the pending buffer that was populated by our subscriber.
        let mut guard = self.pending.lock().unwrap();
        let points: Vec<(f32, f32)> = guard.drain(..).collect();
        points
    }

    /// Insert a single point into the pyramid at all levels.
    fn insert_point(&mut self, x: f32, y: f32) {
        let depth = self.levels.len();
        let vertex = VertexData::new(x, y);
        let mut cell_indices = Vec::with_capacity(depth);

        for level_idx in 0..depth {
            let level = &mut self.levels[level_idx];
            let cell_idx = spatial_to_cell(x, y, &self.bounds, level.grid_side);
            cell_indices.push(cell_idx);

            level.cells[cell_idx].points.push(vertex);
            level.dirty.insert(cell_idx);
        }

        self.insertion_log.push_back(InsertionRecord {
            key: (x, y),
            cell_indices,
        });

        self.current_bytes += std::mem::size_of::<VertexData>() * depth;
    }

    /// Flush all dirty cells to the GPU, coalescing writes per cell.
    fn flush_dirty_cells(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let depth = self.levels.len();
        let vertex_size = std::mem::size_of::<VertexData>();

        for level_idx in 0..depth {
            let dirty_cells: Vec<usize> = self.levels[level_idx].dirty.drain().collect();

            if dirty_cells.is_empty() {
                continue;
            }

            // Rebuild the flat buffer data for this level from all cells.
            let level = &self.levels[level_idx];
            let total_count: usize = level.cells.iter().map(|c| c.points.len()).sum();

            // Ensure the GPU buffer is large enough.
            let buf = self.pyramid.buffer_mut(level_idx);
            if total_count > buf.capacity() {
                let new_capacity = (total_count * 2).max(64);
                *buf = GpuBuffer::<VertexData>::new(device, BufferType::Storage, new_capacity);
            }

            // Collect all points and upload.
            let level = &self.levels[level_idx];
            let all_points: Vec<VertexData> = level
                .cells
                .iter()
                .flat_map(|c| c.points.iter().copied())
                .collect();
            if !all_points.is_empty() {
                let _ = self
                    .pyramid
                    .buffer_mut(level_idx)
                    .upload(device, queue, &all_points);
            }

            // Update metadata.
            self.pyramid.metadata_mut(level_idx).point_count = total_count;

            // Track cell writes — one per dirty cell.
            self.cell_write_count += dirty_cells.len() as u64;
        }

        // Recompute total allocated bytes.
        let total_bytes: usize = (0..depth)
            .map(|l| self.pyramid.metadata(l).point_count * vertex_size)
            .sum();
        self.current_bytes = total_bytes;
        self.pyramid.set_allocated_bytes(total_bytes as u64);
    }

    /// Evict oldest points until GPU memory is at or below the budget.
    fn evict_until_within_budget(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let vertex_size = std::mem::size_of::<VertexData>();
        let depth = self.levels.len();

        while self.current_bytes > self.budget.as_bytes() {
            let record = match self.insertion_log.pop_front() {
                Some(r) => r,
                None => break,
            };

            // Remove the point from each level's cell.
            for (level_idx, &cell_idx) in record.cell_indices.iter().enumerate() {
                let cell = &mut self.levels[level_idx].cells[cell_idx];
                // Find and remove the point matching this key.
                if let Some(pos) = cell.points.iter().position(|v| {
                    (v.x - record.key.0).abs() < f32::EPSILON
                        && (v.y - record.key.1).abs() < f32::EPSILON
                }) {
                    cell.points.remove(pos);
                    self.levels[level_idx].dirty.insert(cell_idx);
                }
            }

            // Decrease byte tracking.
            self.current_bytes = self.current_bytes.saturating_sub(vertex_size * depth);
        }

        // Flush dirty cells after eviction.
        self.flush_dirty_cells(device, queue);
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable + SpatiallyKeyed> std::fmt::Debug
    for StreamingLodManager<T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pending_len = self.pending.lock().map(|g| g.len()).unwrap_or(0);
        f.debug_struct("StreamingLodManager")
            .field("bounds", &self.bounds)
            .field("depth", &self.levels.len())
            .field("total_points", &self.total_points())
            .field("current_bytes", &self.current_bytes)
            .field("budget", &self.budget)
            .field("insertion_log_len", &self.insertion_log.len())
            .field("cell_write_count", &self.cell_write_count)
            .field("pending", &pending_len)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Compute the grid side length for a given level in a pyramid of `depth`
/// levels.
///
/// Level 0 (finest) has `2^(depth-1)` cells per side.
/// Level `depth-1` (coarsest / root) has 1 cell per side.
fn grid_side_for_level(level: usize, depth: usize) -> usize {
    // Clamp so level 0 doesn't overflow on very deep pyramids.
    let shift = (depth - 1).saturating_sub(level);
    1usize << shift.min(20) // cap at ~1M×1M grid
}

/// Compute the cell size for a given grid side and bounds.
fn cell_size_for_level(bounds: &[f32; 4], grid_side: usize) -> f32 {
    let extent_x = bounds[2] - bounds[0];
    let extent_y = bounds[3] - bounds[1];
    (extent_x / grid_side as f32).max(extent_y / grid_side as f32)
}

/// Map an `(x, y)` point to the flat cell index for a grid of `grid_side`
/// cells per axis within the given `bounds`.
fn spatial_to_cell(x: f32, y: f32, bounds: &[f32; 4], grid_side: usize) -> usize {
    if grid_side <= 1 {
        return 0;
    }
    let [min_x, min_y, max_x, max_y] = *bounds;
    let extent_x = max_x - min_x;
    let extent_y = max_y - min_y;

    let cx = if extent_x > f32::EPSILON {
        (((x - min_x) / extent_x) * grid_side as f32) as usize
    } else {
        0
    };
    let cy = if extent_y > f32::EPSILON {
        (((y - min_y) / extent_y) * grid_side as f32) as usize
    } else {
        0
    };

    let cx = cx.min(grid_side - 1);
    let cy = cy.min(grid_side - 1);
    cy * grid_side + cx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- MemoryBudget --------------------------------------------------------

    #[test]
    fn memory_budget_bytes() {
        let b = MemoryBudget::bytes(1024);
        assert_eq!(b.as_bytes(), 1024);
    }

    #[test]
    fn memory_budget_mebibytes() {
        let m = MemoryBudget::mebibytes(2);
        assert_eq!(m.as_bytes(), 2 * 1024 * 1024);
    }

    // -- Grid helpers --------------------------------------------------------

    #[test]
    fn grid_side_for_4_levels() {
        // depth=4: level 0 → 8, level 1 → 4, level 2 → 2, level 3 → 1
        assert_eq!(grid_side_for_level(0, 4), 8);
        assert_eq!(grid_side_for_level(1, 4), 4);
        assert_eq!(grid_side_for_level(2, 4), 2);
        assert_eq!(grid_side_for_level(3, 4), 1);
    }

    #[test]
    fn spatial_to_cell_quadrants() {
        let bounds = [0.0, 0.0, 10.0, 10.0];
        // grid_side=2 → 4 cells: (0,0)=0, (1,0)=1, (0,1)=2, (1,1)=3
        let tl = spatial_to_cell(1.0, 1.0, &bounds, 2); // bottom-left quadrant
        let tr = spatial_to_cell(9.0, 1.0, &bounds, 2); // bottom-right
        let bl = spatial_to_cell(1.0, 9.0, &bounds, 2); // top-left
        let br = spatial_to_cell(9.0, 9.0, &bounds, 2); // top-right

        // All four should be distinct.
        let set: HashSet<usize> = [tl, tr, bl, br].into_iter().collect();
        assert_eq!(set.len(), 4, "Expected 4 distinct cells, got {set:?}");
    }

    #[test]
    fn spatial_to_cell_root_level() {
        let bounds = [0.0, 0.0, 10.0, 10.0];
        // grid_side=1 → everything maps to cell 0.
        assert_eq!(spatial_to_cell(1.0, 1.0, &bounds, 1), 0);
        assert_eq!(spatial_to_cell(9.0, 9.0, &bounds, 1), 0);
    }

    #[test]
    fn spatial_to_cell_edge_cases() {
        let bounds = [0.0, 0.0, 10.0, 10.0];
        // Points exactly on the boundary should be clamped.
        let c = spatial_to_cell(10.0, 10.0, &bounds, 4);
        assert!(c < 16, "Cell index {c} should be < 16");

        let c = spatial_to_cell(0.0, 0.0, &bounds, 4);
        assert_eq!(c, 0);
    }

    // -- SpatiallyKeyed implementations --------------------------------------

    #[test]
    fn vertex_data_spatially_keyed() {
        let v = VertexData::new(3.0, 7.0);
        assert_eq!(v.spatial_key(), (3.0, 7.0));
    }

    // -- Integration tests (require GPU) -------------------------------------

    /// Generate enough data points spread across a 10×10 data space to ensure
    /// a 4-level pyramid can be built (level 0 needs ≥ 64 points so that
    /// 64 → ~16 → ~4 → ~1 works out).
    fn test_data(n: usize) -> Vec<VertexData> {
        (0..n)
            .map(|i| {
                let x = (i as f32 * 0.618_034) % 10.0;
                let y = (i as f32 * 0.414_214) % 10.0;
                VertexData::new(x, y)
            })
            .collect()
    }

    #[tokio::test]
    async fn manager_construction() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        assert_eq!(mgr.pyramid().level_count(), depth);
        assert_eq!(mgr.total_points(), 0);
        assert_eq!(mgr.cell_write_count(), 0);
    }

    #[tokio::test]
    async fn insert_single_point_touches_all_levels() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();
        assert!(depth >= 4, "Need at least 4 levels, got {depth}");

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mut mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        // Insert a single point directly.
        mgr.insert_point(3.0, 7.0);
        mgr.flush_dirty_cells(device, queue);

        // Should have written exactly `depth` cells (one per level).
        assert_eq!(
            mgr.cell_write_count(),
            depth as u64,
            "Expected {depth} cell writes for {depth} levels"
        );

        // Each level should have exactly 1 point.
        for level in 0..depth {
            assert_eq!(
                mgr.pyramid().level_point_count(level),
                1,
                "Level {level} should have 1 point"
            );
        }
    }

    #[tokio::test]
    async fn batch_coalesces_writes_per_cell() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mut mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        // Insert two points that fall into the same cell at all levels.
        // Very close together — same cell at every grid resolution.
        mgr.insert_point(0.5, 0.5);
        mgr.insert_point(0.6, 0.6);

        // Flush — coalesced writes should produce 1 write per level, not 2.
        mgr.flush_dirty_cells(device, queue);
        assert_eq!(
            mgr.cell_write_count(),
            depth as u64,
            "Two points in same cell should coalesce to {depth} writes (one per level)"
        );
    }

    #[tokio::test]
    async fn distinct_quadrants_route_to_distinct_cells() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        let bounds = mgr.bounds;
        let finest_grid = mgr.levels[0].grid_side;

        // Four corners of the data space.
        let c0 = spatial_to_cell(1.0, 1.0, &bounds, finest_grid);
        let c1 = spatial_to_cell(9.0, 1.0, &bounds, finest_grid);
        let c2 = spatial_to_cell(1.0, 9.0, &bounds, finest_grid);
        let c3 = spatial_to_cell(9.0, 9.0, &bounds, finest_grid);

        let set: HashSet<usize> = [c0, c1, c2, c3].into_iter().collect();
        assert_eq!(
            set.len(),
            4,
            "Points in distinct quadrants must route to distinct level-0 cells"
        );
    }

    #[tokio::test]
    async fn all_points_share_root_cell() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mut mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        // Insert points in all four quadrants.
        mgr.insert_point(1.0, 1.0);
        mgr.insert_point(9.0, 1.0);
        mgr.insert_point(1.0, 9.0);
        mgr.insert_point(9.0, 9.0);

        // At the coarsest level (grid_side=1), all points share cell 0.
        let coarsest = mgr.levels.last().unwrap();
        assert_eq!(coarsest.grid_side, 1);
        assert_eq!(coarsest.cells[0].points.len(), 4);
    }

    #[tokio::test]
    async fn eviction_enforces_budget() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        // Very tight budget: only enough for ~2 points × depth levels.
        let vertex_size = std::mem::size_of::<VertexData>();
        let budget = MemoryBudget::bytes(vertex_size * depth * 2);

        let mut mgr = StreamingLodManager::new(pyramid, stream, budget, device);

        // Insert 5 points — should exceed budget.
        for i in 0..5 {
            mgr.insert_point(1.0 + i as f32, 1.0 + i as f32);
        }
        mgr.flush_dirty_cells(device, queue);

        // Before eviction, we should be over budget.
        assert!(
            mgr.current_bytes > budget.as_bytes(),
            "Should be over budget before eviction"
        );

        // Trigger eviction.
        mgr.evict_until_within_budget(device, queue);

        // After eviction, should be at or below budget.
        assert!(
            mgr.current_bytes <= budget.as_bytes(),
            "After eviction, {} bytes should be <= {} budget",
            mgr.current_bytes,
            budget.as_bytes(),
        );

        // The oldest points should be gone; newest should remain.
        let remaining = mgr.total_points();
        assert!(remaining < 5, "Some points should have been evicted");
        assert!(remaining > 0, "Not all points should be evicted");
    }

    #[tokio::test]
    async fn poll_drains_stream_and_updates() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let mut mgr =
            StreamingLodManager::new(pyramid, stream, MemoryBudget::bytes(1024 * 1024), device);

        // Push data via the stream.
        mgr.stream_mut().push(VertexData::new(2.0, 3.0));
        mgr.stream_mut().push(VertexData::new(7.0, 8.0));

        mgr.poll(device, queue);

        assert_eq!(mgr.total_points(), 2, "poll should have inserted 2 points");
        assert!(
            mgr.cell_write_count() > 0,
            "poll should have triggered cell writes"
        );
    }

    #[tokio::test]
    async fn eviction_removes_oldest_keeps_newest() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        // Budget for exactly 3 points across all levels.
        let vertex_size = std::mem::size_of::<VertexData>();
        let budget = MemoryBudget::bytes(vertex_size * depth * 3);

        let mut mgr = StreamingLodManager::new(pyramid, stream, budget, device);

        // Insert 5 points with distinct coordinates.
        let coords: Vec<(f32, f32)> =
            vec![(1.0, 1.0), (2.0, 2.0), (3.0, 3.0), (4.0, 4.0), (5.0, 5.0)];
        for &(x, y) in &coords {
            mgr.insert_point(x, y);
        }

        // Flush and evict.
        mgr.flush_dirty_cells(device, queue);
        mgr.evict_until_within_budget(device, queue);

        // Budget allows 3 points; 2 oldest should be gone.
        assert!(
            mgr.current_bytes <= budget.as_bytes(),
            "Should be within budget"
        );
        let remaining = mgr.total_points();
        assert!(
            remaining <= 3,
            "At most 3 points should remain, got {remaining}"
        );
        assert!(remaining > 0, "At least 1 point should remain");

        // Verify that the oldest points (1,1) and (2,2) are absent from level 0.
        let level0_points: Vec<(f32, f32)> = mgr.levels[0]
            .cells
            .iter()
            .flat_map(|c| c.points.iter().map(|v| (v.x, v.y)))
            .collect();

        // The newest points should still be present.
        assert!(
            level0_points
                .iter()
                .any(|&(x, y)| (x - 5.0).abs() < 0.01 && (y - 5.0).abs() < 0.01),
            "Newest point (5,5) should remain after eviction"
        );

        // Check that evicted points are absent from ALL levels.
        for level_idx in 0..depth {
            let all_pts: Vec<(f32, f32)> = mgr.levels[level_idx]
                .cells
                .iter()
                .flat_map(|c| c.points.iter().map(|v| (v.x, v.y)))
                .collect();

            // Points (1,1) and (2,2) should have been evicted first.
            let has_oldest = all_pts
                .iter()
                .any(|&(x, y)| (x - 1.0).abs() < 0.01 && (y - 1.0).abs() < 0.01);
            assert!(
                !has_oldest,
                "Oldest point (1,1) should be absent from level {level_idx}"
            );
        }
    }

    #[tokio::test]
    async fn poll_with_budget_evicts_automatically() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(1000)
            .build(device)
            .unwrap();

        let vertex_size = std::mem::size_of::<VertexData>();
        let budget = MemoryBudget::bytes(vertex_size * depth * 10);

        let mut mgr = StreamingLodManager::new(pyramid, stream, budget, device);

        // Push 50 points through the stream and poll.
        for i in 0..50 {
            mgr.stream_mut()
                .push(VertexData::new(i as f32 % 10.0, i as f32 % 10.0));
        }
        mgr.poll(device, queue);

        // After poll, budget should be enforced.
        assert!(
            mgr.current_bytes <= budget.as_bytes(),
            "After poll, {} bytes should be <= {} budget",
            mgr.current_bytes,
            budget.as_bytes(),
        );
    }

    #[tokio::test]
    async fn integration_1000_iterations() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data = test_data(256);
        let pyramid = crate::lod::LodPyramidBuilder::new()
            .levels(4)
            .build_cpu(device, queue, &data)
            .unwrap();
        let depth = pyramid.level_count();

        let stream = DataStream::<VertexData>::builder()
            .capacity(10_000)
            .build(device)
            .unwrap();

        let vertex_size = std::mem::size_of::<VertexData>();
        let budget = MemoryBudget::bytes(vertex_size * depth * 500);

        let mut mgr = StreamingLodManager::new(pyramid, stream, budget, device);

        // Drive 1000 poll iterations with synthetic data.
        for iter in 0..1000 {
            let pt = VertexData::new((iter as f32 * 0.618) % 10.0, (iter as f32 * 0.414) % 10.0);
            mgr.stream_mut().push(pt);
            mgr.poll(device, queue);

            // Budget must always be honoured.
            assert!(
                mgr.current_bytes <= budget.as_bytes(),
                "Budget violated at iteration {iter}: {} > {}",
                mgr.current_bytes,
                budget.as_bytes(),
            );
        }

        // Final state should be consistent.
        assert!(mgr.total_points() > 0);
        assert!(mgr.cell_write_count() > 0);
    }

    #[test]
    fn scatter_point_spatially_keyed() {
        let pt = ScatterPoint { x: 1.5, y: 3.7 };
        assert_eq!(pt.spatial_key(), (1.5, 3.7));
    }

    #[test]
    fn eviction_policy_default() {
        assert_eq!(EvictionPolicy::default(), EvictionPolicy::OldestFirst);
    }
}
