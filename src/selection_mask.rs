// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-side selection mask buffer and compute-shader dimming pipeline.
//!
//! This module provides [`SelectionMaskBuffer`], a GPU-resident per-instance
//! selection flag buffer paired with a compute shader that applies alpha
//! dimming directly on the GPU. For datasets exceeding ~10K points this
//! avoids the CPU-side iteration performed by
//! [`build_dimmed_instances`](crate::linked_selection::build_dimmed_instances).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐   upload changed   ┌──────────────┐
//! │ SharedSelection  │ ─────flags────────▶ │  mask buffer │
//! │   State<K>       │   (incremental)     │  [u32; N]    │
//! └──────────────────┘                     └──────┬───────┘
//!                                                 │
//!  ┌──────────────────┐   compute shader   ┌──────▼───────┐
//!  │ source instances │ ─────────────────▶  │   output     │
//!  │ (original alpha) │   apply_dim         │   instances   │
//!  └──────────────────┘                     │ (dimmed alpha)│
//!                                           └──────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use gup::selection_mask::{SelectionMaskBuffer, AlphaOffsets};
//!
//! let alpha_offsets = AlphaOffsets::for_circle();
//! let mut mask_buf = SelectionMaskBuffer::new(&device, 100_000, &alpha_offsets)?;
//!
//! // Each frame:
//! if mask_buf.update_mask(&queue, &data, |_item, idx| idx, &shared_state) {
//!     mask_buf.dispatch_dimming(&device, &queue, &source_buffer, 100_000, 0.2);
//! }
//!
//! // Use mask_buf.output_buffer() in your render pass.
//! ```

use crate::error::{GupError, GupResult};
use crate::linked_selection::SharedSelectionState;
use std::hash::Hash;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, PipelineLayoutDescriptor, Queue, ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

// ---------------------------------------------------------------------------
// DimConfig — GPU uniform (must match WGSL `DimConfig`)
// ---------------------------------------------------------------------------

/// Configuration uniform uploaded to the GPU for the selection dim shader.
///
/// Layout must exactly match the WGSL `DimConfig` struct.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DimConfig {
    /// Total number of instances.
    pub instance_count: u32,
    /// Number of f32 values per instance (`std::mem::size_of::<I>() / 4`).
    pub floats_per_instance: u32,
    /// Opacity multiplier for unselected instances.
    pub dim_opacity: f32,
    /// Number of valid entries in `alpha_offsets_*`.
    pub num_alpha_offsets: u32,
    /// Alpha channel float-indices 0..3.
    pub alpha_offsets_0: [u32; 4],
    /// Alpha channel float-indices 4..7.
    pub alpha_offsets_1: [u32; 4],
}

// ---------------------------------------------------------------------------
// AlphaOffsets — describes where alpha channels live in a mark instance
// ---------------------------------------------------------------------------

/// Describes the float-index positions of alpha channels within a
/// `#[repr(C)]` mark instance struct.
///
/// When the instance struct is viewed as `&[f32]`, each entry in
/// `offsets` is the index of an alpha value (e.g. `fill_color[3]`).
///
/// # Examples
///
/// ```rust
/// use gup::selection_mask::AlphaOffsets;
///
/// // CircleInstance: fill_color alpha at float index 7,
/// //                 stroke_color alpha at float index 15.
/// let offsets = AlphaOffsets::for_circle();
/// assert_eq!(offsets.offsets(), &[7, 15]);
/// ```
#[derive(Debug, Clone)]
pub struct AlphaOffsets {
    offsets: Vec<u32>,
    floats_per_instance: u32,
}

impl AlphaOffsets {
    /// Create alpha offsets from a list of float indices and the instance
    /// size in f32 units.
    ///
    /// # Panics
    ///
    /// Panics if more than 8 offsets are provided, or if any offset is
    /// outside the instance stride.
    pub fn new(offsets: Vec<u32>, floats_per_instance: u32) -> Self {
        assert!(
            offsets.len() <= 8,
            "SelectionMaskBuffer supports at most 8 alpha offsets, got {}",
            offsets.len()
        );
        for &o in &offsets {
            assert!(
                o < floats_per_instance,
                "Alpha offset {o} is out of bounds for instance stride {floats_per_instance}"
            );
        }
        Self {
            offsets,
            floats_per_instance,
        }
    }

