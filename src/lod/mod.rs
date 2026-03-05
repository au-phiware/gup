// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Level-of-Detail (LOD) pyramid for billion-point rendering.
//!
//! This module provides a hierarchical LOD pyramid data structure that stores
//! progressively coarser representations of a point dataset. At a given zoom
//! level only the appropriate tier is rendered, keeping GPU throughput
//! proportional to the visible detail rather than to the raw data size.
//!
//! # Architecture
//!
//! The pyramid is a `Vec<GpuBuffer<VertexData>>` where:
//! - **Level 0** contains the full-resolution source data.
//! - **Level N** (coarsest) contains the fewest points.
//!
//! Each level beyond 0 is produced by grid-based spatial aggregation of the
//! previous level: the data space is divided into an N×N grid and one
//! representative point per occupied cell is emitted.
//!
//! # Usage
//!
//! ```no_run
//! use gup::lod::{LodPyramid, LodPyramidBuilder, VertexData, select_lod_level};
//! use gup::mark::batch_renderer::Viewport2D;
//!
//! // Build a pyramid from CPU data
//! # async fn example() -> gup::error::GupResult<()> {
//! # let device: wgpu::Device = todo!();
//! # let queue: wgpu::Queue = todo!();
//! let points: Vec<VertexData> = vec![/* ... */];
//! let pyramid = LodPyramidBuilder::new()
//!     .levels(5)
//!     .max_gpu_bytes(512 * 1024 * 1024) // 512 MB budget
//!     .build_cpu(&device, &queue, &points)?;
//!
//! // Select the right level for the current viewport
//! let viewport = Viewport2D::default();
//! let level = select_lod_level(&viewport, pyramid.level_point_count(0) as u64, pyramid.level_count());
//! let buffer = pyramid.buffer(level);
//! # Ok(())
//! # }
//! ```

mod selection;
pub mod streaming;

pub use selection::select_lod_level;
pub use streaming::MemoryBudget;

use crate::buffer::{BufferPool, BufferType, GpuBuffer};
use crate::error::{GupError, GupResult};
use wgpu::util::DeviceExt;

/// A single point in the LOD pyramid.
///
/// Uses a 16-byte layout (4 × f32) that is naturally aligned for GPU access.
/// The `weight` field stores how many original source points this
/// representative aggregates (1.0 for level-0 points).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexData {
    /// X coordinate in data space.
    pub x: f32,
    /// Y coordinate in data space.
    pub y: f32,
    /// Aggregation weight — number of source points this vertex represents.
    pub weight: f32,
    /// Reserved padding for GPU alignment (keeps struct 16-byte aligned).
    pub _padding: f32,
}

impl VertexData {
    /// Create a new vertex at the given position with weight 1.
    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            weight: 1.0,
            _padding: 0.0,
        }
    }

    /// Create a new vertex with an explicit weight.
    #[inline]
    pub fn with_weight(x: f32, y: f32, weight: f32) -> Self {
        Self {
            x,
            y,
            weight,
            _padding: 0.0,
        }
    }
}

/// Metadata for a single LOD level.
#[derive(Debug, Clone)]
pub struct LodLevelMetadata {
    /// Number of points stored in this level's buffer.
    pub point_count: usize,
    /// Grid cell size used to produce this level (0.0 for level 0).
    pub cell_size: f32,
    /// Spatial bounding box of the source data: `[min_x, min_y, max_x, max_y]`.
    pub bounds: [f32; 4],
}

/// Hierarchical Level-of-Detail pyramid holding progressively coarser
/// representations of a point dataset.
///
/// Level 0 is the full-resolution data; higher levels are coarser.
pub struct LodPyramid {
    /// GPU buffers, one per level (index 0 = full resolution).
    levels: Vec<GpuBuffer<VertexData>>,
    /// Per-level metadata.
    metadata: Vec<LodLevelMetadata>,
    /// Configured memory budget in bytes.
    budget_bytes: u64,
    /// Actual GPU bytes allocated across all levels.
    allocated_bytes: u64,
}

impl LodPyramid {
    /// Number of LOD levels in the pyramid.
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Get the GPU buffer for the given level.
    ///
    /// # Panics
    ///
    /// Panics if `level >= self.level_count()`.
    pub fn buffer(&self, level: usize) -> &GpuBuffer<VertexData> {
        &self.levels[level]
    }

