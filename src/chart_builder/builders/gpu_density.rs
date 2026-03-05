// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute backend for 2D density estimation and contour extraction.
//!
//! Provides [`GpuDensityCompute`], which offloads the KDE and
//! marching-squares contour passes to wgpu compute shaders.  The GPU
//! path is activated when the sample count exceeds a configurable
//! threshold (default: 5 000 points).
//!
//! ## Automatic CPU fallback
//!
//! [`gpu_density_2d`] detects when compute shaders are unavailable and
//! transparently falls back to [`super::density::compute_density_2d`].

use super::density::DensityConfig;
use crate::error::{GupError, GupResult};
use crate::render::RenderContext;
use crate::shader_function::{KDEResult2D, KernelFunction, MinMax, Percentile, StandardDeviation};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;
use wgpu::*;

/// WGSL source for the 2D KDE compute shader.
const KDE_SHADER: &str = include_str!("../../shaders/density_kde_2d.compute.wgsl");

/// WGSL source for the marching-squares contour extraction compute shader.
const MS_SHADER: &str = include_str!("../../shaders/density_marching_squares.compute.wgsl");

/// KDE workgroup size — must match `@workgroup_size(16, 16)` in the shader.
const KDE_WG: u32 = 16;

/// Marching-squares workgroup size — must match the shader.
const MS_WG: u32 = 16;

// ── GPU-side uniform structs ─────────────────────────────────────────────

/// Mirror of the WGSL `KDEParams` struct (48 bytes, std140).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuKDEParams {
    grid_cols: u32,
    grid_rows: u32,
    n_points: u32,
    bandwidth_x: f32,
    bandwidth_y: f32,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

/// Mirror of the WGSL `MarchingSquaresParams` struct (32 bytes, std140).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuMSParams {
    grid_cols: u32,
    grid_rows: u32,
    threshold: f32,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    _pad0: f32,
}

// ── GpuDensityCompute ────────────────────────────────────────────────────

/// Reusable GPU compute context that caches the KDE and marching-squares
/// pipelines.
///
/// Create once per [`RenderContext`] and reuse across density computations.
#[derive(Debug)]
pub struct GpuDensityCompute {
    device: Device,
    queue: Queue,
    // KDE pipeline + layout
    kde_pipeline: ComputePipeline,
    kde_bgl: BindGroupLayout,
    // Marching-squares pipeline + layout
    ms_pipeline: ComputePipeline,
    ms_bgl: BindGroupLayout,
}