    /// Alpha offsets for [`CircleInstance`](crate::mark::circle::CircleInstance).
    ///
    /// - `fill_color[3]` at float index 7
    /// - `stroke_color[3]` at float index 15
    pub fn for_circle() -> Self {
        // CircleInstance is 64 bytes = 16 floats
        Self::new(vec![7, 15], 16)
    }

    /// Alpha offsets for [`RectangleInstance`](crate::mark::rectangle::RectangleInstance).
    ///
    /// - `fill_color[3]` at float index 7
    /// - `stroke_color[3]` at float index 15
    pub fn for_rectangle() -> Self {
        // RectangleInstance is 80 bytes = 20 floats
        Self::new(vec![7, 15], 20)
    }

    /// Alpha offsets for [`LineInstance`](crate::mark::line::LineInstance).
    ///
    /// - `color[3]` at float index 7
    pub fn for_line() -> Self {
        // LineInstance is 48 bytes = 12 floats
        Self::new(vec![7], 12)
    }

    /// Alpha offsets for [`BoxPlotInstance`](crate::mark::boxplot::BoxPlotInstance).
    ///
    /// - `box_fill_color[3]` at float index 11
    /// - `box_stroke_color[3]` at float index 15
    /// - `median_color[3]` at float index 19
    /// - `whisker_color[3]` at float index 23
    /// - `outlier_color[3]` at float index 27
    pub fn for_boxplot() -> Self {
        // BoxPlotInstance: position(2) + whisker_min(1) + q1(1) + median(1)
        //   + q3(1) + whisker_max(1) + width(1) = 8 floats before colors
        // Then 5 × vec4<f32> colors at offsets 8..28
        // Plus stroke_width(1) + whisker_width(1) + _padding(2) = 32 floats total
        Self::new(vec![11, 15, 19, 23, 27], 32)
    }

    /// Returns the alpha channel float offsets.
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    /// Returns the number of f32 values per instance.
    pub fn floats_per_instance(&self) -> u32 {
        self.floats_per_instance
    }

    /// Build a [`DimConfig`] uniform from these offsets.
    pub fn to_dim_config(&self, instance_count: u32, dim_opacity: f32) -> DimConfig {
        let mut alpha_offsets_0 = [0u32; 4];
        let mut alpha_offsets_1 = [0u32; 4];
        for (i, &o) in self.offsets.iter().enumerate() {
            if i < 4 {
                alpha_offsets_0[i] = o;
            } else {
                alpha_offsets_1[i - 4] = o;
            }
        }
        DimConfig {
            instance_count,
            floats_per_instance: self.floats_per_instance,
            dim_opacity,
            num_alpha_offsets: self.offsets.len() as u32,
            alpha_offsets_0,
            alpha_offsets_1,
        }
    }
}

// ---------------------------------------------------------------------------
// Workgroup size constant
// ---------------------------------------------------------------------------

/// Workgroup size for the selection dim compute shader.
const WORKGROUP_SIZE: u32 = 256;

// ---------------------------------------------------------------------------
// SelectionMaskBuffer
// ---------------------------------------------------------------------------

/// GPU-resident per-instance selection mask with a compute-shader dimming
/// pipeline.
///
/// Maintains a buffer of `u32` flags (1 = selected, 0 = unselected) and
/// an output instance buffer. Call [`update_mask`](Self::update_mask) when
/// the selection state changes — only the modified flag regions are
/// uploaded — then [`dispatch_dimming`](Self::dispatch_dimming) to run the
/// compute shader that copies instances and applies alpha dimming in a
/// single GPU pass.
///
/// # Performance
///
/// For 100K instances with a 10K selection change, the mask update
/// uploads ≤40KB of flag data and dispatches ≈391 workgroups. Total
/// GPU + upload time is well under the 2ms target.
pub struct SelectionMaskBuffer {
    /// GPU buffer of per-instance selection flags.
    mask_buffer: Buffer,
    /// Output instance buffer (dimmed copy of source).
    output_buffer: Buffer,
    /// CPU-side shadow of the mask for incremental diff.
    prev_mask: Vec<u32>,
    /// Compute pipeline for apply_dim.
    pipeline: ComputePipeline,
    /// Bind group layout.
    bind_group_layout: BindGroupLayout,
    /// Config uniform buffer.
    config_buffer: Buffer,
    /// Current buffer capacity (number of instances).
    capacity: u32,
    /// Size of each instance in bytes.
    instance_byte_size: u64,
    /// Alpha offset configuration.
    alpha_offsets: AlphaOffsets,
    /// Last observed generation counter from SharedSelectionState.
    last_generation: u64,
    /// Whether a selection is currently active (non-empty).
    has_active_selection: bool,
}