    /// Get metadata for the given level.
    ///
    /// # Panics
    ///
    /// Panics if `level >= self.level_count()`.
    pub fn metadata(&self, level: usize) -> &LodLevelMetadata {
        &self.metadata[level]
    }

    /// Point count stored in the given level.
    pub fn level_point_count(&self, level: usize) -> usize {
        self.metadata[level].point_count
    }

    /// The configured memory budget in bytes.
    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    /// Actual GPU bytes allocated across all levels.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }

    // -----------------------------------------------------------------------
    // Crate-internal mutation API (used by StreamingLodManager)
    // -----------------------------------------------------------------------

    /// Construct an `LodPyramid` from pre-built components.
    ///
    /// This is used by [`StreamingLodManager`](streaming::StreamingLodManager)
    /// to create and maintain its internal pyramid representation.
    pub(crate) fn from_parts(
        levels: Vec<GpuBuffer<VertexData>>,
        metadata: Vec<LodLevelMetadata>,
        budget_bytes: u64,
        allocated_bytes: u64,
    ) -> Self {
        Self {
            levels,
            metadata,
            budget_bytes,
            allocated_bytes,
        }
    }

    /// Mutable reference to the GPU buffer at the given level.
    pub(crate) fn buffer_mut(&mut self, level: usize) -> &mut GpuBuffer<VertexData> {
        &mut self.levels[level]
    }

    /// Mutable reference to the metadata at the given level.
    pub(crate) fn metadata_mut(&mut self, level: usize) -> &mut LodLevelMetadata {
        &mut self.metadata[level]
    }

    /// Update the tracked allocated byte count.
    pub(crate) fn set_allocated_bytes(&mut self, bytes: u64) {
        self.allocated_bytes = bytes;
    }
}

impl std::fmt::Debug for LodPyramid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LodPyramid")
            .field("level_count", &self.levels.len())
            .field("metadata", &self.metadata)
            .field("budget_bytes", &self.budget_bytes)
            .field("allocated_bytes", &self.allocated_bytes)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for constructing an [`LodPyramid`].
///
/// Supports both a synchronous CPU fallback path and (in future) a GPU
/// compute path. The CPU path is suitable for tests and small datasets.
#[derive(Debug, Clone)]
pub struct LodPyramidBuilder {
    /// Number of LOD levels to build (including level 0).
    levels: usize,
    /// Maximum GPU memory budget in bytes (0 = unlimited).
    max_gpu_bytes: u64,
    /// Reduction factor per level — target ratio of input points to output
    /// points at each coarsening step. Default is 4.0.
    reduction_factor: f32,
}

impl Default for LodPyramidBuilder {
    fn default() -> Self {
        Self {
            levels: 5,
            max_gpu_bytes: 0,
            reduction_factor: 4.0,
        }
    }
}

impl LodPyramidBuilder {
    /// Create a new builder with default settings (5 levels, no budget).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of LOD levels (including level 0 = full resolution).
    pub fn levels(mut self, levels: usize) -> Self {
        self.levels = levels.max(1);
        self
    }

    /// Set the maximum GPU memory budget in bytes.
    ///
    /// When the computed pyramid would exceed this budget the builder drops
    /// the highest-resolution levels first and emits a warning.
    /// A value of 0 means unlimited.
    pub fn max_gpu_bytes(mut self, bytes: u64) -> Self {
        self.max_gpu_bytes = bytes;
        self
    }

    /// Set the target reduction factor per level.
    ///
    /// Each coarser level aims to contain roughly `input_count / factor`
    /// points. Default is 4.0.
    pub fn reduction_factor(mut self, factor: f32) -> Self {
        self.reduction_factor = factor.max(1.1);
        self
    }