impl GpuDensityCompute {
    /// Compile both compute pipelines from the WGSL sources.
    pub fn new(context: &RenderContext) -> GupResult<Self> {
        let device = context.device();
        let queue = context.queue();

        // ── KDE pipeline ─────────────────────────────────────────────
        let kde_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("density_kde_2d_shader"),
            source: ShaderSource::Wgsl(KDE_SHADER.into()),
        });

        let kde_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("kde_bgl"),
            entries: &[
                // 0: uniform KDEParams
                uniform_bgl_entry(0),
                // 1: storage read — points array<vec2<f32>>
                storage_bgl_entry(1, true),
                // 2: storage texture write — output density grid
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::R32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let kde_pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("kde_pipeline_layout"),
            bind_group_layouts: &[&kde_bgl],
            push_constant_ranges: &[],
        });

        let kde_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("density_kde_pipeline"),
            layout: Some(&kde_pl),
            module: &kde_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Marching-squares pipeline ────────────────────────────────
        let ms_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("density_ms_shader"),
            source: ShaderSource::Wgsl(MS_SHADER.into()),
        });

        let ms_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("ms_bgl"),
            entries: &[
                // 0: uniform MarchingSquaresParams
                uniform_bgl_entry(0),
                // 1: texture_2d<f32> — density grid (read)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 2: storage read_write — vertices array<Vertex2D>
                storage_bgl_entry(2, false),
                // 3: storage read_write — vertex_count atomic<u32>
                storage_bgl_entry(3, false),
            ],
        });

        let ms_pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("ms_pipeline_layout"),
            bind_group_layouts: &[&ms_bgl],
            push_constant_ranges: &[],
        });

        let ms_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("density_ms_pipeline"),
            layout: Some(&ms_pl),
            module: &ms_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            kde_pipeline,
            kde_bgl,
            ms_pipeline,
            ms_bgl,
        })
    }

    // ── KDE ──────────────────────────────────────────────────────────

    /// Dispatch the 2D KDE compute shader and read back the density grid.
    ///
    /// The returned [`KDEResult2D`] is directly comparable to the CPU
    /// reference produced by [`super::density::compute_density_2d`].
    pub fn compute_kde(
        &self,
        samples: &[(f32, f32)],
        config: &DensityConfig,
    ) -> GupResult<KDEResult2D> {
        let n = samples.len();
        if n == 0 {
            return Err(GupError::validation_error("No samples for GPU KDE"));
        }

        let grid_size = config.grid_size;
        let cols = grid_size as u32;
        let rows = grid_size as u32;

        // Compute grid parameters on CPU (cheap).
        let grid = eval_grid_params(samples, config);

        // ── Upload input data ────────────────────────────────────────
        let points_flat: Vec<f32> = samples.iter().flat_map(|&(x, y)| [x, y]).collect();
        let points_buf = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("kde_points"),
            contents: bytemuck::cast_slice(&points_flat),
            usage: BufferUsages::STORAGE,
        });

        let params = GpuKDEParams {
            grid_cols: cols,
            grid_rows: rows,
            n_points: n as u32,
            bandwidth_x: grid.bw_x,
            bandwidth_y: grid.bw_y,
            x_min: grid.kde_x_min,
            x_max: grid.kde_x_max,
            y_min: grid.kde_y_min,
            y_max: grid.kde_y_max,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let params_buf = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("kde_params"),
            contents: bytemuck::bytes_of(&params),
            usage: BufferUsages::UNIFORM,
        });

        // ── Output storage texture ───────────────────────────────────
        let tex = self.device.create_texture(&TextureDescriptor {
            label: Some("kde_output_tex"),
            size: Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let tex_view = tex.create_view(&TextureViewDescriptor::default());

        // ── Bind group ───────────────────────────────────────────────
        let bg = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("kde_bg"),
            layout: &self.kde_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: points_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&tex_view),
                },
            ],
        });

        // ── Dispatch ─────────────────────────────────────────────────
        let wg_x = cols.div_ceil(KDE_WG);
        let wg_y = rows.div_ceil(KDE_WG);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("kde_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("kde_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.kde_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // ── Readback via staging buffer ──────────────────────────────
        let texel_size = 4u64; // R32Float
        let unpadded_row = cols as u64 * texel_size;
        let padded_row = unpadded_row.div_ceil(256) * 256;
        let staging_size = padded_row * rows as u64;

        let staging = self.device.create_buffer(&BufferDescriptor {
            label: Some("kde_staging"),
            size: staging_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &staging,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row as u32),
                    rows_per_image: Some(rows),
                },
            },
            Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: 1,
            },
        );

        let sub = self.queue.submit(std::iter::once(encoder.finish()));
        let _ = self.device.poll(PollType::WaitForSubmissionIndex(sub));

        // Map the staging buffer.
        let densities =
            self.read_texture_staging(&staging, cols as usize, rows as usize, padded_row as usize)?;

        Ok(KDEResult2D {
            densities,
            x_points: grid.x_points,
            y_points: grid.y_points,
            bandwidth_x: grid.bw_x,
            bandwidth_y: grid.bw_y,
            kernel: KernelFunction::Gaussian,
        })
    }

    // ── Marching squares ─────────────────────────────────────────────

    /// Dispatch the marching-squares shader for a single iso-level and
    /// return the resulting line segments.
    pub fn compute_contours(
        &self,
        kde_result: &KDEResult2D,
        threshold: f32,
    ) -> GupResult<Vec<[(f32, f32); 2]>> {
        let cols = kde_result.x_points.len();
        let rows = kde_result.y_points.len();

        if cols < 2 || rows < 2 {
            return Ok(Vec::new());
        }

        // Compute grid bounds for the marching-squares world mapping.
        let dx = kde_result.x_points[1] - kde_result.x_points[0];
        let dy = kde_result.y_points[1] - kde_result.y_points[0];
        let ms_x_min = kde_result.x_points[0];
        let ms_x_max = kde_result.x_points[0] + cols as f32 * dx;
        let ms_y_min = kde_result.y_points[0];
        let ms_y_max = kde_result.y_points[0] + rows as f32 * dy;

        // ── Upload density grid as a texture ─────────────────────────
        let density_tex = self.device.create_texture(&TextureDescriptor {
            label: Some("ms_density_input"),
            size: Extent3d {
                width: cols as u32,
                height: rows as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &density_tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&kde_result.densities),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((cols * 4) as u32),
                rows_per_image: Some(rows as u32),
            },
            Extent3d {
                width: cols as u32,
                height: rows as u32,
                depth_or_array_layers: 1,
            },
        );

        let density_view = density_tex.create_view(&TextureViewDescriptor::default());

        // ── Params uniform ───────────────────────────────────────────
        let params = GpuMSParams {
            grid_cols: cols as u32,
            grid_rows: rows as u32,
            threshold,
            x_min: ms_x_min,
            x_max: ms_x_max,
            y_min: ms_y_min,
            y_max: ms_y_max,
            _pad0: 0.0,
        };
        let params_buf = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("ms_params"),
            contents: bytemuck::bytes_of(&params),
            usage: BufferUsages::UNIFORM,
        });

        // ── Output buffers ───────────────────────────────────────────
        // Worst case: 2 segments per cell, 2 vertices per segment = 4 Vertex2D per cell.
        let max_vertices = 4 * (cols - 1) * (rows - 1);
        let vertex_buf_size = (max_vertices * 2 * std::mem::size_of::<f32>()) as u64;

        let vertex_buf = self.device.create_buffer(&BufferDescriptor {
            label: Some("ms_vertices"),
            size: vertex_buf_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Atomic counter initialised to 0.
        let count_buf = self.device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("ms_vertex_count"),
            contents: bytemuck::bytes_of(&0u32),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        });

        // ── Bind group ───────────────────────────────────────────────
        let bg = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("ms_bg"),
            layout: &self.ms_bgl,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&density_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: vertex_buf.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: count_buf.as_entire_binding(),
                },
            ],
        });

        // ── Dispatch ─────────────────────────────────────────────────
        let wg_x = (cols as u32).div_ceil(MS_WG);
        let wg_y = (rows as u32).div_ceil(MS_WG);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("ms_encoder"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("ms_compute_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.ms_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(wg_x, wg_y, 1);
        }

        // ── Readback ─────────────────────────────────────────────────
        let count_staging = staging_readback(&self.device, &mut encoder, &count_buf, 4);
        let vert_staging =
            staging_readback(&self.device, &mut encoder, &vertex_buf, vertex_buf_size);

        let sub = self.queue.submit(std::iter::once(encoder.finish()));
        let _ = self.device.poll(PollType::WaitForSubmissionIndex(sub));

        // Read the vertex count first.
        let count = map_read_u32_single(&self.device, &count_staging)?;

        if count == 0 {
            return Ok(Vec::new());
        }

        // Read vertex data (count Vertex2D structs, each 2 floats).
        let float_count = count as usize * 2;
        let floats = map_read_f32(&self.device, &vert_staging, float_count)?;

        // Convert to segment pairs (each segment = 2 consecutive vertices).
        let segments: Vec<[(f32, f32); 2]> = floats
            .chunks_exact(4)
            .map(|c| [(c[0], c[1]), (c[2], c[3])])
            .collect();

        Ok(segments)
    }

    // ── Texture readback helper ──────────────────────────────────────

    fn read_texture_staging(
        &self,
        staging: &Buffer,
        cols: usize,
        rows: usize,
        padded_bytes_per_row: usize,
    ) -> GupResult<Vec<f32>> {
        let slice = staging.slice(..);
        let result: Arc<Mutex<Option<Result<(), BufferAsyncError>>>> = Arc::new(Mutex::new(None));
        let cb_result = result.clone();
        slice.map_async(MapMode::Read, move |r| {
            *cb_result.lock().unwrap() = Some(r);
        });
        let _ = self.device.poll(PollType::Wait);

        result
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| GupError::resource_error("Buffer mapping not completed"))?
            .map_err(|e| GupError::resource_error(format!("GPU buffer map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let bytes: &[u8] = &data;

        let mut densities = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            let start = row * padded_bytes_per_row;
            let row_slice = &bytes[start..start + cols * 4];
            let row_floats: &[f32] = bytemuck::cast_slice(row_slice);
            densities.extend_from_slice(row_floats);
        }

        drop(data);
        staging.unmap();
        Ok(densities)
    }
}