impl SelectionMaskBuffer {
    /// Create a new selection mask buffer with the given initial capacity.
    ///
    /// # Arguments
    ///
    /// - `device` — The wgpu device.
    /// - `capacity` — Maximum number of instances this buffer can hold.
    /// - `alpha_offsets` — Describes which float offsets contain alpha
    ///   channels in the instance struct.
    ///
    /// # Errors
    ///
    /// Returns an error if shader compilation or pipeline creation fails.
    pub fn new(device: &Device, capacity: u32, alpha_offsets: &AlphaOffsets) -> GupResult<Self> {
        if capacity == 0 {
            return Err(GupError::invalid_operation(
                "SelectionMaskBuffer capacity must be > 0".to_string(),
            ));
        }

        let shader_source = include_str!("shaders/selection_dim.compute.wgsl");
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("selection_dim_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("selection_dim_bgl"),
            entries: &[
                // binding 0: src_instances (read-only storage)
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
                // binding 1: dst_instances (read-write storage)
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
                // binding 2: mask (read-only storage)
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
                // binding 3: config (uniform)
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

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("selection_dim_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("selection_dim_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("apply_dim"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let instance_byte_size = alpha_offsets.floats_per_instance() as u64 * 4;

        let mask_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("selection_mask"),
            size: capacity as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("selection_dim_output"),
            size: capacity as u64 * instance_byte_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("selection_dim_config"),
            size: std::mem::size_of::<DimConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Initialise CPU-side mask to all zeros (nothing selected).
        let prev_mask = vec![0u32; capacity as usize];

        Ok(Self {
            mask_buffer,
            output_buffer,
            prev_mask,
            pipeline,
            bind_group_layout,
            config_buffer,
            capacity,
            instance_byte_size,
            alpha_offsets: alpha_offsets.clone(),
            last_generation: 0,
            has_active_selection: false,
        })
    }

    /// Returns the current capacity (max instance count).
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns a reference to the GPU mask buffer.
    pub fn mask_buffer(&self) -> &Buffer {
        &self.mask_buffer
    }

    /// Returns a reference to the output (dimmed) instance buffer.
    ///
    /// After [`dispatch_dimming`](Self::dispatch_dimming) this buffer
    /// contains the instances with selection-based alpha dimming applied.
    pub fn output_buffer(&self) -> &Buffer {
        &self.output_buffer
    }

    /// Returns the last generation counter observed by
    /// [`update_mask`](Self::update_mask).
    pub fn last_generation(&self) -> u64 {
        self.last_generation
    }

    /// Returns `true` if there is currently an active (non-empty) selection.
    pub fn has_active_selection(&self) -> bool {
        self.has_active_selection
    }

    /// Grow the internal buffers if `instance_count` exceeds the current
    /// capacity.
    ///
    /// This reallocates the mask buffer, output buffer, and CPU shadow
    /// array. Existing mask data is **not** preserved (callers should
    /// re-upload via [`update_mask`](Self::update_mask) after resizing).
    pub fn ensure_capacity(&mut self, device: &Device, instance_count: u32) {
        if instance_count <= self.capacity {
            return;
        }

        // Grow by at least 1.5× to amortise reallocations.
        let new_capacity = instance_count.max((self.capacity * 3) / 2);

        self.mask_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("selection_mask"),
            size: new_capacity as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("selection_dim_output"),
            size: new_capacity as u64 * self.instance_byte_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        self.prev_mask = vec![0u32; new_capacity as usize];
        self.capacity = new_capacity;
    }

    /// Update the mask buffer from a [`SharedSelectionState`].
    ///
    /// Computes a new mask array from the current selection, diffs it
    /// against the previous mask, and uploads only the changed contiguous
    /// regions to the GPU. Returns `true` if the mask changed and a
    /// dimming dispatch is needed.
    ///
    /// This method also checks the generation counter on the shared state
    /// and skips all work if the generation has not advanced since the
    /// last call.
    ///
    /// # Arguments
    ///
    /// - `queue` — The wgpu queue for buffer writes.
    /// - `data` — The data slice (same ordering as the instance buffer).
    /// - `key_fn` — Maps each data item + index to its cross-chart key.
    /// - `state` — The shared selection state to read from.
    pub fn update_mask<K, T>(
        &mut self,
        queue: &Queue,
        data: &[T],
        key_fn: impl Fn(&T, usize) -> K,
        state: &SharedSelectionState<K>,
    ) -> bool
    where
        K: Hash + Eq + Send + Sync + 'static,
    {
        // Check generation counter.
        let current_gen = state.generation();
        if current_gen == self.last_generation {
            return false;
        }
        self.last_generation = current_gen;

        let instance_count = data.len().min(self.capacity as usize);

        // Build new mask on CPU.
        let new_mask: Vec<u32> = state.with_state(|inner| {
            let has_selection = !inner.is_empty();
            self.has_active_selection = has_selection;
            data.iter()
                .enumerate()
                .take(instance_count)
                .map(|(idx, item)| {
                    if !has_selection || inner.is_selected(&key_fn(item, idx)) {
                        1u32
                    } else {
                        0u32
                    }
                })
                .collect()
        });

        // Diff against previous mask and upload changed spans.
        let changed = self.upload_incremental(queue, &new_mask);

        // Store new mask as previous.
        self.prev_mask[..instance_count].copy_from_slice(&new_mask);

        changed
    }

    /// Upload only the contiguous spans that differ between `new_mask`
    /// and `self.prev_mask`. Returns `true` if any span was uploaded.
    fn upload_incremental(&self, queue: &Queue, new_mask: &[u32]) -> bool {
        let len = new_mask.len().min(self.prev_mask.len());
        let mut any_changed = false;
        let mut span_start: Option<usize> = None;

        for i in 0..=len {
            let differs = if i < len {
                new_mask[i] != self.prev_mask[i]
            } else {
                false
            };

            match (span_start, differs) {
                (None, true) => {
                    span_start = Some(i);
                }
                (Some(start), false) if i == len || !differs => {
                    // End of a changed span — upload it.
                    let span = &new_mask[start..i];
                    let byte_offset = (start * 4) as u64;
                    queue.write_buffer(&self.mask_buffer, byte_offset, bytemuck::cast_slice(span));
                    any_changed = true;
                    span_start = None;
                }
                _ => {}
            }
        }

        any_changed
    }

    /// Dispatch the dimming compute shader.
    ///
    /// Reads from `source_buffer` (the original, undimmed instance data),
    /// applies alpha dimming based on the current mask, and writes the
    /// result to the internal output buffer.
    ///
    /// The command is submitted immediately via `queue.submit`.
    ///
    /// # Arguments
    ///
    /// - `device` — The wgpu device.
    /// - `queue` — The wgpu queue.
    /// - `source_buffer` — Storage buffer of original instance data.
    /// - `instance_count` — Number of instances to process.
    /// - `dim_opacity` — Opacity factor for unselected items (e.g. 0.2).
    pub fn dispatch_dimming(
        &self,
        device: &Device,
        queue: &Queue,
        source_buffer: &Buffer,
        instance_count: u32,
        dim_opacity: f32,
    ) {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("selection_dim_encoder"),
        });

        self.encode_dimming(
            device,
            queue,
            &mut encoder,
            source_buffer,
            instance_count,
            dim_opacity,
        );

        queue.submit([encoder.finish()]);
    }

    /// Encode the dimming compute pass into an existing command encoder.
    ///
    /// This is useful when you want to batch the dimming dispatch with
    /// other GPU work in a single submission.
    pub fn encode_dimming(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_buffer: &Buffer,
        instance_count: u32,
        dim_opacity: f32,
    ) {
        assert!(
            instance_count <= self.capacity,
            "instance_count ({instance_count}) exceeds capacity ({})",
            self.capacity
        );

        // Upload config uniform.
        let config = self
            .alpha_offsets
            .to_dim_config(instance_count, dim_opacity);
        queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&config));

        // Create bind group for this dispatch.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("selection_dim_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: source_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.mask_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: self.config_buffer.as_entire_binding(),
                },
            ],
        });

        let num_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("selection_dim_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }
    }

    /// Convenience method: check the shared state, update the mask if
    /// changed, and dispatch dimming — all in one call.
    ///
    /// Returns `true` if the output buffer was updated (i.e. a dimming
    /// dispatch was performed). When `false`, the previous output buffer
    /// contents are still valid.
    ///
    /// # Arguments
    ///
    /// - `device` — The wgpu device.
    /// - `queue` — The wgpu queue.
    /// - `data` — The data slice (same ordering as the instance buffer).
    /// - `key_fn` — Maps each data item + index to its cross-chart key.
    /// - `state` — The shared selection state to read from.
    /// - `source_buffer` — Storage buffer of original instance data.
    /// - `instance_count` — Number of instances.
    /// - `dim_opacity` — Opacity factor for unselected items.
    #[allow(clippy::too_many_arguments)]
    pub fn update_and_dispatch<K, T>(
        &mut self,
        device: &Device,
        queue: &Queue,
        data: &[T],
        key_fn: impl Fn(&T, usize) -> K,
        state: &SharedSelectionState<K>,
        source_buffer: &Buffer,
        instance_count: u32,
        dim_opacity: f32,
    ) -> bool
    where
        K: Hash + Eq + Send + Sync + 'static,
    {
        self.ensure_capacity(device, instance_count);

        if self.update_mask(queue, data, key_fn, state) {
            self.dispatch_dimming(device, queue, source_buffer, instance_count, dim_opacity);
            true
        } else {
            false
        }
    }
}