    /// Build the LOD pyramid on the CPU (synchronous fallback).
    ///
    /// Suitable for tests and small-to-medium datasets. For very large
    /// datasets prefer the GPU compute path.
    pub fn build_cpu(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[VertexData],
    ) -> GupResult<LodPyramid> {
        if data.is_empty() {
            return Err(GupError::InvalidDataFormat {
                message: "Cannot build LOD pyramid from empty data".into(),
            });
        }

        // Compute bounding box of the source data.
        let bounds = compute_bounds(data);
        let vertex_size = std::mem::size_of::<VertexData>() as u64;

        // Build level 0 — upload raw data.
        let mut levels: Vec<GpuBuffer<VertexData>> = Vec::with_capacity(self.levels);
        let mut metadata: Vec<LodLevelMetadata> = Vec::with_capacity(self.levels);
        let mut allocated_bytes: u64 = 0;

        let mut buf0 = GpuBuffer::<VertexData>::new(device, BufferType::Storage, data.len());
        buf0.upload(device, queue, data)?;
        allocated_bytes += data.len() as u64 * vertex_size;

        metadata.push(LodLevelMetadata {
            point_count: data.len(),
            cell_size: 0.0,
            bounds,
        });
        levels.push(buf0);

        // Build coarser levels via grid-based aggregation.
        let mut prev_data: Vec<VertexData> = data.to_vec();

        for level_idx in 1..self.levels {
            if prev_data.len() <= 1 {
                break; // Cannot reduce further.
            }

            let target_count = (prev_data.len() as f32 / self.reduction_factor).max(1.0) as usize;
            let grid_side = (target_count as f32).sqrt().max(1.0).ceil() as usize;
            let cell_size_x = (bounds[2] - bounds[0]) / grid_side as f32;
            let cell_size_y = (bounds[3] - bounds[1]) / grid_side as f32;
            let cell_size = cell_size_x.max(cell_size_y);

            let aggregated = aggregate_grid(&prev_data, &bounds, grid_side);

            if aggregated.is_empty() {
                break;
            }

            // Check memory budget before allocating.
            let new_bytes = aggregated.len() as u64 * vertex_size;
            if self.max_gpu_bytes > 0 && allocated_bytes + new_bytes > self.max_gpu_bytes {
                let dropped = self.levels - level_idx;
                log::warn!(
                    "LOD pyramid memory budget exceeded: dropping {} level(s) \
                     (budget={} bytes, would need {} bytes)",
                    dropped,
                    self.max_gpu_bytes,
                    allocated_bytes + new_bytes,
                );
                break;
            }

            let mut buf =
                GpuBuffer::<VertexData>::new(device, BufferType::Storage, aggregated.len());
            buf.upload(device, queue, &aggregated)?;
            allocated_bytes += new_bytes;

            metadata.push(LodLevelMetadata {
                point_count: aggregated.len(),
                cell_size,
                bounds,
            });
            levels.push(buf);

            prev_data = aggregated;
        }

        // Enforce memory budget — drop highest-resolution levels first.
        if self.max_gpu_bytes > 0 && allocated_bytes > self.max_gpu_bytes {
            let mut drop_count = 0;
            while levels.len() > 1 && allocated_bytes > self.max_gpu_bytes {
                let removed_bytes = metadata[0].point_count as u64 * vertex_size;
                levels.remove(0);
                metadata.remove(0);
                allocated_bytes -= removed_bytes;
                drop_count += 1;
            }
            if drop_count > 0 {
                log::warn!(
                    "LOD pyramid budget enforced: dropped {} highest-resolution level(s)",
                    drop_count,
                );
            }
        }

        Ok(LodPyramid {
            levels,
            metadata,
            budget_bytes: self.max_gpu_bytes,
            allocated_bytes,
        })
    }

