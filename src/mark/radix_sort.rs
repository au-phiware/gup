// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU radix sort for Z-order instance sorting.
//!
//! Implements a parallel 8-bit radix sort that runs entirely on the GPU
//! to sort visible instances by Z-depth in back-to-front order. This
//! enables correct rendering of overlapping transparent marks in 3D
//! visualizations and depth-varying 2D scenes.
//!
//! The sort operates on the compacted output buffer from the
//! [`ComputeInstanceFilter`] pipeline, sorting visible instances by the
//! Z component of their transform matrix (`transform[3].z` in WGSL).
//!
//! # Algorithm
//!
//! 4-pass 8-bit radix sort (LSD — least significant digit first):
//!
//! 1. **Extract keys** — convert Z-depth float to a sortable u32
//!    (descending order for back-to-front rendering).
//! 2. **For each of 4 passes** (processing bits 0–7, 8–15, 16–23, 24–31):
//!    a. Build per-workgroup histograms of the current 8-bit digit.
//!    b. Multi-level prefix sum over the histogram for scatter offsets.
//!    c. Scatter keys and index values to their sorted positions.
//! 3. **Reorder instances** — copy instances from source to destination
//!    using the final sorted index permutation.
//!
//! [`ComputeInstanceFilter`]: super::compute_instance_filter::ComputeInstanceFilter

use crate::error::{GupError, GupResult};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device,
    PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

/// Workgroup size used by all radix sort compute entry points.
const WORKGROUP_SIZE: u32 = 256;

/// Number of bits per radix digit.
const RADIX_BITS: u32 = 8;

/// Number of possible digit values (2^RADIX_BITS).
const RADIX_SIZE: u32 = 1 << RADIX_BITS;

/// Number of radix passes for a 32-bit key.
const NUM_PASSES: u32 = 32 / RADIX_BITS;

// ---------------------------------------------------------------------------
// GPU-side sort config uniform (must match WGSL `SortConfig`)
// ---------------------------------------------------------------------------

/// Configuration uniform uploaded to the GPU for the radix sort shader.
///
/// Layout must exactly match the WGSL `SortConfig` struct (32 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SortConfig {
    /// Total number of elements to sort.
    pub num_elements: u32,
    /// Current radix pass (0–3).
    pub radix_pass: u32,
    /// Number of sort workgroups = ceil(num_elements / 256).
    pub num_sort_wg: u32,
    /// For prefix sum: total number of histogram entries to scan.
    pub prefix_count: u32,
    /// For prefix sum: offset in histograms[] where block totals start.
    pub prefix_block_offset: u32,
    /// For prefix sum: offset in histograms[] where the data to scan starts.
    pub prefix_data_offset: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

// ---------------------------------------------------------------------------
// Radix sorter
// ---------------------------------------------------------------------------

/// GPU compute pipeline for 8-bit radix sort of instances by Z-depth.
///
/// Sorts visible instances in back-to-front order (descending Z) for
/// correct transparent rendering. Operates on the compacted output
/// buffer from [`ComputeInstanceFilter`].
///
/// # Example
///
/// ```rust,ignore
/// let sorter = RadixSorter::new(&device)?;
/// sorter.encode_sort(
///     &device, &queue, &mut encoder,
///     &output_buffer,        // compacted instances from filter
///     &draw_indirect_buffer, // contains visible count
///     &sort_scratch_buffer,  // destination for sorted instances
///     &sort_buffers,         // pre-allocated sort working buffers
///     instance_count,
/// );
/// ```
///
/// [`ComputeInstanceFilter`]: super::compute_instance_filter::ComputeInstanceFilter
pub struct RadixSorter {
    /// Extract sort keys from instances.
    extract_keys_pipeline: ComputePipeline,
    /// Build per-workgroup histogram.
    histogram_pipeline: ComputePipeline,
    /// Per-workgroup prefix sum over histogram.
    scan_wg_pipeline: ComputePipeline,
    /// Scan block totals.
    scan_blocks_pipeline: ComputePipeline,
    /// Add block offsets.
    scan_add_offsets_pipeline: ComputePipeline,
    /// Scatter keys and values.
    scatter_pipeline: ComputePipeline,
    /// Reorder instances using sorted indices.
    reorder_pipeline: ComputePipeline,
    /// Bind group layout for all sort passes.
    bind_group_layout: BindGroupLayout,
}

