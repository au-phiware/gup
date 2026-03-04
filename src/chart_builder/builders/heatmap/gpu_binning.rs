// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute-shader backend for 2D heatmap binning.
//!
//! This module provides [`GpuBinner`], which offloads the binning work
//! performed by [`super::binning::BinGrid::from_data`] to a wgpu compute
//! shader.  The GPU path uses atomic operations to bin records in parallel
//! and reads the results back into a [`BinGrid`] that is fully compatible
//! with the existing rendering pipeline.
//!
//! ## Automatic CPU fallback
//!
//! [`gpu_bin_data`] detects when compute shaders are unavailable (e.g.
//! when no suitable adapter is found) and transparently falls back to the
//! CPU implementation.

use super::HeatmapCell;
use super::binning::{AggregateFunc, BinGrid, BinSpec};
use crate::error::{GupError, GupResult};
use crate::render::RenderContext;
use wgpu::util::DeviceExt;
use wgpu::*;

/// WGSL source for the 2D binning compute shader.
const BINNING_SHADER: &str = include_str!("../../../shaders/heatmap_binning.compute.wgsl");

/// Workgroup size — must match the `@workgroup_size` in the WGSL source.
const WORKGROUP_SIZE: u32 = 256;

// ── GPU-side uniform config ──────────────────────────────────────────────

/// Mirror of the WGSL `BinConfig` struct.
///
/// Layout must match exactly: 8 × u32/f32 = 32 bytes, `std140` uniform.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuBinConfig {
    x_bins: u32,
    y_bins: u32,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    data_length: u32,
    _padding: u32,
}

// ── GpuBinner ────────────────────────────────────────────────────────────