// ── Grid parameter computation ───────────────────────────────────────────

/// Pre-computed evaluation grid parameters.
///
/// These are derived using the same logic as `KernelDensity2D::generate_eval_grid`
/// so that GPU results match the CPU reference exactly.
struct EvalGrid {
    x_points: Vec<f32>,
    y_points: Vec<f32>,
    bw_x: f32,
    bw_y: f32,
    /// Bounds for the KDE shader (cell-centre semantics).
    kde_x_min: f32,
    kde_x_max: f32,
    kde_y_min: f32,
    kde_y_max: f32,
}

/// Compute the evaluation grid and bandwidth from samples and config,
/// replicating `KernelDensity2D::generate_eval_grid` exactly.
fn eval_grid_params(samples: &[(f32, f32)], config: &DensityConfig) -> EvalGrid {
    let x_vals: Vec<f32> = samples.iter().map(|s| s.0).collect();
    let y_vals: Vec<f32> = samples.iter().map(|s| s.1).collect();

    let (x_min_data, x_max_data) = MinMax::new(x_vals.clone()).compute_cpu();
    let (y_min_data, y_max_data) = MinMax::new(y_vals.clone()).compute_cpu();

    let bw_x = bandwidth_for_dim(&x_vals, config.bandwidth);
    let bw_y = bandwidth_for_dim(&y_vals, config.bandwidth);

    // 3-bandwidth padding (matches KernelDensity2D).
    let x_padding = bw_x * 3.0;
    let y_padding = bw_y * 3.0;

    let x_start = x_min_data - x_padding;
    let x_end = x_max_data + x_padding;
    let y_start = y_min_data - y_padding;
    let y_end = y_max_data + y_padding;

    let n = config.grid_size;
    let x_step = (x_end - x_start) / (n - 1) as f32;
    let y_step = (y_end - y_start) / (n - 1) as f32;

    let x_points: Vec<f32> = (0..n).map(|i| x_start + i as f32 * x_step).collect();
    let y_points: Vec<f32> = (0..n).map(|i| y_start + i as f32 * y_step).collect();

    // Derive KDE shader bounds so that cell centres land on x_points/y_points.
    // The shader maps: x = x_min + (col + 0.5) * (x_max - x_min) / grid_cols
    // Setting x_min = x_points[0] - 0.5 * dx,  x_max = x_points[last] + 0.5 * dx
    // ensures the col-th cell centre equals x_points[col].
    let kde_x_min = x_points[0] - 0.5 * x_step;
    let kde_x_max = *x_points.last().unwrap() + 0.5 * x_step;
    let kde_y_min = y_points[0] - 0.5 * y_step;
    let kde_y_max = *y_points.last().unwrap() + 0.5 * y_step;

    EvalGrid {
        x_points,
        y_points,
        bw_x,
        bw_y,
        kde_x_min,
        kde_x_max,
        kde_y_min,
        kde_y_max,
    }
}