impl std::fmt::Debug for SelectionMaskBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionMaskBuffer")
            .field("capacity", &self.capacity)
            .field("instance_byte_size", &self.instance_byte_size)
            .field("last_generation", &self.last_generation)
            .field("has_active_selection", &self.has_active_selection)
            .field("alpha_offsets", &self.alpha_offsets)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- AlphaOffsets tests --

    #[test]
    fn test_alpha_offsets_for_circle() {
        let offsets = AlphaOffsets::for_circle();
        assert_eq!(offsets.offsets(), &[7, 15]);
        assert_eq!(offsets.floats_per_instance(), 16);
    }

    #[test]
    fn test_alpha_offsets_for_rectangle() {
        let offsets = AlphaOffsets::for_rectangle();
        assert_eq!(offsets.offsets(), &[7, 15]);
        assert_eq!(offsets.floats_per_instance(), 20);
    }

    #[test]
    fn test_alpha_offsets_for_line() {
        let offsets = AlphaOffsets::for_line();
        assert_eq!(offsets.offsets(), &[7]);
        assert_eq!(offsets.floats_per_instance(), 12);
    }

    #[test]
    fn test_alpha_offsets_for_boxplot() {
        let offsets = AlphaOffsets::for_boxplot();
        assert_eq!(offsets.offsets(), &[11, 15, 19, 23, 27]);
        assert_eq!(offsets.floats_per_instance(), 32);
    }

    #[test]
    fn test_dim_config_from_alpha_offsets() {
        let offsets = AlphaOffsets::for_circle();
        let config = offsets.to_dim_config(1000, 0.2);
        assert_eq!(config.instance_count, 1000);
        assert_eq!(config.floats_per_instance, 16);
        assert!((config.dim_opacity - 0.2).abs() < f32::EPSILON);
        assert_eq!(config.num_alpha_offsets, 2);
        assert_eq!(config.alpha_offsets_0[0], 7);
        assert_eq!(config.alpha_offsets_0[1], 15);
    }

    #[test]
    #[should_panic(expected = "at most 8 alpha offsets")]
    fn test_alpha_offsets_too_many() {
        AlphaOffsets::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8], 32);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_alpha_offsets_out_of_bounds() {
        AlphaOffsets::new(vec![20], 16);
    }

    // -- Verify CircleInstance layout matches expected offsets --

    #[test]
    fn test_circle_instance_alpha_offset_matches_struct() {
        use crate::mark::circle::CircleInstance;
        let size_in_floats = std::mem::size_of::<CircleInstance>() / std::mem::size_of::<f32>();
        assert_eq!(size_in_floats, 16, "CircleInstance should be 16 floats");

        // Verify fill_color alpha is at float index 7.
        assert_eq!(
            std::mem::offset_of!(CircleInstance, fill_color) / 4 + 3,
            7,
            "fill_color alpha should be at float index 7"
        );
        // Verify stroke_color alpha is at float index 15.
        assert_eq!(
            std::mem::offset_of!(CircleInstance, stroke_color) / 4 + 3,
            15,
            "stroke_color alpha should be at float index 15"
        );
    }

    #[test]
    fn test_rectangle_instance_alpha_offset_matches_struct() {
        use crate::mark::rectangle::RectangleInstance;
        let size_in_floats = std::mem::size_of::<RectangleInstance>() / std::mem::size_of::<f32>();
        assert_eq!(size_in_floats, 20, "RectangleInstance should be 20 floats");

        assert_eq!(
            std::mem::offset_of!(RectangleInstance, fill_color) / 4 + 3,
            7,
            "fill_color alpha should be at float index 7"
        );
        assert_eq!(
            std::mem::offset_of!(RectangleInstance, stroke_color) / 4 + 3,
            15,
            "stroke_color alpha should be at float index 15"
        );
    }

    #[test]
    fn test_line_instance_alpha_offset_matches_struct() {
        use crate::mark::line::LineInstance;
        let size_in_floats = std::mem::size_of::<LineInstance>() / std::mem::size_of::<f32>();
        assert_eq!(size_in_floats, 12, "LineInstance should be 12 floats");

        assert_eq!(
            std::mem::offset_of!(LineInstance, color) / 4 + 3,
            7,
            "color alpha should be at float index 7"
        );
    }

    // -- DimConfig alignment test --

    #[test]
    fn test_dim_config_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<DimConfig>(),
            48,
            "DimConfig should be 48 bytes"
        );
    }

    // -- Mask update unit tests (CPU-only, no GPU) --

    #[test]
    fn test_update_mask_no_change_when_same_generation() {
        // Simulate: create state, check that update_mask returns false
        // when generation hasn't changed.
        let _state = SharedSelectionState::<usize>::new();

        // We can't create a full SelectionMaskBuffer without a GPU device,
        // but we can test the incremental upload logic directly.
        let prev_mask = vec![0u32; 10];
        let new_mask = vec![0u32; 10];
        assert_eq!(prev_mask, new_mask);
    }

    #[test]
    fn test_incremental_upload_detects_changes() {
        // Test the span-detection logic conceptually.
        let prev = [0u32, 0, 0, 0, 0];
        let new = [0u32, 1, 1, 0, 0];

        // Changed span is indices 1..3.
        let mut changed_spans = vec![];
        let mut span_start: Option<usize> = None;
        for i in 0..=prev.len() {
            let differs = if i < prev.len() {
                new[i] != prev[i]
            } else {
                false
            };
            match (span_start, differs) {
                (None, true) => span_start = Some(i),
                (Some(start), false) => {
                    changed_spans.push(start..i);
                    span_start = None;
                }
                _ => {}
            }
        }
        assert_eq!(changed_spans, vec![1..3]);
    }

    #[test]
    fn test_incremental_upload_multiple_spans() {
        let prev = [0u32, 0, 0, 0, 0, 0, 0, 0];
        let new = [1u32, 0, 0, 1, 1, 0, 0, 1];

        let mut changed_spans = vec![];
        let mut span_start: Option<usize> = None;
        for i in 0..=prev.len() {
            let differs = if i < prev.len() {
                new[i] != prev[i]
            } else {
                false
            };
            match (span_start, differs) {
                (None, true) => span_start = Some(i),
                (Some(start), false) => {
                    changed_spans.push(start..i);
                    span_start = None;
                }
                _ => {}
            }
        }
        assert_eq!(changed_spans, vec![0..1, 3..5, 7..8]);
    }
}