    /// Build the LOD pyramid using GPU compute shaders.
    ///
    /// This is the high-performance path for large datasets. It dispatches
    /// a grid-based aggregation compute shader for each level, reading from
    /// the previous level's output buffer so that total build cost scales
    /// with the output size rather than the input size.
    pub async fn build_gpu(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pool: &mut BufferPool,
        data: &[VertexData],
    ) -> GupResult<LodPyramid> {
        if data.is_empty() {
            return Err(GupError::InvalidDataFormat {
                message: "Cannot build LOD pyramid from empty data".into(),
            });
        }

        let bounds = compute_bounds(data);
        let vertex_size = std::mem::size_of::<VertexData>() as u64;

        // Upload level-0 data.
        let mut buf0: GpuBuffer<VertexData> = pool.allocate(BufferType::Storage, data.len());
        buf0.upload(device, queue, data)?;

        let mut levels = vec![buf0];
        let mut metadata = vec![LodLevelMetadata {
            point_count: data.len(),
            cell_size: 0.0,
            bounds,
        }];
        let mut allocated_bytes = data.len() as u64 * vertex_size;

        // Create the compute pipeline once.
        let (assign_pipeline, compact_pipeline, bind_group_layout) =
            create_aggregate_pipelines(device);

        let mut prev_count = data.len();

        for _level_idx in 1..self.levels {
            if prev_count <= 1 {
                break;
            }

            let target_count = (prev_count as f32 / self.reduction_factor).max(1.0) as usize;
            let grid_side = (target_count as f32).sqrt().max(1.0).ceil() as usize;
            let total_cells = grid_side * grid_side;
            let cell_size_x = (bounds[2] - bounds[0]) / grid_side as f32;
            let cell_size_y = (bounds[3] - bounds[1]) / grid_side as f32;
            let cell_size = cell_size_x.max(cell_size_y);

            // Allocate output buffer (max = total_cells) and grid buffer.
            let output_buf: GpuBuffer<VertexData> = pool.allocate(BufferType::Storage, total_cells);

            // Grid buffer: one u32 per cell, initialized to 0xFFFFFFFF.
            let grid_init: Vec<u32> = vec![0xFFFF_FFFFu32; total_cells];
            let mut grid_buf = GpuBuffer::<u32>::new(device, BufferType::Storage, total_cells);
            grid_buf.upload(device, queue, &grid_init)?;

            // Counter buffer: single atomic u32.
            let counter_init = [0u32];
            let mut counter_buf = GpuBuffer::<u32>::new(device, BufferType::Storage, 1);
            counter_buf.upload(device, queue, &counter_init)?;

            // Uniform buffer with grid params.
            let params = AggregateParams {
                grid_width: grid_side as u32,
                grid_height: grid_side as u32,
                min_x: bounds[0],
                min_y: bounds[1],
                max_x: bounds[2],
                max_y: bounds[3],
                input_count: prev_count as u32,
                _padding: 0,
            };
            let params_bytes = bytemuck::bytes_of(&params);
            let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("lod_aggregate_params"),
                contents: params_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            // Create bind group.
            let input_buffer = levels.last().unwrap().buffer();
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lod_aggregate_bind_group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: grid_buf.buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: counter_buf.buffer().as_entire_binding(),
                    },
                ],
            });

            // Dispatch assign pass.
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("lod_assign"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&assign_pipeline);
                cpass.set_bind_group(0, &bind_group, &[]);
                let workgroups = (prev_count as u32).div_ceil(256);
                cpass.dispatch_workgroups(workgroups, 1, 1);
            }
            // Dispatch compact pass.
            {
                let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("lod_compact"),
                    timestamp_writes: None,
                });
                cpass.set_pipeline(&compact_pipeline);
                cpass.set_bind_group(0, &bind_group, &[]);
                let workgroups = (total_cells as u32).div_ceil(256);
                cpass.dispatch_workgroups(workgroups, 1, 1);
            }
            queue.submit(std::iter::once(encoder.finish()));

            // Read back the output count.
            let output_count = read_counter(device, queue, &counter_buf).await?;

            if output_count == 0 {
                break;
            }

            // Check memory budget.
            let new_bytes = output_count as u64 * vertex_size;
            if self.max_gpu_bytes > 0 && allocated_bytes + new_bytes > self.max_gpu_bytes {
                let dropped = self.levels - levels.len();
                log::warn!(
                    "LOD pyramid memory budget exceeded: dropping {} level(s) \
                     (budget={} bytes, would need {} bytes)",
                    dropped,
                    self.max_gpu_bytes,
                    allocated_bytes + new_bytes,
                );
                break;
            }

            allocated_bytes += new_bytes;
            metadata.push(LodLevelMetadata {
                point_count: output_count as usize,
                cell_size,
                bounds,
            });
            levels.push(output_buf);
            prev_count = output_count as usize;
        }

        // Enforce memory budget — drop highest-resolution levels first.
        if self.max_gpu_bytes > 0 && allocated_bytes > self.max_gpu_bytes {
            let mut drop_count = 0;
            while levels.len() > 1 && allocated_bytes > self.max_gpu_bytes {
                let removed_bytes = metadata[0].point_count as u64 * vertex_size;
                levels.remove(0);
                metadata.remove(0);
                allocated_bytes -= removed_bytes;
                drop_count += 1;
            }
            if drop_count > 0 {
                log::warn!(
                    "LOD pyramid budget enforced: dropped {} highest-resolution level(s)",
                    drop_count,
                );
            }
        }

        Ok(LodPyramid {
            levels,
            metadata,
            budget_bytes: self.max_gpu_bytes,
            allocated_bytes,
        })
    }
}