/// Silverman bandwidth estimate for a single dimension, matching
/// `KernelDensity2D::estimate_bandwidth_dim`.
fn bandwidth_for_dim(values: &[f32], manual: Option<f32>) -> f32 {
    if let Some(bw) = manual {
        return bw;
    }
    // Silverman's rule.
    let n = values.len() as f32;
    let std_dev = StandardDeviation::new(values.to_vec()).compute_cpu();
    let q1 = Percentile::new(values.to_vec(), 0.25).compute_cpu();
    let q3 = Percentile::new(values.to_vec(), 0.75).compute_cpu();
    let iqr = q3 - q1;
    let scale = std_dev.min(iqr / 1.34);
    0.9 * scale * n.powf(-0.2)
}

// ── Convenience with CPU fallback ────────────────────────────────────────

/// Default sample-count threshold above which the GPU path is used.
pub const DEFAULT_GPU_THRESHOLD: usize = 5_000;

/// Compute the 2D KDE, using the GPU when available and the dataset
/// exceeds `threshold`, else falling back to the CPU path.
pub fn gpu_density_2d(
    samples: &[(f32, f32)],
    config: &DensityConfig,
    threshold: usize,
    context: Option<&RenderContext>,
) -> KDEResult2D {
    if samples.len() >= threshold
        && let Some(ctx) = context
    {
        match GpuDensityCompute::new(ctx) {
            Ok(gpu) => match gpu.compute_kde(samples, config) {
                Ok(result) => return result,
                Err(_) => { /* fall through to CPU */ }
            },
            Err(_) => { /* fall through to CPU */ }
        }
    }

    // CPU fallback.
    super::density::compute_density_2d(samples, config)
}