/// Reusable GPU binner that caches the compute pipeline.
///
/// Create once per [`RenderContext`] and reuse for multiple binning calls.
#[derive(Debug)]
pub struct GpuBinner {
    device: Device,
    queue: Queue,
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl GpuBinner {
    /// Compile the binning compute pipeline.
    pub fn new(context: &RenderContext) -> GupResult<Self> {
        let device = context.device();
        let queue = context.queue();

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("heatmap_binning_shader"),
            source: ShaderSource::Wgsl(BINNING_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("heatmap_binning_bgl"),
            entries: &[
                // binding 0: x_data (storage, read-only)
                bgl_entry(0, BufferBindingType::Storage { read_only: true }),
                // binding 1: y_data
                bgl_entry(1, BufferBindingType::Storage { read_only: true }),
                // binding 2: fill_data
                bgl_entry(2, BufferBindingType::Storage { read_only: true }),
                // binding 3: config (uniform)
                bgl_entry(3, BufferBindingType::Uniform),
                // binding 4: out_count (storage, read-write)
                bgl_entry(4, BufferBindingType::Storage { read_only: false }),
                // binding 5: out_sum
                bgl_entry(5, BufferBindingType::Storage { read_only: false }),
                // binding 6: out_min
                bgl_entry(6, BufferBindingType::Storage { read_only: false }),
                // binding 7: out_max
                bgl_entry(7, BufferBindingType::Storage { read_only: false }),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("heatmap_binning_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("heatmap_binning_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("bin_data"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            bind_group_layout,
        })
    }

    /// Run the GPU binning pass and return a [`BinGrid`].
    ///
    /// All three value slices must have the same length.
    #[allow(clippy::too_many_arguments)]
    pub async fn bin(
        &self,
        x_values: &[f32],
        y_values: &[f32],
        fill_values: &[f32],
        x_spec: BinSpec,
        y_spec: BinSpec,
        func: AggregateFunc,
        no_data: f32,
    ) -> GupResult<BinGrid> {
        let n = x_values.len();
        if n != y_values.len() || n != fill_values.len() {
            return Err(GupError::validation_error(
                "x_values, y_values and fill_values must have the same length",
            ));
        }

        let n_cells = x_spec.bins * y_spec.bins;
        if n_cells == 0 {
            return Ok(BinGrid {
                cells: Vec::new(),
                x_spec,
                y_spec,
            });
        }

        // Empty data → return grid filled with no_data (no GPU dispatch).
        if n == 0 {
            let cells = (0..n_cells)
                .map(|i| HeatmapCell {
                    x_index: (i % x_spec.bins) as u32,
                    y_index: (i / x_spec.bins) as u32,
                    value: no_data,
                })
                .collect();
            return Ok(BinGrid {
                cells,
                x_spec,
                y_spec,
            });
        }

        // ── Upload input data ────────────────────────────────────────

        let x_buf = self.storage_buffer("x_data", bytemuck::cast_slice(x_values));
        let y_buf = self.storage_buffer("y_data", bytemuck::cast_slice(y_values));
        let f_buf = self.storage_buffer("fill_data", bytemuck::cast_slice(fill_values));

        let config = GpuBinConfig {
            x_bins: x_spec.bins as u32,
            y_bins: y_spec.bins as u32,
            x_min: x_spec.min,
            x_max: x_spec.max,
            y_min: y_spec.min,
            y_max: y_spec.max,
            data_length: n as u32,
            _padding: 0,
        };
        let config_buf = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("heatmap_binning_config"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM,
        });

        // ── Create output buffers (zeroed) ───────────────────────────

        let cell_bytes = (n_cells * std::mem::size_of::<u32>()) as u64;

        let count_buf = self.zeroed_storage_buffer("out_count", cell_bytes);
        let sum_buf = self.zeroed_storage_buffer("out_sum", cell_bytes);

        // Min buffer initialised to +Inf (0x7F800000).
        let min_buf = self.filled_storage_buffer("out_min", n_cells, f32::INFINITY.to_bits());
        // Max buffer initialised to -Inf (0xFF800000).
        let max_buf = self.filled_storage_buffer("out_max", n_cells, f32::NEG_INFINITY.to_bits());

        // ── Bind group ───────────────────────────────────────────────

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("heatmap_binning_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                buf_entry(0, &x_buf),
                buf_entry(1, &y_buf),
                buf_entry(2, &f_buf),
                buf_entry(3, &config_buf),
                buf_entry(4, &count_buf),
                buf_entry(5, &sum_buf),
                buf_entry(6, &min_buf),
                buf_entry(7, &max_buf),
            ],
        });

        // ── Dispatch ─────────────────────────────────────────────────

        let workgroups = (n as u32).div_ceil(WORKGROUP_SIZE);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("heatmap_binning_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("heatmap_binning_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // ── Readback ─────────────────────────────────────────────────

        // We only need to read back the buffers required by `func`.
        // For Mean we need both count and sum.
        // We always read count to distinguish empty cells for no_data.

        let need_sum = matches!(func, AggregateFunc::Sum | AggregateFunc::Mean);
        let need_min = matches!(func, AggregateFunc::Min);
        let need_max = matches!(func, AggregateFunc::Max);

        let staging_count = Some(self.staging_readback(&mut encoder, &count_buf, cell_bytes));
        let staging_sum = if need_sum {
            Some(self.staging_readback(&mut encoder, &sum_buf, cell_bytes))
        } else {
            None
        };
        let staging_min = if need_min {
            Some(self.staging_readback(&mut encoder, &min_buf, cell_bytes))
        } else {
            None
        };
        let staging_max = if need_max {
            Some(self.staging_readback(&mut encoder, &max_buf, cell_bytes))
        } else {
            None
        };

        let submission = self.queue.submit(std::iter::once(encoder.finish()));

        // Wait for GPU.
        let _ = self
            .device
            .poll(PollType::WaitForSubmissionIndex(submission));

        // ── Map & build BinGrid ──────────────────────────────────────

        let counts = {
            let buf = staging_count.as_ref().unwrap();
            self.map_read_u32(buf, n_cells).await?
        };
        let sums = if let Some(ref buf) = staging_sum {
            Some(self.map_read_f32(buf, n_cells).await?)
        } else {
            None
        };
        let mins = if let Some(ref buf) = staging_min {
            Some(self.map_read_f32(buf, n_cells).await?)
        } else {
            None
        };
        let maxs = if let Some(ref buf) = staging_max {
            Some(self.map_read_f32(buf, n_cells).await?)
        } else {
            None
        };

        let cells: Vec<HeatmapCell> = (0..n_cells)
            .map(|i| {
                let xi = (i % x_spec.bins) as u32;
                let yi = (i / x_spec.bins) as u32;

                let count = counts[i];
                let value = if count == 0 {
                    no_data
                } else {
                    match func {
                        AggregateFunc::Count => count as f32,
                        AggregateFunc::Sum => sums.as_ref().unwrap()[i],
                        AggregateFunc::Mean => sums.as_ref().unwrap()[i] / count as f32,
                        AggregateFunc::Min => mins.as_ref().unwrap()[i],
                        AggregateFunc::Max => maxs.as_ref().unwrap()[i],
                    }
                };

                HeatmapCell {
                    x_index: xi,
                    y_index: yi,
                    value,
                }
            })
            .collect();

        Ok(BinGrid {
            cells,
            x_spec,
            y_spec,
        })
    }

    // ── helpers ──────────────────────────────────────────────────────

    fn storage_buffer(&self, label: &str, contents: &[u8]) -> Buffer {
        self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: BufferUsages::STORAGE,
        })
    }

    fn zeroed_storage_buffer(&self, label: &str, size: u64) -> Buffer {
        self.device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn filled_storage_buffer(&self, label: &str, count: usize, fill_bits: u32) -> Buffer {
        let data: Vec<u32> = vec![fill_bits; count];
        self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&data),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        })
    }

    fn staging_readback(&self, encoder: &mut CommandEncoder, src: &Buffer, size: u64) -> Buffer {
        let staging = self.device.create_buffer(&BufferDescriptor {
            label: Some("heatmap_staging"),
            size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
        staging
    }

    async fn map_read_u32(&self, staging: &Buffer, count: usize) -> GupResult<Vec<u32>> {
        let slice = staging.slice(..);
        let (tx, rx) = futures_channel::oneshot::channel();
        slice.map_async(MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(PollType::Wait);
        rx.await
            .map_err(|_| GupError::resource_error("GPU readback channel cancelled"))?
            .map_err(|e| GupError::resource_error(format!("GPU buffer map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let typed: &[u32] = bytemuck::cast_slice(&data);
        let result = typed[..count].to_vec();
        drop(data);
        staging.unmap();
        Ok(result)
    }

    async fn map_read_f32(&self, staging: &Buffer, count: usize) -> GupResult<Vec<f32>> {
        let raw = self.map_read_u32(staging, count).await?;
        Ok(raw.into_iter().map(f32::from_bits).collect())
    }
}

// ── Convenience function with CPU fallback ───────────────────────────────

/// Bin data using the GPU when available, falling back to the CPU path.
///
/// This is the primary entry-point for callers that want transparent
/// GPU acceleration without managing a [`GpuBinner`] directly.
#[allow(clippy::too_many_arguments)]
pub async fn gpu_bin_data(
    x_values: &[f32],
    y_values: &[f32],
    fill_values: &[f32],
    x_spec: BinSpec,
    y_spec: BinSpec,
    func: AggregateFunc,
    no_data: f32,
    context: Option<&RenderContext>,
) -> BinGrid {
    if let Some(ctx) = context {
        match GpuBinner::new(ctx) {
            Ok(binner) => {
                match binner
                    .bin(
                        x_values,
                        y_values,
                        fill_values,
                        x_spec,
                        y_spec,
                        func,
                        no_data,
                    )
                    .await
                {
                    Ok(grid) => return grid,
                    Err(_e) => {
                        // GPU binning failed — fall through to CPU.
                    }
                }
            }
            Err(_e) => {
                // Pipeline creation failed — fall through to CPU.
            }
        }
    }

    // CPU fallback.
    BinGrid::from_data(
        x_values,
        y_values,
        fill_values,
        x_spec,
        y_spec,
        func,
        no_data,
    )
}

// ── Bind group layout helper ─────────────────────────────────────────────

fn bgl_entry(binding: u32, ty: BufferBindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn buf_entry(binding: u32, buffer: &Buffer) -> BindGroupEntry<'_> {
    BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a RenderContext for tests.
    async fn test_context() -> RenderContext {
        RenderContext::new()
            .await
            .expect("GPU context required for tests")
    }

    fn make_spec(bins: usize, min: f32, max: f32) -> BinSpec {
        BinSpec::new(bins, min, max)
    }

    // ── Round-trip equivalence tests ─────────────────────────────────

    /// AC6: GPU-binned results match CPU-binned results within tolerance.
    #[tokio::test]
    async fn gpu_cpu_equivalence_count() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 10_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(10, 0.0, 10.0);
        let y_spec = make_spec(10, 0.0, 10.0);

        let cpu = BinGrid::from_data(
            &xs,
            &ys,
            &fs,
            x_spec,
            y_spec,
            AggregateFunc::Count,
            f32::NAN,
        );
        let gpu = binner
            .bin(
                &xs,
                &ys,
                &fs,
                x_spec,
                y_spec,
                AggregateFunc::Count,
                f32::NAN,
            )
            .await
            .unwrap();

        assert_grids_match(&cpu, &gpu, 0.0);
    }

    #[tokio::test]
    async fn gpu_cpu_equivalence_sum() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 10_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(10, 0.0, 10.0);
        let y_spec = make_spec(10, 0.0, 10.0);

        let cpu = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Sum, 0.0);
        let gpu = binner
            .bin(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Sum, 0.0)
            .await
            .unwrap();

        // Float addition is not associative, so allow a small tolerance.
        assert_grids_match(&cpu, &gpu, 0.1);
    }

    #[tokio::test]
    async fn gpu_cpu_equivalence_mean() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 10_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(10, 0.0, 10.0);
        let y_spec = make_spec(10, 0.0, 10.0);

        let cpu = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Mean, f32::NAN);
        let gpu = binner
            .bin(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Mean, f32::NAN)
            .await
            .unwrap();

        assert_grids_match(&cpu, &gpu, 0.01);
    }

    #[tokio::test]
    async fn gpu_cpu_equivalence_min() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 10_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(10, 0.0, 10.0);
        let y_spec = make_spec(10, 0.0, 10.0);

        let cpu = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Min, f32::NAN);
        let gpu = binner
            .bin(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Min, f32::NAN)
            .await
            .unwrap();

        assert_grids_match(&cpu, &gpu, 0.0);
    }

    #[tokio::test]
    async fn gpu_cpu_equivalence_max() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 10_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(10, 0.0, 10.0);
        let y_spec = make_spec(10, 0.0, 10.0);

        let cpu = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Max, f32::NAN);
        let gpu = binner
            .bin(&xs, &ys, &fs, x_spec, y_spec, AggregateFunc::Max, f32::NAN)
            .await
            .unwrap();

        assert_grids_match(&cpu, &gpu, 0.0);
    }

    #[tokio::test]
    async fn gpu_empty_input() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let grid = binner
            .bin(
                &[],
                &[],
                &[],
                make_spec(3, 0.0, 3.0),
                make_spec(3, 0.0, 3.0),
                AggregateFunc::Count,
                f32::NAN,
            )
            .await
            .unwrap();

        assert_eq!(grid.cells.len(), 9);
        for c in &grid.cells {
            assert!(c.value.is_nan(), "empty cell should be NaN");
        }
    }

    #[tokio::test]
    async fn gpu_single_cell() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let grid = binner
            .bin(
                &[0.5],
                &[0.5],
                &[42.0],
                make_spec(1, 0.0, 1.0),
                make_spec(1, 0.0, 1.0),
                AggregateFunc::Sum,
                0.0,
            )
            .await
            .unwrap();

        assert_eq!(grid.cells.len(), 1);
        assert!(
            (grid.cells[0].value - 42.0).abs() < f32::EPSILON,
            "got {}",
            grid.cells[0].value
        );
    }

    #[tokio::test]
    async fn gpu_cpu_fallback_works() {
        // Without a context the convenience function falls back to CPU.
        let grid = gpu_bin_data(
            &[0.5],
            &[0.5],
            &[1.0],
            make_spec(1, 0.0, 1.0),
            make_spec(1, 0.0, 1.0),
            AggregateFunc::Count,
            0.0,
            None,
        )
        .await;

        assert_eq!(grid.cells.len(), 1);
        assert!((grid.cells[0].value - 1.0).abs() < f32::EPSILON);
    }

    /// AC4-style test: 100×100 grid with 100k records.
    #[tokio::test]
    async fn gpu_cpu_equivalence_large_grid() {
        let ctx = test_context().await;
        let binner = GpuBinner::new(&ctx).unwrap();

        let n = 100_000usize;
        let (xs, ys, fs) = deterministic_data(n);

        let x_spec = make_spec(100, 0.0, 10.0);
        let y_spec = make_spec(100, 0.0, 10.0);

        for func in [
            AggregateFunc::Count,
            AggregateFunc::Sum,
            AggregateFunc::Mean,
            AggregateFunc::Min,
            AggregateFunc::Max,
        ] {
            let cpu = BinGrid::from_data(&xs, &ys, &fs, x_spec, y_spec, func, f32::NAN);
            let gpu = binner
                .bin(&xs, &ys, &fs, x_spec, y_spec, func, f32::NAN)
                .await
                .unwrap();

            // Float reductions are non-associative; allow a small tolerance
            // for Sum and Mean.
            let tol = match func {
                AggregateFunc::Sum => 1.0,
                AggregateFunc::Mean => 0.1,
                _ => 0.0,
            };
            assert_grids_match(&cpu, &gpu, tol);
        }
    }

    // ── Test helpers ─────────────────────────────────────────────────

    /// Deterministic pseudo-random data for reproducible tests.
    fn deterministic_data(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let mut xs = Vec::with_capacity(n);
        let mut ys = Vec::with_capacity(n);
        let mut fs = Vec::with_capacity(n);

        for i in 0..n {
            let t = i as f32 / n as f32;
            xs.push((t * 997.0) % 1.0 * 10.0);
            ys.push((t * 991.0) % 1.0 * 10.0);
            fs.push((t * 1009.0) % 1.0 * 100.0);
        }

        (xs, ys, fs)
    }

    /// Assert two BinGrids have identical structure and values within
    /// `tolerance`.
    fn assert_grids_match(cpu: &BinGrid, gpu: &BinGrid, tolerance: f32) {
        assert_eq!(cpu.cells.len(), gpu.cells.len(), "cell count mismatch");
        for (i, (c, g)) in cpu.cells.iter().zip(gpu.cells.iter()).enumerate() {
            assert_eq!(c.x_index, g.x_index, "x_index mismatch at cell {i}");
            assert_eq!(c.y_index, g.y_index, "y_index mismatch at cell {i}");
            if c.value.is_nan() && g.value.is_nan() {
                continue;
            }
            assert!(
                (c.value - g.value).abs() <= tolerance,
                "value mismatch at cell {i} ({},{}): cpu={} gpu={} (tol={tolerance})",
                c.x_index,
                c.y_index,
                c.value,
                g.value,
            );
        }
    }
}