// ---------------------------------------------------------------------------
// CPU grid aggregation
// ---------------------------------------------------------------------------

/// Compute the axis-aligned bounding box of a set of points.
///
/// Returns `[min_x, min_y, max_x, max_y]`.
fn compute_bounds(data: &[VertexData]) -> [f32; 4] {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for v in data {
        min_x = min_x.min(v.x);
        min_y = min_y.min(v.y);
        max_x = max_x.max(v.x);
        max_y = max_y.max(v.y);
    }
    // Ensure non-zero extent to avoid division by zero.
    if (max_x - min_x).abs() < f32::EPSILON {
        max_x = min_x + 1.0;
    }
    if (max_y - min_y).abs() < f32::EPSILON {
        max_y = min_y + 1.0;
    }
    [min_x, min_y, max_x, max_y]
}

/// Grid-based aggregation: select one representative point per occupied cell.
///
/// Uses a deterministic first-point-wins strategy (lowest index) to match the
/// GPU compute shader's `atomicMin` behaviour.
fn aggregate_grid(data: &[VertexData], bounds: &[f32; 4], grid_side: usize) -> Vec<VertexData> {
    let [min_x, min_y, max_x, max_y] = *bounds;
    let extent_x = max_x - min_x;
    let extent_y = max_y - min_y;

    // Grid stores the index of the representative point (or usize::MAX if empty).
    let total_cells = grid_side * grid_side;
    let mut grid: Vec<usize> = vec![usize::MAX; total_cells];

    for (idx, v) in data.iter().enumerate() {
        let cx = (((v.x - min_x) / extent_x) * grid_side as f32) as usize;
        let cy = (((v.y - min_y) / extent_y) * grid_side as f32) as usize;
        let cx = cx.min(grid_side - 1);
        let cy = cy.min(grid_side - 1);
        let cell = cy * grid_side + cx;

        // First-point-wins: keep the lowest index.
        if grid[cell] == usize::MAX || idx < grid[cell] {
            grid[cell] = idx;
        }
    }

    grid.iter()
        .filter(|&&idx| idx != usize::MAX)
        .map(|&idx| data[idx])
        .collect()
}

// ---------------------------------------------------------------------------
// GPU compute pipeline
// ---------------------------------------------------------------------------

/// Uniform parameters for the aggregation compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AggregateParams {
    grid_width: u32,
    grid_height: u32,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    input_count: u32,
    _padding: u32,
}