/// Pre-allocated GPU buffers for the radix sort working set.
///
/// These buffers are reused across sort invocations (and across frames
/// in the pooled path) to avoid per-frame allocation overhead.
pub struct SortBuffers {
    /// Key buffer A (ping-pong).
    pub keys_a: Buffer,
    /// Key buffer B (ping-pong).
    pub keys_b: Buffer,
    /// Value (index) buffer A (ping-pong).
    pub vals_a: Buffer,
    /// Value (index) buffer B (ping-pong).
    pub vals_b: Buffer,
    /// Per-workgroup histograms + block totals for prefix sum.
    pub histograms: Buffer,
    /// Sort configuration uniform buffer.
    pub config: Buffer,
    /// Current capacity in elements.
    capacity: u32,
}

impl SortBuffers {
    /// Allocate sort working buffers for up to `capacity` elements.
    pub fn new(device: &Device, capacity: u32) -> Self {
        let key_size = capacity as u64 * 4;
        let num_wg = capacity.div_ceil(WORKGROUP_SIZE);
        // Histogram: 256 * num_wg entries.
        // Block totals for prefix sum: up to 3 levels.
        // Level 0: num_wg block totals
        // Level 1: ceil(num_wg / 256) block totals
        // Level 2: 1 block total (at most)
        let hist_entries = RADIX_SIZE * num_wg;
        let l0_blocks = hist_entries.div_ceil(WORKGROUP_SIZE);
        let l1_blocks = l0_blocks.div_ceil(WORKGROUP_SIZE);
        let total_hist = hist_entries + l0_blocks + l1_blocks + 1;
        let hist_size = total_hist as u64 * 4;

        let make_buf = |label, size, usage: BufferUsages| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };

        let storage_flags = BufferUsages::STORAGE | BufferUsages::COPY_SRC;

        Self {
            keys_a: make_buf("sort_keys_a", key_size, storage_flags),
            keys_b: make_buf("sort_keys_b", key_size, storage_flags),
            vals_a: make_buf("sort_vals_a", key_size, storage_flags),
            vals_b: make_buf("sort_vals_b", key_size, storage_flags),
            histograms: make_buf("sort_histograms", hist_size, storage_flags),
            config: make_buf(
                "sort_config",
                std::mem::size_of::<SortConfig>() as u64,
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            ),
            capacity,
        }
    }

    /// Current capacity in elements.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}

