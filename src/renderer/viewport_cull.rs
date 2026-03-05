// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU viewport frustum culler for `VertexData` points.
//!
//! Operates on a selected LOD tier's GPU buffer, producing a compacted output
//! buffer and an indirect draw argument buffer with no CPU readback.
//!
//! # Pipeline
//!
//! 1. **Cull** — mark points inside/outside the viewport bounds.
//! 2. **Prefix sum** — compute output indices via parallel scan.
//! 3. **Compact** — write visible points to a dense output buffer.

use crate::error::{GupError, GupResult};
use std::sync::Arc;
use wgpu::*;

/// Maximum number of points supported in a single cull dispatch.
///
/// Limited by the prefix-sum block scan (256 workgroups × 256 threads).
pub const MAX_POINTS: u32 = 256 * 256 * 256; // ~16.7M

const WORKGROUP_SIZE: u32 = 256;

/// GPU configuration uniform matching the WGSL `CullConfig` struct.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CullConfig {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    point_count: u32,
    vertex_count: u32,
    _pad0: u32,
    _pad1: u32,
}

/// Result of a viewport culling dispatch.
pub struct ViewportCullResult {
    /// Compacted buffer of visible `VertexData` points.
    pub output_buffer: Arc<Buffer>,
    /// Indirect draw argument buffer `[vertex_count, instance_count, 0, 0]`.
    pub draw_indirect_buffer: Arc<Buffer>,
}

/// Viewport frustum culler for `VertexData` points.
///
/// Encapsulates the GPU compute pipelines for frustum culling, prefix-sum
/// scanning, and stream compaction of LOD pyramid points.
pub struct ViewportCuller {
    cull_pipeline: ComputePipeline,
    prefix_sum_pipeline: ComputePipeline,
    prefix_sum_blocks_pipeline: ComputePipeline,
    prefix_sum_super_blocks_pipeline: ComputePipeline,
    prefix_sum_add_offsets_pipeline: ComputePipeline,
    compact_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl ViewportCuller {
    /// Create a new viewport culler.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader_source = include_str!("../shaders/viewport_cull.compute.wgsl");
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("viewport_cull_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("viewport_cull_bgl"),
            entries: &[
                // binding 0: input points (read-only)
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
                // binding 1: output points
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
                // binding 2: config uniform
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: visibility flags
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
                // binding 4: prefix sums
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
                // binding 5: draw indirect
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: block sums
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 7: super-block sums
                BindGroupLayoutEntry {
                    binding: 7,
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
            label: Some("viewport_cull_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let make_pipeline = |entry: &str, label: &str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        Ok(Self {
            cull_pipeline: make_pipeline("cull_points", "viewport_cull_pipeline"),
            prefix_sum_pipeline: make_pipeline(
                "prefix_sum_workgroup",
                "viewport_prefix_sum_pipeline",
            ),
            prefix_sum_blocks_pipeline: make_pipeline(
                "prefix_sum_blocks",
                "viewport_prefix_sum_blocks_pipeline",
            ),
            prefix_sum_super_blocks_pipeline: make_pipeline(
                "prefix_sum_super_blocks",
                "viewport_prefix_sum_super_blocks_pipeline",
            ),
            prefix_sum_add_offsets_pipeline: make_pipeline(
                "prefix_sum_add_offsets",
                "viewport_prefix_sum_add_offsets_pipeline",
            ),
            compact_pipeline: make_pipeline("compact_points", "viewport_compact_pipeline"),
            bind_group_layout,
        })
    }

    /// Dispatch the full culling pipeline.
    ///
    /// # Parameters
    ///
    /// - `input_buffer`: the selected LOD tier's `VertexData` GPU buffer.
    /// - `point_count`: number of points in the input buffer.
    /// - `vertex_count`: vertices per instance for the indirect draw (usually 1
    ///   for point rendering, or the vertex count of the mark quad).
    /// - `bounds`: viewport frustum `[min_x, max_x, min_y, max_y]` in world space.
    ///
    /// # Returns
    ///
    /// A [`ViewportCullResult`] containing the compacted output buffer and an
    /// indirect draw argument buffer. No CPU readback occurs.
    pub async fn dispatch(
        &self,
        device: &Device,
        queue: &Queue,
        input_buffer: &Buffer,
        point_count: u32,
        vertex_count: u32,
        bounds: [f32; 4],
    ) -> GupResult<ViewportCullResult> {
        if point_count == 0 {
            return Err(GupError::invalid_operation(
                "Cannot cull zero points".to_string(),
            ));
        }
        if point_count > MAX_POINTS {
            return Err(GupError::invalid_operation(format!(
                "Point count {point_count} exceeds maximum {MAX_POINTS}"
            )));
        }

        let point_size = std::mem::size_of::<crate::lod::VertexData>() as u64;
        let num_workgroups = point_count.div_ceil(WORKGROUP_SIZE);

        // Allocate transient GPU buffers.
        let output_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_output"),
            size: point_count as u64 * point_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let visibility_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_visibility"),
            size: point_count as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let prefix_sums_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_prefix_sums"),
            size: point_count as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_draw_indirect"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let config = CullConfig {
            min_x: bounds[0],
            max_x: bounds[1],
            min_y: bounds[2],
            max_y: bounds[3],
            point_count,
            vertex_count,
            _pad0: 0,
            _pad1: 0,
        };

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_config"),
            size: std::mem::size_of::<CullConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));