/// Create the assign and compact compute pipelines for grid aggregation.
fn create_aggregate_pipelines(
    device: &wgpu::Device,
) -> (
    wgpu::ComputePipeline,
    wgpu::ComputePipeline,
    wgpu::BindGroupLayout,
) {
    let shader_src = include_str!("../shaders/lod_aggregate.compute.wgsl");
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("lod_aggregate_shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("lod_aggregate_bgl"),
        entries: &[
            // binding 0: input VertexData[] (read-only storage)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 1: output VertexData[] (read-write storage)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 2: params uniform
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 3: grid[] (read-write storage of atomic u32)
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 4: output_counter (read-write storage of atomic u32)
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lod_aggregate_pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let assign_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("lod_assign_pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("assign_main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let compact_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("lod_compact_pipeline"),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("compact_main"),
        compilation_options: Default::default(),
        cache: None,
    });

    (assign_pipeline, compact_pipeline, bind_group_layout)
}

/// Read back the value of a single-element u32 counter buffer from the GPU.
async fn read_counter(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    counter_buf: &GpuBuffer<u32>,
) -> GupResult<u32> {
    // Create a staging buffer for readback.
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lod_counter_staging"),
        size: 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_buffer_to_buffer(counter_buf.buffer(), 0, &staging, 0, 4);
    queue.submit(std::iter::once(encoder.finish()));

    // Map and read.
    let slice = staging.slice(..);
    let (sender, receiver) = futures::channel::oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    receiver
        .await
        .map_err(|_| GupError::BufferError {
            message: "Counter readback channel cancelled".into(),
        })?
        .map_err(|e| GupError::BufferError {
            message: format!("Counter readback failed: {e}"),
        })?;

    let mapped = slice.get_mapped_range();
    let value = *bytemuck::from_bytes::<u32>(&mapped);
    drop(mapped);
    staging.unmap();

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_data_layout() {
        assert_eq!(std::mem::size_of::<VertexData>(), 16);
        assert_eq!(std::mem::align_of::<VertexData>(), 4);
        assert_eq!(std::mem::offset_of!(VertexData, x), 0);
        assert_eq!(std::mem::offset_of!(VertexData, y), 4);
        assert_eq!(std::mem::offset_of!(VertexData, weight), 8);
        assert_eq!(std::mem::offset_of!(VertexData, _padding), 12);
    }

    #[test]
    fn vertex_data_constructors() {
        let v = VertexData::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.weight, 1.0);

        let v2 = VertexData::with_weight(3.0, 4.0, 10.0);
        assert_eq!(v2.weight, 10.0);
    }

    #[test]
    fn compute_bounds_basic() {
        let data = vec![
            VertexData::new(0.0, 0.0),
            VertexData::new(10.0, 20.0),
            VertexData::new(5.0, -5.0),
        ];
        let b = compute_bounds(&data);
        assert_eq!(b, [0.0, -5.0, 10.0, 20.0]);
    }

    #[test]
    fn compute_bounds_single_point() {
        let data = vec![VertexData::new(5.0, 5.0)];
        let b = compute_bounds(&data);
        // Should expand to avoid zero extent.
        assert!(b[2] > b[0]);
        assert!(b[3] > b[1]);
    }

    #[test]
    fn aggregate_grid_reduces_points() {
        // 100 points in a 10x10 area → 3x3 grid should produce ≤9 points.
        let mut data = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                data.push(VertexData::new(i as f32, j as f32));
            }
        }
        let bounds = compute_bounds(&data);
        let result = aggregate_grid(&data, &bounds, 3);
        assert!(result.len() <= 9, "Expected ≤9, got {}", result.len());
        assert!(!result.is_empty());
    }

    #[test]
    fn aggregate_grid_deterministic() {
        let data = vec![
            VertexData::new(0.0, 0.0),
            VertexData::new(0.1, 0.1),
            VertexData::new(5.0, 5.0),
        ];
        let bounds = compute_bounds(&data);
        let r1 = aggregate_grid(&data, &bounds, 2);
        let r2 = aggregate_grid(&data, &bounds, 2);
        assert_eq!(r1.len(), r2.len());
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.x, b.x);
            assert_eq!(a.y, b.y);
        }
    }

    #[test]
    fn aggregate_params_layout() {
        assert_eq!(std::mem::size_of::<AggregateParams>(), 32);
    }

    #[tokio::test]
    async fn build_cpu_small_dataset() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data: Vec<VertexData> = (0..100)
            .map(|i| VertexData::new((i % 10) as f32, (i / 10) as f32))
            .collect();

        let pyramid = LodPyramidBuilder::new()
            .levels(3)
            .build_cpu(device, queue, &data)
            .unwrap();

        assert!(pyramid.level_count() >= 2);
        assert_eq!(pyramid.level_point_count(0), 100);
        // Each subsequent level should have fewer points.
        for i in 1..pyramid.level_count() {
            assert!(
                pyramid.level_point_count(i) < pyramid.level_point_count(i - 1),
                "Level {} ({}) should have fewer points than level {} ({})",
                i,
                pyramid.level_point_count(i),
                i - 1,
                pyramid.level_point_count(i - 1),
            );
        }
    }

    #[tokio::test]
    async fn build_cpu_empty_data_returns_error() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let result = LodPyramidBuilder::new().build_cpu(ctx.device(), ctx.queue(), &[]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn build_cpu_memory_budget() {
        let guard = crate::test_utils::create_test_context().await.unwrap();
        let ctx = guard.context();
        let device = ctx.device();
        let queue = ctx.queue();

        let data: Vec<VertexData> = (0..1000)
            .map(|i| VertexData::new((i % 32) as f32, (i / 32) as f32))
            .collect();

        // Set a very small budget that can only hold a fraction.
        let vertex_size = std::mem::size_of::<VertexData>() as u64;
        let budget = data.len() as u64 * vertex_size + 100; // Just enough for level 0 + a tiny bit.

        let pyramid = LodPyramidBuilder::new()
            .levels(5)
            .max_gpu_bytes(budget)
            .build_cpu(device, queue, &data)
            .unwrap();

        // Budget should prevent building all 5 levels.
        assert!(
            pyramid.level_count() < 5,
            "Expected fewer than 5 levels with tight budget, got {}",
            pyramid.level_count()
        );
        assert!(pyramid.allocated_bytes() <= budget);
    }
}