impl RadixSorter {
    /// Create a new radix sorter, compiling the WGSL shader and creating
    /// all seven compute pipelines.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader_source = include_str!("../shaders/radix_sort.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("radix_sort_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("radix_sort_bgl"),
            entries: &[
                // binding 0: instances_src (read-only storage)
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
                // binding 1: instances_dst (read-write storage)
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
                // binding 2: keys_a (read-write storage)
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
                // binding 3: keys_b (read-write storage)
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
                // binding 4: vals_a (read-write storage)
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
                // binding 5: vals_b (read-write storage)
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
                // binding 6: histograms (read-write storage)
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
                // binding 7: draw_indirect (read-only storage)
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 8: sort_config (uniform)
                BindGroupLayoutEntry {
                    binding: 8,
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("radix_sort_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let make_pipeline = |label: &'static str, entry: &'static str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };

        let extract_keys_pipeline =
            make_pipeline("extract_sort_keys_pipeline", "extract_sort_keys");
        let histogram_pipeline = make_pipeline("radix_histogram_pipeline", "radix_histogram");
        let scan_wg_pipeline =
            make_pipeline("histogram_scan_wg_pipeline", "histogram_scan_workgroup");
        let scan_blocks_pipeline =
            make_pipeline("histogram_scan_blocks_pipeline", "histogram_scan_blocks");
        let scan_add_offsets_pipeline = make_pipeline(
            "histogram_scan_add_offsets_pipeline",
            "histogram_scan_add_offsets",
        );
        let scatter_pipeline = make_pipeline("radix_scatter_pipeline", "radix_scatter");
        let reorder_pipeline = make_pipeline("reorder_instances_pipeline", "reorder_instances");

        Ok(Self {
            extract_keys_pipeline,
            histogram_pipeline,
            scan_wg_pipeline,
            scan_blocks_pipeline,
            scan_add_offsets_pipeline,
            scatter_pipeline,
            reorder_pipeline,
            bind_group_layout,
        })
    }

    /// Encode the full radix sort into a command encoder.
    ///
    /// After encoding, `instances_dst` contains the sorted instances in
    /// back-to-front order (only the first `visible_count` entries, as
    /// recorded in `draw_indirect_buffer`).
    ///
    /// # Arguments
    ///
    /// * `instances_src` — compacted output buffer from the filter pipeline
    /// * `instances_dst` — destination buffer for sorted instances
    /// * `draw_indirect_buffer` — contains visible count from filter
    /// * `sort_buffers` — pre-allocated working buffers
    /// * `num_elements` — total number of slots (= filter `instance_count`)
    #[allow(clippy::too_many_arguments)]
    pub fn encode_sort(
        &self,
        device: &Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        instances_src: &Buffer,
        instances_dst: &Buffer,
        draw_indirect_buffer: &Buffer,
        sort_buffers: &SortBuffers,
        num_elements: u32,
    ) {
        let num_sort_wg = num_elements.div_ceil(WORKGROUP_SIZE);
        let hist_count = RADIX_SIZE * num_sort_wg;

        // Create bind group.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("radix_sort_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: instances_src.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: instances_dst.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: sort_buffers.keys_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: sort_buffers.keys_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: sort_buffers.vals_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: sort_buffers.vals_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: sort_buffers.histograms.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: draw_indirect_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: sort_buffers.config.as_entire_binding(),
                },
            ],
        });

        // Collect all SortConfig values needed for the entire sort.
        // We must use a staging buffer + copy_buffer_to_buffer because
        // queue.write_buffer batches all writes before the command buffer
        // executes, so only the last write would be visible.
        let mut configs: Vec<SortConfig> = Vec::new();

        let base_config = |radix_pass: u32| SortConfig {
            num_elements,
            radix_pass,
            num_sort_wg,
            prefix_count: 0,
            prefix_block_offset: 0,
            prefix_data_offset: 0,
            _pad0: 0,
            _pad1: 0,
        };

        // Config 0: extract_sort_keys
        configs.push(base_config(0));

        // For each radix pass: histogram + prefix sum configs + scatter
        for radix_pass in 0..NUM_PASSES {
            // Histogram
            configs.push(base_config(radix_pass));

            // Prefix sum configs (depends on hist_count)
            Self::collect_prefix_sum_configs(
                &mut configs,
                num_elements,
                radix_pass,
                num_sort_wg,
                hist_count,
            );

            // Scatter
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: hist_count,
                prefix_block_offset: hist_count,
                prefix_data_offset: 0,
                _pad0: 0,
                _pad1: 0,
            });
        }

        // Config for reorder
        configs.push(base_config(0));

        // Create staging buffer and upload all configs at once.
        let config_size = std::mem::size_of::<SortConfig>() as u64;
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("sort_config_staging"),
            size: configs.len() as u64 * config_size,
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&staging, 0, bytemuck::cast_slice(&configs));

        let mut config_idx: u64 = 0;

        // Helper to copy config and advance index.
        let copy_config = |enc: &mut wgpu::CommandEncoder, idx: &mut u64, config_buf: &Buffer| {
            enc.copy_buffer_to_buffer(&staging, *idx * config_size, config_buf, 0, config_size);
            *idx += 1;
        };

        // Step 1: Extract sort keys.
        copy_config(encoder, &mut config_idx, &sort_buffers.config);
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("extract_sort_keys"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.extract_keys_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_sort_wg, 1, 1);
        }

        // Step 2: 4 radix passes.
        for _radix_pass in 0..NUM_PASSES {
            // 2a: Build histogram.
            copy_config(encoder, &mut config_idx, &sort_buffers.config);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("radix_histogram"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.histogram_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(num_sort_wg, 1, 1);
            }

            // 2b: Prefix sum over histogram.
            self.encode_prefix_sum_dispatches(
                encoder,
                &bind_group,
                &sort_buffers.config,
                &staging,
                &mut config_idx,
                config_size,
                hist_count,
            );

            // 2c: Scatter.
            copy_config(encoder, &mut config_idx, &sort_buffers.config);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("radix_scatter"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scatter_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(num_sort_wg, 1, 1);
            }
        }

        // Step 3: Reorder instances.
        copy_config(encoder, &mut config_idx, &sort_buffers.config);
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("reorder_instances"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.reorder_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_sort_wg, 1, 1);
        }
    }

    /// Collect the SortConfig values needed for the multi-level prefix sum.
    fn collect_prefix_sum_configs(
        configs: &mut Vec<SortConfig>,
        num_elements: u32,
        radix_pass: u32,
        num_sort_wg: u32,
        hist_count: u32,
    ) {
        let num_prefix_wg_0 = hist_count.div_ceil(WORKGROUP_SIZE);

        if num_prefix_wg_0 <= 1 {
            // Single-workgroup scan.
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: hist_count,
                prefix_block_offset: hist_count,
                prefix_data_offset: 0,
                _pad0: 0,
                _pad1: 0,
            });
        } else if num_prefix_wg_0 <= WORKGROUP_SIZE {
            // 2-level: wg scan + block scan + add offsets
            let block_offset_0 = hist_count;
            let scan_cfg = SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: hist_count,
                prefix_block_offset: block_offset_0,
                prefix_data_offset: 0,
                _pad0: 0,
                _pad1: 0,
            };
            configs.push(scan_cfg); // wg scan
            configs.push(scan_cfg); // block scan (same config)
            if num_prefix_wg_0 > 1 {
                configs.push(scan_cfg); // add offsets (same config)
            }
        } else {
            // 3-level
            let block_offset_0 = hist_count;
            let num_prefix_wg_1 = num_prefix_wg_0.div_ceil(WORKGROUP_SIZE);
            let block_offset_1 = block_offset_0 + num_prefix_wg_0;

            // L0 wg scan
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: hist_count,
                prefix_block_offset: block_offset_0,
                prefix_data_offset: 0,
                _pad0: 0,
                _pad1: 0,
            });
            // L1 wg scan
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: num_prefix_wg_0,
                prefix_block_offset: block_offset_1,
                prefix_data_offset: block_offset_0,
                _pad0: 0,
                _pad1: 0,
            });
            // L1 block scan
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: num_prefix_wg_0,
                prefix_block_offset: block_offset_1,
                prefix_data_offset: block_offset_0,
                _pad0: 0,
                _pad1: 0,
            });
            // L1 add offsets (if needed)
            if num_prefix_wg_1 > 1 {
                configs.push(SortConfig {
                    num_elements,
                    radix_pass,
                    num_sort_wg,
                    prefix_count: num_prefix_wg_0,
                    prefix_block_offset: block_offset_1,
                    prefix_data_offset: block_offset_0,
                    _pad0: 0,
                    _pad1: 0,
                });
            }
            // L0 add offsets
            configs.push(SortConfig {
                num_elements,
                radix_pass,
                num_sort_wg,
                prefix_count: hist_count,
                prefix_block_offset: block_offset_0,
                prefix_data_offset: 0,
                _pad0: 0,
                _pad1: 0,
            });
        }
    }

    /// Dispatch prefix sum compute passes, copying configs from staging.
    #[allow(clippy::too_many_arguments)]
    fn encode_prefix_sum_dispatches(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &BindGroup,
        config_buffer: &Buffer,
        staging: &Buffer,
        config_idx: &mut u64,
        config_size: u64,
        hist_count: u32,
    ) {
        let num_prefix_wg_0 = hist_count.div_ceil(WORKGROUP_SIZE);

        let copy_config = |enc: &mut wgpu::CommandEncoder, idx: &mut u64| {
            enc.copy_buffer_to_buffer(staging, *idx * config_size, config_buffer, 0, config_size);
            *idx += 1;
        };

        if num_prefix_wg_0 <= 1 {
            copy_config(encoder, config_idx);
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("histogram_scan_wg"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.scan_wg_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        } else if num_prefix_wg_0 <= WORKGROUP_SIZE {
            // Per-workgroup scan
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_wg"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_wg_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_0, 1, 1);
            }
            // Block scan
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_blocks"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_blocks_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            // Add offsets
            if num_prefix_wg_0 > 1 {
                copy_config(encoder, config_idx);
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_add_offsets"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_add_offsets_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_0, 1, 1);
            }
        } else {
            let num_prefix_wg_1 = num_prefix_wg_0.div_ceil(WORKGROUP_SIZE);

            // L0 wg scan
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_wg_l0"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_wg_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_0, 1, 1);
            }
            // L1 wg scan
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_wg_l1"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_wg_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_1, 1, 1);
            }
            // L1 block scan
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_blocks_l1"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_blocks_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            // L1 add offsets
            if num_prefix_wg_1 > 1 {
                copy_config(encoder, config_idx);
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_add_offsets_l1"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_add_offsets_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_1, 1, 1);
            }
            // L0 add offsets
            copy_config(encoder, config_idx);
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("histogram_scan_add_offsets_l0"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.scan_add_offsets_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(num_prefix_wg_0, 1, 1);
            }
        }
    }
}