        let block_sums_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_block_sums"),
            size: (num_workgroups.max(1) as u64) * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let num_block_chunks = num_workgroups.div_ceil(WORKGROUP_SIZE);
        let super_block_sums_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_super_block_sums"),
            size: (num_block_chunks.max(1) as u64) * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create bind group.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("viewport_cull_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: visibility_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: prefix_sums_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: draw_indirect_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: block_sums_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: super_block_sums_buffer.as_entire_binding(),
                },
            ],
        });

        // Encode compute passes.
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("viewport_cull_encoder"),
        });

        // Pass 1: Frustum cull.
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_cull_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.cull_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Pass 2: Per-workgroup prefix sum.
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_prefix_sum_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.prefix_sum_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Pass 3: Scan block sums (one workgroup per chunk of 256 blocks).
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_prefix_sum_blocks_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.prefix_sum_blocks_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_block_chunks, 1, 1);
        }

        // Pass 3b: Scan super-block sums (single workgroup) + write draw_indirect.
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_prefix_sum_super_blocks_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.prefix_sum_super_blocks_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(1, 1, 1);
        }

        // Pass 4: Add block + super-block offsets.
        if num_workgroups > 1 {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_add_offsets_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.prefix_sum_add_offsets_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Pass 5: Compact visible points.
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("viewport_compact_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.compact_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit([encoder.finish()]);

        Ok(ViewportCullResult {
            output_buffer,
            draw_indirect_buffer,
        })
    }

    /// Read back the draw indirect buffer for testing.
    ///
    /// Returns `[vertex_count, instance_count, first_vertex, first_instance]`.
    pub async fn read_draw_indirect(
        &self,
        device: &Device,
        queue: &Queue,
        result: &ViewportCullResult,
    ) -> GupResult<[u32; 4]> {
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("viewport_cull_readback"),
            size: 16,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("viewport_cull_readback_encoder"),
        });
        encoder.copy_buffer_to_buffer(&result.draw_indirect_buffer, 0, &staging, 0, 16);
        queue.submit([encoder.finish()]);

        let slice = staging.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        receiver
            .await
            .map_err(|_| GupError::RenderError {
                message: "Buffer map cancelled".to_string(),
            })?
            .map_err(|e| GupError::RenderError {
                message: format!("Buffer map failed: {e}"),
            })?;

        let data = slice.get_mapped_range();
        let values: [u32; 4] = bytemuck::cast_slice(&data)[0..4].try_into().unwrap();
        drop(data);
        staging.unmap();

        Ok(values)
    }
}

impl std::fmt::Debug for ViewportCuller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewportCuller").finish_non_exhaustive()
    }
}