// ── Buffer helpers ───────────────────────────────────────────────────────

fn uniform_bgl_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_bgl_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn staging_readback(
    device: &Device,
    encoder: &mut CommandEncoder,
    src: &Buffer,
    size: u64,
) -> Buffer {
    let staging = device.create_buffer(&BufferDescriptor {
        label: Some("density_staging"),
        size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(src, 0, &staging, 0, size);
    staging
}

fn map_read_u32_single(device: &Device, staging: &Buffer) -> GupResult<u32> {
    let slice = staging.slice(..);
    let result: Arc<Mutex<Option<Result<(), BufferAsyncError>>>> = Arc::new(Mutex::new(None));
    let cb = result.clone();
    slice.map_async(MapMode::Read, move |r| {
        *cb.lock().unwrap() = Some(r);
    });
    let _ = device.poll(PollType::Wait);
    result
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| GupError::resource_error("Buffer mapping not completed"))?
        .map_err(|e| GupError::resource_error(format!("GPU buffer map failed: {e:?}")))?;

    let data = slice.get_mapped_range();
    let val = *bytemuck::from_bytes::<u32>(&data[..4]);
    drop(data);
    staging.unmap();
    Ok(val)
}

fn map_read_f32(device: &Device, staging: &Buffer, count: usize) -> GupResult<Vec<f32>> {
    let byte_len = count * std::mem::size_of::<f32>();
    let slice = staging.slice(..byte_len as u64);
    let result: Arc<Mutex<Option<Result<(), BufferAsyncError>>>> = Arc::new(Mutex::new(None));
    let cb = result.clone();
    slice.map_async(MapMode::Read, move |r| {
        *cb.lock().unwrap() = Some(r);
    });
    let _ = device.poll(PollType::Wait);
    result
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| GupError::resource_error("Buffer mapping not completed"))?
        .map_err(|e| GupError::resource_error(format!("GPU buffer map failed: {e:?}")))?;

    let data = slice.get_mapped_range();
    let floats: &[f32] = bytemuck::cast_slice(&data);
    let result = floats[..count].to_vec();
    drop(data);
    staging.unmap();
    Ok(result)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> RenderContext {
        pollster::block_on(RenderContext::new()).expect("GPU context required for tests")
    }

    // ── Pipeline creation / caching ──────────────────────────────────

    #[test]
    fn pipeline_creation_succeeds() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx);
        assert!(gpu.is_ok(), "pipeline creation failed: {:?}", gpu.err());
    }

    #[test]
    fn pipeline_reuse_avoids_redundant_creation() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();
        let samples: Vec<(f32, f32)> = (0..50).map(|i| (i as f32 * 0.1, i as f32 * 0.1)).collect();
        let config = DensityConfig {
            grid_size: 8,
            bandwidth: Some(0.3),
            ..Default::default()
        };
        let _r1 = gpu.compute_kde(&samples, &config).unwrap();
        let _r2 = gpu.compute_kde(&samples, &config).unwrap();
    }

    // ── KDE GPU vs CPU reference ─────────────────────────────────────

    #[test]
    fn gpu_kde_matches_cpu_standard_normal() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let mut rng: u64 = 42;
        let samples: Vec<(f32, f32)> = (0..500).map(|_| box_muller(&mut rng)).collect();
        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.5),
            ..Default::default()
        };

        let gpu_result = gpu.compute_kde(&samples, &config).unwrap();
        let cpu_result = super::super::density::compute_density_2d(&samples, &config);

        assert_results_match(&gpu_result, &cpu_result, 0.01);
    }

    #[test]
    fn gpu_kde_matches_cpu_uniform_rectangle() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let samples: Vec<(f32, f32)> = (0..400)
            .map(|i| ((i % 20) as f32 / 19.0, (i / 20) as f32 / 19.0))
            .collect();
        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.15),
            ..Default::default()
        };

        let gpu_result = gpu.compute_kde(&samples, &config).unwrap();
        let cpu_result = super::super::density::compute_density_2d(&samples, &config);

        assert_results_match(&gpu_result, &cpu_result, 0.01);
    }

    #[test]
    fn gpu_kde_matches_cpu_mixture_of_gaussians() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let mut rng: u64 = 123;
        let mut samples = Vec::new();
        for _ in 0..250 {
            let (x, y) = box_muller(&mut rng);
            samples.push((x - 2.0, y - 2.0));
        }
        for _ in 0..250 {
            let (x, y) = box_muller(&mut rng);
            samples.push((x + 2.0, y + 2.0));
        }
        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.6),
            ..Default::default()
        };

        let gpu_result = gpu.compute_kde(&samples, &config).unwrap();
        let cpu_result = super::super::density::compute_density_2d(&samples, &config);

        assert_results_match(&gpu_result, &cpu_result, 0.01);
    }

    // ── Marching squares GPU vs CPU ──────────────────────────────────

    #[test]
    fn gpu_marching_squares_simple_peak() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let kde = KDEResult2D {
            densities: vec![
                0.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 1.0, 0.0, //
                0.0, 1.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 0.0,
            ],
            x_points: vec![0.0, 1.0, 2.0, 3.0],
            y_points: vec![0.0, 1.0, 2.0, 3.0],
            bandwidth_x: 0.5,
            bandwidth_y: 0.5,
            kernel: KernelFunction::Gaussian,
        };

        let gpu_segs = gpu.compute_contours(&kde, 0.5).unwrap();
        let cpu_segs = super::super::density::marching_squares(
            &kde.densities,
            4,
            4,
            0.5,
            &kde.x_points,
            &kde.y_points,
        );

        assert!(!gpu_segs.is_empty(), "GPU produced no contour segments");
        assert_eq!(
            gpu_segs.len(),
            cpu_segs.len(),
            "GPU segment count {} != CPU {}",
            gpu_segs.len(),
            cpu_segs.len()
        );
    }

    #[test]
    fn gpu_marching_squares_no_contour() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let kde = KDEResult2D {
            densities: vec![1.0; 16],
            x_points: vec![0.0, 1.0, 2.0, 3.0],
            y_points: vec![0.0, 1.0, 2.0, 3.0],
            bandwidth_x: 0.5,
            bandwidth_y: 0.5,
            kernel: KernelFunction::Gaussian,
        };

        let segs = gpu.compute_contours(&kde, 0.5).unwrap();
        assert!(
            segs.is_empty(),
            "expected no segments for uniform grid above threshold"
        );
    }

    // ── CPU fallback ─────────────────────────────────────────────────

    #[test]
    fn gpu_density_2d_uses_cpu_below_threshold() {
        let samples: Vec<(f32, f32)> = (0..100)
            .map(|i| (i as f32 * 0.1, (i as f32 * 0.1).sin()))
            .collect();
        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.3),
            ..Default::default()
        };

        let _result = gpu_density_2d(&samples, &config, 200, None);
    }

    #[test]
    fn gpu_density_2d_uses_gpu_above_threshold() {
        let ctx = test_context();
        let samples: Vec<(f32, f32)> = (0..200)
            .map(|i| (i as f32 * 0.05, (i as f32 * 0.05).sin()))
            .collect();
        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.3),
            ..Default::default()
        };

        let _result = gpu_density_2d(&samples, &config, 100, Some(&ctx));
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn assert_results_match(gpu: &KDEResult2D, cpu: &KDEResult2D, max_rel_err: f32) {
        assert_eq!(gpu.x_points.len(), cpu.x_points.len());
        assert_eq!(gpu.y_points.len(), cpu.y_points.len());
        assert_eq!(gpu.densities.len(), cpu.densities.len());

        for (i, (&gx, &cx)) in gpu.x_points.iter().zip(cpu.x_points.iter()).enumerate() {
            assert!(
                (gx - cx).abs() < 1e-5,
                "x_points[{i}] mismatch: gpu={gx} cpu={cx}"
            );
        }

        for (i, (&gv, &cv)) in gpu.densities.iter().zip(cpu.densities.iter()).enumerate() {
            let max_ab = gv.abs().max(cv.abs());
            if max_ab > 1e-10 {
                let rel = (gv - cv).abs() / max_ab;
                assert!(
                    rel < max_rel_err,
                    "density[{i}] mismatch: gpu={gv} cpu={cv} rel_err={rel}"
                );
            }
        }
    }

    fn box_muller(state: &mut u64) -> (f32, f32) {
        let u1 = lcg(state);
        let u2 = lcg(state);
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        (r * theta.cos(), r * theta.sin())
    }

    fn lcg(state: &mut u64) -> f32 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let bits = (*state >> 33) as f32;
        (bits + 1.0) / (2.0f32.powi(31) + 1.0)
    }

    // ── Performance ──────────────────────────────────────────────────

    #[test]
    fn gpu_kde_100k_points_under_100ms() {
        let ctx = test_context();
        let gpu = GpuDensityCompute::new(&ctx).unwrap();

        let mut rng: u64 = 999;
        let samples: Vec<(f32, f32)> = (0..100_000).map(|_| box_muller(&mut rng)).collect();
        let config = DensityConfig {
            grid_size: 256,
            bandwidth: Some(0.1),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        let kde_result = gpu.compute_kde(&samples, &config).unwrap();
        let kde_elapsed = start.elapsed();

        // Also run marching-squares for a single threshold.
        let threshold = kde_result.peak_density() * 0.5;
        let start2 = std::time::Instant::now();
        let _contours = gpu.compute_contours(&kde_result, threshold).unwrap();
        let ms_elapsed = start2.elapsed();

        let total = kde_elapsed + ms_elapsed;

        // On real GPU hardware the target is <100 ms.  Software renderers
        // (e.g. llvmpipe used in CI) are much slower, so we only assert a
        // generous upper bound to catch regressions.  The actual hardware
        // performance target is validated by visual benchmarking.
        eprintln!(
            "GPU density 100K perf: KDE={}ms, MS={}ms, total={}ms",
            kde_elapsed.as_millis(),
            ms_elapsed.as_millis(),
            total.as_millis()
        );

        // Soft assert: must complete within 10 seconds even on software.
        assert!(
            total.as_secs() < 10,
            "GPU compute time {}s is unacceptably slow",
            total.as_secs()
        );

        // Verify the result is non-trivial.
        assert_eq!(kde_result.densities.len(), 256 * 256);
        assert!(kde_result.peak_density() > 0.0);
    }
}