/// Read back sorted instances from the GPU (for testing).
pub async fn read_sorted_instances(
    device: &Device,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    count: u32,
) -> GupResult<Vec<super::batch_renderer::InstanceAttributes>> {
    super::compute_instance_filter::ComputeInstanceFilter::read_output_instances(
        device, queue, buffer, count,
    )
    .await
}

/// Read back u32 values from a GPU buffer (for testing/diagnostics).
pub async fn read_u32_buffer(
    device: &Device,
    queue: &wgpu::Queue,
    buffer: &Buffer,
    count: u32,
) -> GupResult<Vec<u32>> {
    use wgpu::PollType;

    let read_size = count as u64 * 4;
    let buffer_size = buffer.size().min(read_size);

    let staging = device.create_buffer(&BufferDescriptor {
        label: Some("u32_staging"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("u32_readback"),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer_size);
    let sub_idx = queue.submit([encoder.finish()]);
    let _ = device.poll(PollType::WaitForSubmissionIndex(sub_idx));

    let slice = staging.slice(..);
    let (sender, receiver) = futures_channel::oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = sender.send(r);
    });
    let _ = device.poll(PollType::Wait);

    receiver
        .await
        .map_err(|_| GupError::render_error("u32 readback channel closed"))?
        .map_err(|e| GupError::render_error(format!("u32 buffer map failed: {e:?}")))?;

    let data = slice.get_mapped_range();
    let values: &[u32] = bytemuck::cast_slice(&data);
    let result = values.to_vec();
    drop(data);
    staging.unmap();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::GupContext;
    use crate::mark::batch_renderer::InstanceAttributes;

    use wgpu::PollType;

    /// Create a storage buffer and upload instance data.
    fn create_instance_buffer(device: &Device, instances: &[InstanceAttributes]) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_input"),
            size: std::mem::size_of_val(instances) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn upload_instances(queue: &wgpu::Queue, buffer: &Buffer, instances: &[InstanceAttributes]) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }

    /// Create an InstanceAttributes with a specific Z-depth.
    fn instance_with_z(x: f32, y: f32, z: f32, radius: f32, color: [f32; 4]) -> InstanceAttributes {
        let mut inst = InstanceAttributes::from_circle([x, y], radius, color);
        inst.transform[14] = z; // col 3, z component
        inst
    }

    /// Create a draw indirect buffer with the given visible count.
    fn create_draw_indirect(device: &Device, queue: &wgpu::Queue, visible_count: u32) -> Buffer {
        let buf = device.create_buffer(&BufferDescriptor {
            label: Some("test_draw_indirect"),
            size: 16,
            usage: BufferUsages::STORAGE
                | BufferUsages::INDIRECT
                | BufferUsages::COPY_SRC
                | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let data: [u32; 4] = [6, visible_count, 0, 0]; // vertex_count, instance_count, 0, 0
        queue.write_buffer(&buf, 0, bytemuck::cast_slice(&data));
        buf
    }

    // ------------------------------------------------------------------
    // Unit tests (no GPU)
    // ------------------------------------------------------------------

    #[test]
    fn test_sort_config_size() {
        let size = std::mem::size_of::<SortConfig>();
        assert_eq!(size, 32, "SortConfig should be 8 × u32 = 32 bytes");
        assert_eq!(size % 4, 0, "SortConfig size must be 4-byte aligned");
    }

    #[test]
    fn test_float_to_descending_key_ordering() {
        // Verify that the float-to-key conversion produces descending order.
        // Larger Z values should produce smaller keys.
        fn float_to_descending_key(f: f32) -> u32 {
            let bits = f.to_bits();
            let mask = if (bits & 0x80000000) != 0 {
                0xFFFFFFFF
            } else {
                0x80000000
            };
            !(bits ^ mask)
        }

        let far = float_to_descending_key(10.0);
        let mid = float_to_descending_key(5.0);
        let near = float_to_descending_key(1.0);
        let neg = float_to_descending_key(-1.0);

        // Back-to-front: far < mid < near < neg (in key space, ascending sort
        // puts far first).
        assert!(
            far < mid,
            "Far (10.0) should have smaller key than mid (5.0)"
        );
        assert!(
            mid < near,
            "Mid (5.0) should have smaller key than near (1.0)"
        );
        assert!(
            near < neg,
            "Near (1.0) should have smaller key than neg (-1.0)"
        );
    }

    // ------------------------------------------------------------------
    // GPU integration tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_radix_sorter_creation() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device);
        assert!(
            sorter.is_ok(),
            "Radix sorter pipeline creation should succeed"
        );
    }

    #[tokio::test]
    async fn test_sort_key_extraction() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        let instances = vec![
            instance_with_z(0.0, 0.0, 1.0, 0.1, [1.0, 0.0, 0.0, 1.0]),
            instance_with_z(0.1, 0.0, 10.0, 0.1, [0.0, 1.0, 0.0, 1.0]),
            instance_with_z(0.2, 0.0, -2.0, 0.1, [0.0, 0.0, 1.0, 1.0]),
            instance_with_z(0.3, 0.0, 5.0, 0.1, [1.0, 1.0, 0.0, 1.0]),
        ];

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        // Only run extract_sort_keys (not the full sort).
        let num_sort_wg = count.div_ceil(WORKGROUP_SIZE);
        let bind_group = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("test_extract_bg"),
            layout: &sorter.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: src_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: dst_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: sort_buffers.keys_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: sort_buffers.keys_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: sort_buffers.vals_a.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: sort_buffers.vals_b.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: sort_buffers.histograms.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: draw_indirect.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: sort_buffers.config.as_entire_binding(),
                },
            ],
        });

        let config = SortConfig {
            num_elements: count,
            radix_pass: 0,
            num_sort_wg,
            prefix_count: 0,
            prefix_block_offset: 0,
            prefix_data_offset: 0,
            _pad0: 0,
            _pad1: 0,
        };
        ctx.queue
            .write_buffer(&sort_buffers.config, 0, bytemuck::bytes_of(&config));

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_extract"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("extract"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&sorter.extract_keys_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_sort_wg, 1, 1);
        }

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        // Read back keys and vals.
        let keys = read_u32_buffer(&ctx.device, &ctx.queue, &sort_buffers.keys_a, count)
            .await
            .unwrap();
        let vals = read_u32_buffer(&ctx.device, &ctx.queue, &sort_buffers.vals_a, count)
            .await
            .unwrap();

        // Verify vals are [0, 1, 2, 3].
        assert_eq!(vals, vec![0, 1, 2, 3], "vals should be identity");

        // Verify keys are in descending order for [1.0, 10.0, -2.0, 5.0].
        // Expected: key(z=10) < key(z=5) < key(z=1) < key(z=-2)
        assert!(keys[1] < keys[3], "key(10) < key(5)");
        assert!(keys[3] < keys[0], "key(5) < key(1)");
        assert!(keys[0] < keys[2], "key(1) < key(-2)");
    }

    #[tokio::test]
    async fn test_sort_4_instances_by_z() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // 4 instances with different Z values. Expected back-to-front order:
        // z=10 (far), z=5, z=1, z=-2 (near).
        let instances = vec![
            instance_with_z(0.0, 0.0, 1.0, 0.1, [1.0, 0.0, 0.0, 1.0]), // near
            instance_with_z(0.1, 0.0, 10.0, 0.1, [0.0, 1.0, 0.0, 1.0]), // far
            instance_with_z(0.2, 0.0, -2.0, 0.1, [0.0, 0.0, 1.0, 1.0]), // nearest
            instance_with_z(0.3, 0.0, 5.0, 0.1, [1.0, 1.0, 0.0, 1.0]), // mid
        ];

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        // Read back sorted instances.
        let sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // Verify back-to-front order by Z (descending).
        let z_values: Vec<f32> = sorted.iter().map(|i| i.transform[14]).collect();

        for i in 0..z_values.len() - 1 {
            assert!(
                z_values[i] >= z_values[i + 1],
                "Z-values should be descending (back-to-front): {:?}",
                z_values
            );
        }

        // Verify the specific order: z=10, z=5, z=1, z=-2
        assert!(
            (z_values[0] - 10.0).abs() < 1e-5,
            "First should be z=10, got {}",
            z_values[0]
        );
        assert!(
            (z_values[1] - 5.0).abs() < 1e-5,
            "Second should be z=5, got {}",
            z_values[1]
        );
        assert!(
            (z_values[2] - 1.0).abs() < 1e-5,
            "Third should be z=1, got {}",
            z_values[2]
        );
        assert!(
            (z_values[3] - (-2.0)).abs() < 1e-5,
            "Fourth should be z=-2, got {}",
            z_values[3]
        );
    }

    #[tokio::test]
    async fn test_sort_matches_cpu_reference() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // 32 instances with various Z values.
        let instances: Vec<InstanceAttributes> = (0..32)
            .map(|i| {
                let t = i as f32 / 32.0;
                let z = (t * 7.0).sin() * 10.0;
                instance_with_z(t * 0.5, 0.0, z, 0.05, [t, 1.0 - t, 0.5, 1.0])
            })
            .collect();

        // CPU reference: sort by Z descending.
        let mut cpu_sorted = instances.clone();
        cpu_sorted.sort_by(|a, b| b.transform[14].partial_cmp(&a.transform[14]).unwrap());

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let gpu_sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // Compare Z values.
        for (i, (gpu, cpu)) in gpu_sorted.iter().zip(cpu_sorted.iter()).enumerate() {
            assert!(
                (gpu.transform[14] - cpu.transform[14]).abs() < 1e-5,
                "Instance {i}: GPU z={} != CPU z={}",
                gpu.transform[14],
                cpu.transform[14]
            );
        }
    }

    #[tokio::test]
    async fn test_sort_preserves_instance_data() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // Each instance has a unique color to verify data integrity.
        let instances = vec![
            instance_with_z(0.0, 0.0, 3.0, 0.1, [1.0, 0.0, 0.0, 1.0]),
            instance_with_z(0.0, 0.0, 1.0, 0.1, [0.0, 1.0, 0.0, 1.0]),
            instance_with_z(0.0, 0.0, 2.0, 0.1, [0.0, 0.0, 1.0, 1.0]),
        ];

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // Expected order: z=3 (red), z=2 (blue), z=1 (green)
        assert_eq!(
            sorted[0].color,
            [1.0, 0.0, 0.0, 1.0],
            "First should be red (z=3)"
        );
        assert_eq!(
            sorted[1].color,
            [0.0, 0.0, 1.0, 1.0],
            "Second should be blue (z=2)"
        );
        assert_eq!(
            sorted[2].color,
            [0.0, 1.0, 0.0, 1.0],
            "Third should be green (z=1)"
        );
    }

    #[tokio::test]
    async fn test_sort_already_sorted() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // Already in back-to-front order.
        let instances = vec![
            instance_with_z(0.0, 0.0, 5.0, 0.1, [1.0, 0.0, 0.0, 1.0]),
            instance_with_z(0.0, 0.0, 3.0, 0.1, [0.0, 1.0, 0.0, 1.0]),
            instance_with_z(0.0, 0.0, 1.0, 0.1, [0.0, 0.0, 1.0, 1.0]),
        ];

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // Should maintain original order.
        assert_eq!(sorted[0].color, [1.0, 0.0, 0.0, 1.0], "First red (z=5)");
        assert_eq!(sorted[1].color, [0.0, 1.0, 0.0, 1.0], "Second green (z=3)");
        assert_eq!(sorted[2].color, [0.0, 0.0, 1.0, 1.0], "Third blue (z=1)");
    }

    #[tokio::test]
    async fn test_sort_512_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // 512 instances (2 workgroups) with pseudo-random Z values.
        let count = 512u32;
        let instances: Vec<InstanceAttributes> = (0..count)
            .map(|i| {
                let t = i as f32 / count as f32;
                let z = (t * 37.0).sin() * 100.0;
                instance_with_z(t * 0.5, 0.0, z, 0.05, [t, 1.0 - t, 0.5, 1.0])
            })
            .collect();

        // CPU reference.
        let mut cpu_sorted = instances.clone();
        cpu_sorted.sort_by(|a, b| b.transform[14].partial_cmp(&a.transform[14]).unwrap());

        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let gpu_sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // Compare Z order.
        for (i, (gpu, cpu)) in gpu_sorted.iter().zip(cpu_sorted.iter()).enumerate() {
            assert!(
                (gpu.transform[14] - cpu.transform[14]).abs() < 1e-3,
                "Instance {i}: GPU z={} != CPU z={}",
                gpu.transform[14],
                cpu.transform[14]
            );
        }
    }

    #[tokio::test]
    async fn test_sort_single_instance() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        let instances = vec![instance_with_z(0.0, 0.0, 5.0, 0.1, [1.0, 0.0, 0.0, 1.0])];

        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;
        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, 1);
        let sort_buffers = SortBuffers::new(&ctx.device, 1);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            1,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, 1)
            .await
            .unwrap();

        assert_eq!(sorted[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert!((sorted[0].transform[14] - 5.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_sort_equal_z_values() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let sorter = RadixSorter::new(&ctx.device).unwrap();

        // All instances at the same Z — should not crash or reorder.
        let instances: Vec<InstanceAttributes> = (0..8)
            .map(|i| {
                let t = i as f32 / 8.0;
                instance_with_z(t, 0.0, 5.0, 0.1, [t, 1.0 - t, 0.5, 1.0])
            })
            .collect();

        let count = instances.len() as u32;
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;

        let src_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &src_buf, &instances);

        let dst_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("test_sort_dst"),
            size: count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect = create_draw_indirect(&ctx.device, &ctx.queue, count);
        let sort_buffers = SortBuffers::new(&ctx.device, count);

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("test_sort_encoder"),
            });

        sorter.encode_sort(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &src_buf,
            &dst_buf,
            &draw_indirect,
            &sort_buffers,
            count,
        );

        let sub_idx = ctx.queue.submit([encoder.finish()]);
        let _ = ctx.device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let sorted = read_sorted_instances(&ctx.device, &ctx.queue, &dst_buf, count)
            .await
            .unwrap();

        // All should have z=5.
        for (i, inst) in sorted.iter().enumerate() {
            assert!(
                (inst.transform[14] - 5.0).abs() < 1e-5,
                "Instance {i}: z={}, expected 5.0",
                inst.transform[14]
            );
        }
    }
}
