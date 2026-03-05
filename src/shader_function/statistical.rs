// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Statistical computation shader functions.
//!
//! Provides GPU-accelerated statistical operations including normalization,
//! standardization, quantile computation, binning, statistics aggregation
//! (mean, median, std dev, min/max), histogram computation, streaming
//! statistics, and kernel density estimation (1D and 2D).

use super::core::*;
use crate::error::{GupError, GupResult};
use std::sync::Arc;

// ============================================================================
// Statistical Shader Functions (GUP-053 AC4)
// ============================================================================

/// Normalization shader function.
///
/// Normalizes a value from a data range [min, max] to [0, 1].
/// This is a composable GPU shader function for real-time data normalization.
#[derive(Clone, Debug)]
pub struct NormalizeFunction {
    /// Minimum value of the input range
    pub min: f32,
    /// Maximum value of the input range
    pub max: f32,
}

impl NormalizeFunction {
    /// Creates a new normalize function with the given range.
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

/// GPU uniform data for the normalize shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NormalizeFunctionUniforms {
    /// Minimum value of the input range.
    pub min: f32,
    /// Maximum value of the input range.
    pub max: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for NormalizeFunctionUniforms {
    fn wgsl_struct_definition() -> String {
        "struct NormalizeFunctionUniforms {\n    min: f32,\n    max: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "NormalizeFunctionUniforms"
    }
}

impl ComposableShaderFunction for NormalizeFunction {
    type Input = f32;
    type Output = f32;
    type Uniforms = NormalizeFunctionUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn normalize_fn(value: f32, params: NormalizeFunctionUniforms) -> f32 {
            let range = params.max - params.min;
            if (range == 0.0) {
                return 0.5;
            }
            return (value - params.min) / range;
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(NormalizeFunctionUniforms {
            min: self.min,
            max: self.max,
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "normalize_fn"
    }
}

/// Standardization (z-score) shader function.
///
/// Transforms a value to its z-score: (value - mean) / std_dev.
/// Produces output centered at 0 with unit variance.
#[derive(Clone, Debug)]
pub struct StandardizeFunction {
    /// Mean of the dataset
    pub mean: f32,
    /// Standard deviation of the dataset
    pub std_dev: f32,
}

impl StandardizeFunction {
    /// Creates a new standardize function with the given mean and standard deviation.
    pub fn new(mean: f32, std_dev: f32) -> Self {
        Self { mean, std_dev }
    }
}

/// GPU uniform data for the standardize (z-score) shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StandardizeFunctionUniforms {
    /// Mean of the dataset.
    pub mean: f32,
    /// Standard deviation of the dataset.
    pub std_dev: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for StandardizeFunctionUniforms {
    fn wgsl_struct_definition() -> String {
        "struct StandardizeFunctionUniforms {\n    mean: f32,\n    std_dev: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "StandardizeFunctionUniforms"
    }
}

impl ComposableShaderFunction for StandardizeFunction {
    type Input = f32;
    type Output = f32;
    type Uniforms = StandardizeFunctionUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn standardize_fn(value: f32, params: StandardizeFunctionUniforms) -> f32 {
            if (params.std_dev == 0.0) {
                return 0.0;
            }
            return (value - params.mean) / params.std_dev;
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(StandardizeFunctionUniforms {
            mean: self.mean,
            std_dev: self.std_dev,
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "standardize_fn"
    }
}

/// Quantile mapping shader function.
///
/// Maps a value to its quantile position within pre-computed quantile
/// boundaries. Given N quantile boundaries, maps input to [0, 1] based
/// on which quantile bin the value falls in.
#[derive(Clone, Debug)]
pub struct QuantileFunction {
    /// Sorted quantile boundaries (e.g., quartile edges)
    pub boundaries: Vec<f32>,
}

impl QuantileFunction {
    /// Creates a new quantile function with the given boundaries.
    pub fn new(boundaries: Vec<f32>) -> Self {
        Self { boundaries }
    }

    /// Creates a quantile function from quartile boundaries (Q1, Q2, Q3).
    pub fn from_quartiles(q1: f32, q2: f32, q3: f32) -> Self {
        Self::new(vec![q1, q2, q3])
    }
}

/// Uniform structure for quantile function (supports up to 16 boundaries).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuantileFunctionUniforms {
    /// Quantile boundary values (up to 16).
    pub boundaries: [f32; 16],
    /// Number of active boundaries.
    pub count: u32,
    /// Padding for GPU alignment.
    pub _padding: [u32; 3],
}

impl ShaderUniform for QuantileFunctionUniforms {
    fn wgsl_struct_definition() -> String {
        "struct QuantileFunctionUniforms {\n    boundaries: array<f32, 16>,\n    count: u32,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "QuantileFunctionUniforms"
    }
}

impl ComposableShaderFunction for QuantileFunction {
    type Input = f32;
    type Output = f32;
    type Uniforms = QuantileFunctionUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn quantile_fn(value: f32, params: QuantileFunctionUniforms) -> f32 {
            if (params.count == 0u) {
                return 0.0;
            }
            // Count how many boundaries the value exceeds
            var bin = 0u;
            for (var i = 0u; i < params.count; i = i + 1u) {
                if (value >= params.boundaries[i]) {
                    bin = i + 1u;
                }
            }
            // Normalize to [0, 1]
            return f32(bin) / f32(params.count + 1u);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        let count = self.boundaries.len().min(16);
        let mut boundaries = [0.0f32; 16];
        for (i, &b) in self.boundaries.iter().take(count).enumerate() {
            boundaries[i] = b;
        }
        Some(QuantileFunctionUniforms {
            boundaries,
            count: count as u32,
            _padding: [0; 3],
        })
    }

    fn function_name() -> &'static str {
        "quantile_fn"
    }
}

/// Binning (discretization) shader function.
///
/// Maps continuous values into discrete bin indices, normalized to [0, 1].
/// Useful for histogram-like visual encoding.
#[derive(Clone, Debug)]
pub struct BinningFunction {
    /// Lower bound of the data range
    pub min: f32,
    /// Upper bound of the data range
    pub max: f32,
    /// Number of bins
    pub bin_count: u32,
}

impl BinningFunction {
    /// Creates a new binning function with the given range and bin count.
    pub fn new(min: f32, max: f32, bin_count: u32) -> Self {
        Self {
            min,
            max,
            bin_count,
        }
    }
}

/// GPU uniform data for the binning shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BinningFunctionUniforms {
    /// Lower bound of the data range.
    pub min: f32,
    /// Upper bound of the data range.
    pub max: f32,
    /// Number of bins.
    pub bin_count: u32,
    /// Padding for GPU alignment.
    pub _padding: u32,
}

impl ShaderUniform for BinningFunctionUniforms {
    fn wgsl_struct_definition() -> String {
        "struct BinningFunctionUniforms {\n    min: f32,\n    max: f32,\n    bin_count: u32,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "BinningFunctionUniforms"
    }
}

impl ComposableShaderFunction for BinningFunction {
    type Input = f32;
    type Output = f32;
    type Uniforms = BinningFunctionUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn binning_fn(value: f32, params: BinningFunctionUniforms) -> f32 {
            let range = params.max - params.min;
            if (range == 0.0 || params.bin_count == 0u) {
                return 0.0;
            }
            let normalized = clamp((value - params.min) / range, 0.0, 1.0);
            let bin = min(u32(normalized * f32(params.bin_count)), params.bin_count - 1u);
            return (f32(bin) + 0.5) / f32(params.bin_count);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(BinningFunctionUniforms {
            min: self.min,
            max: self.max,
            bin_count: self.bin_count,
            _padding: 0,
        })
    }

    fn function_name() -> &'static str {
        "binning_fn"
    }
}

// ============================================================================
// Statistical Aggregation Functions (GUP-139)
// ============================================================================

/// GPU-accelerated statistical aggregation system for computing mean, median,
/// standard deviation, percentiles, and other statistical measures on large datasets.
///
/// This module provides compute shader-based parallel reduction algorithms for
/// efficient statistical computation on the GPU. These functions are designed for
/// data-driven statistical visualizations like box plots, violin plots, and
/// distribution analyses.
///
/// # Architecture
///
/// Statistical aggregations use a two-stage reduction approach:
/// 1. **Local Reduction**: Each workgroup computes partial results using shared memory
/// 2. **Global Reduction**: Partial results are combined to produce final statistics
///
/// This approach enables efficient processing of millions of data points with minimal
/// CPU-GPU round trips.
/// Result of statistical aggregation computation
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StatisticsResult {
    /// Number of valid data points processed
    pub count: u32,
    /// Sum of all values
    pub sum: f32,
    /// Minimum value
    pub min: f32,
    /// Maximum value
    pub max: f32,
    /// Mean (average) value
    pub mean: f32,
    /// Variance
    pub variance: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

/// GPU compute pipeline for statistical aggregations
pub struct StatisticsCompute {
    /// Compute pipeline for basic statistics (mean, min, max, std dev)
    basic_stats_pipeline: Option<wgpu::ComputePipeline>,
    /// Compute pipeline for variance (second pass)
    variance_pipeline: Option<wgpu::ComputePipeline>,
    /// Compute pipeline for median and percentiles
    #[allow(dead_code)]
    percentile_pipeline: Option<wgpu::ComputePipeline>,
    /// Input data buffer
    data_buffer: Option<wgpu::Buffer>,
    /// Output statistics buffer
    result_buffer: Option<wgpu::Buffer>,
    /// Maximum number of elements
    #[allow(dead_code)]
    max_elements: usize,
    /// Device and queue references
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
}

impl StatisticsCompute {
    /// Create a new statistics compute system
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        max_elements: usize,
    ) -> GupResult<Self> {
        let basic_stats_pipeline = Self::create_basic_stats_pipeline(device).await?;
        let variance_pipeline = Self::create_variance_pipeline(device).await?;
        let percentile_pipeline = Self::create_percentile_pipeline(device).await?;

        let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_data"),
            size: (max_elements * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_result"),
            size: std::mem::size_of::<StatisticsResult>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            basic_stats_pipeline: Some(basic_stats_pipeline),
            variance_pipeline: Some(variance_pipeline),
            percentile_pipeline: Some(percentile_pipeline),
            data_buffer: Some(data_buffer),
            result_buffer: Some(result_buffer),
            max_elements,
            device: Some(Arc::new(device.clone())),
            queue: Some(Arc::new(queue.clone())),
        })
    }

    /// Create compute pipeline for basic statistics
    async fn create_basic_stats_pipeline(
        device: &wgpu::Device,
    ) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("../shaders/statistics.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("statistics_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("basic_stats_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_basic_stats"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create compute pipeline for variance calculation
    async fn create_variance_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("../shaders/statistics.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("statistics_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("variance_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_variance"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create compute pipeline for percentile calculation
    async fn create_percentile_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("../shaders/percentile.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("percentile_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("percentile_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_percentile"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Compute basic statistics (mean, min, max, std dev) for a dataset
    pub async fn compute_basic_stats(&self, data: &[f32]) -> GupResult<StatisticsResult> {
        if data.is_empty() {
            return Ok(StatisticsResult {
                count: 0,
                sum: 0.0,
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                variance: 0.0,
                std_dev: 0.0,
                _padding: 0,
            });
        }

        let device = self.device.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute device not initialized".to_string(),
            )
        })?;
        let queue = self.queue.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute queue not initialized".to_string(),
            )
        })?;
        let data_buffer = self.data_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute data_buffer not initialized".to_string(),
            )
        })?;
        let result_buffer = self.result_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute result_buffer not initialized".to_string(),
            )
        })?;

        // Upload data to GPU
        queue.write_buffer(data_buffer, 0, bytemuck::cast_slice(data));

        // Initialize result buffer with actual data count
        // IMPORTANT: Set count to actual data size so shader can use it via result.count
        let init_result = StatisticsResult {
            count: data.len() as u32,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            _padding: 0,
        };
        queue.write_buffer(result_buffer, 0, bytemuck::bytes_of(&init_result));

        // Create bind group
        let pipeline = self.basic_stats_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute pipeline not initialized".to_string(),
            )
        })?;
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("statistics_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute pass for basic stats
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("statistics_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("statistics_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            // Dispatch with workgroups covering all data
            let workgroup_size = 256;
            let num_workgroups = data.len().div_ceil(workgroup_size) as u32;
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit(Some(encoder.finish()));
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        // Second pass: compute variance (requires mean from first pass)
        let variance_pipeline = self.variance_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute variance pipeline not initialized".to_string(),
            )
        })?;

        // Create bind group for variance pipeline
        let variance_bind_group_layout = variance_pipeline.get_bind_group_layout(0);
        let variance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("variance_bind_group"),
            layout: &variance_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("variance_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("variance_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(variance_pipeline);
            compute_pass.set_bind_group(0, &variance_bind_group, &[]);
            let workgroup_size = 256;
            let num_workgroups = data.len().div_ceil(workgroup_size) as u32;
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit(Some(encoder.finish()));

        // Read results back
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_staging"),
            size: std::mem::size_of::<StatisticsResult>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("statistics_copy_encoder"),
        });
        encoder.copy_buffer_to_buffer(
            result_buffer,
            0,
            &staging_buffer,
            0,
            std::mem::size_of::<StatisticsResult>() as u64,
        );
        queue.submit(Some(encoder.finish()));

        // Wait for GPU to complete
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        // Map and read results
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.await
            .map_err(|_| {
                GupError::gpu_initialization_failed(
                    "Failed to receive buffer map result".to_string(),
                )
            })?
            .map_err(|e| {
                GupError::gpu_initialization_failed(format!("Buffer mapping failed: {:?}", e))
            })?;

        let data = buffer_slice.get_mapped_range();

        let result: StatisticsResult =
            *bytemuck::from_bytes(&data[..std::mem::size_of::<StatisticsResult>()]);
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

/// Configuration for histogram computation on GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HistogramConfig {
    /// Number of bins
    pub bin_count: u32,
    /// Minimum value for binning range
    pub min_value: f32,
    /// Maximum value for binning range
    pub max_value: f32,
    /// 0 = counts, 1 = probabilities
    pub normalize: u32,
    /// Actual number of data elements (not buffer size)
    pub data_length: u32,
    /// Padding for 16-byte alignment (uniform buffer requirement)
    _padding: u32,
    _padding2: u32,
    _padding3: u32,
}

/// GPU compute pipeline for histogram generation
pub struct HistogramCompute {
    /// Compute pipeline for histogram binning
    histogram_pipeline: Option<wgpu::ComputePipeline>,
    /// Input data buffer
    data_buffer: Option<wgpu::Buffer>,
    /// Output bins buffer (atomic u32 array)
    bins_buffer: Option<wgpu::Buffer>,
    /// Configuration uniform buffer
    config_buffer: Option<wgpu::Buffer>,
    /// Maximum number of elements
    #[allow(dead_code)]
    max_elements: usize,
    /// Maximum number of bins
    max_bins: usize,
    /// Device and queue references
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
}

impl HistogramCompute {
    /// Create a new histogram compute system
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        max_elements: usize,
        max_bins: usize,
    ) -> GupResult<Self> {
        let histogram_pipeline = Self::create_histogram_pipeline(device).await?;

        let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_data"),
            size: (max_elements * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bins_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_bins"),
            size: (max_bins * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_config"),
            size: std::mem::size_of::<HistogramConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            histogram_pipeline: Some(histogram_pipeline),
            data_buffer: Some(data_buffer),
            bins_buffer: Some(bins_buffer),
            config_buffer: Some(config_buffer),
            max_elements,
            max_bins,
            device: Some(Arc::new(device.clone())),
            queue: Some(Arc::new(queue.clone())),
        })
    }

    /// Create compute pipeline for histogram generation
    async fn create_histogram_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("../shaders/histogram.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("histogram_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("histogram_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_histogram"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Compute histogram for a dataset
    pub async fn compute_histogram(
        &self,
        data: &[f32],
        bin_count: usize,
        min_value: f32,
        max_value: f32,
        normalize: bool,
    ) -> GupResult<HistogramResult> {
        if data.is_empty() {
            return Ok(HistogramResult {
                bins: vec![0; bin_count],
                edges: vec![0.0; bin_count + 1],
                min: min_value,
                max: max_value,
                count: 0,
            });
        }

        if bin_count > self.max_bins {
            return Err(GupError::buffer_error(format!(
                "Requested {} bins exceeds maximum of {}",
                bin_count, self.max_bins
            )));
        }

        let device = self.device.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute device not initialized".to_string(),
            )
        })?;
        let queue = self.queue.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute queue not initialized".to_string(),
            )
        })?;
        let data_buffer = self.data_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute data_buffer not initialized".to_string(),
            )
        })?;
        let bins_buffer = self.bins_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute bins_buffer not initialized".to_string(),
            )
        })?;
        let config_buffer = self.config_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute config_buffer not initialized".to_string(),
            )
        })?;

        // Upload data to GPU
        queue.write_buffer(data_buffer, 0, bytemuck::cast_slice(data));

        // Clear bins buffer
        let zero_bins = vec![0u32; bin_count];
        queue.write_buffer(bins_buffer, 0, bytemuck::cast_slice(&zero_bins));

        // Upload configuration
        let config = HistogramConfig {
            bin_count: bin_count as u32,
            min_value,
            max_value,
            normalize: if normalize { 1 } else { 0 },
            data_length: data.len() as u32,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
        };
        queue.write_buffer(config_buffer, 0, bytemuck::bytes_of(&config));

        // Create bind group
        let pipeline = self.histogram_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute pipeline not initialized".to_string(),
            )
        })?;
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("histogram_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bins_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("histogram_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("histogram_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            let workgroup_count = (data.len() as u32).div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Read back results
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_staging"),
            size: (bin_count * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(
            bins_buffer,
            0,
            &staging_buffer,
            0,
            (bin_count * std::mem::size_of::<u32>()) as u64,
        );

        queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });

        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        receiver
            .await
            .map_err(|_| GupError::webgpu_error("Failed to receive buffer mapping".to_string()))?
            .map_err(|e| GupError::webgpu_error(format!("Failed to map buffer: {:?}", e)))?;

        let buffer_data = buffer_slice.get_mapped_range();
        let bins: Vec<u32> = bytemuck::cast_slice(&buffer_data).to_vec();
        drop(buffer_data);
        staging_buffer.unmap();

        // Compute bin edges
        let range = max_value - min_value;
        let step = range / bin_count as f32;
        let edges: Vec<f32> = (0..=bin_count)
            .map(|i| min_value + i as f32 * step)
            .collect();

        Ok(HistogramResult {
            bins,
            edges,
            min: min_value,
            max: max_value,
            count: data.len(),
        })
    }
}

/// Mean calculation shader function - computes average of dataset
#[derive(Clone, Debug)]
pub struct Mean {
    /// Data values to compute mean over
    pub values: Vec<f32>,
}

impl Mean {
    /// Creates a new mean computation from the given values.
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute mean on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.values.iter().sum();
        sum / self.values.len() as f32
    }
}

/// Standard deviation shader function
#[derive(Clone, Debug)]
pub struct StandardDeviation {
    /// Data values to compute std dev over
    pub values: Vec<f32>,
}

impl StandardDeviation {
    /// Creates a new standard deviation computation from the given values.
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute standard deviation on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = Mean::new(self.values.clone()).compute_cpu();
        let variance: f32 = self
            .values
            .iter()
            .map(|v| {
                let diff = v - mean;
                diff * diff
            })
            .sum::<f32>()
            / self.values.len() as f32;
        variance.sqrt()
    }
}

/// Min/Max aggregation shader function
#[derive(Clone, Debug)]
pub struct MinMax {
    /// Data values to find min/max over
    pub values: Vec<f32>,
}

impl MinMax {
    /// Creates a new min/max computation from the given values.
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute min and max on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> (f32, f32) {
        if self.values.is_empty() {
            return (0.0, 0.0);
        }
        let min = self.values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max = self.values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        (min, max)
    }
}

/// Percentile calculation shader function
#[derive(Clone, Debug)]
pub struct Percentile {
    /// Data values to compute percentile over
    pub values: Vec<f32>,
    /// Percentile to compute (0.0 to 1.0)
    pub percentile: f32,
}

impl Percentile {
    /// Creates a new percentile computation from the given values and percentile.
    pub fn new(values: Vec<f32>, percentile: f32) -> Self {
        Self { values, percentile }
    }

    /// Compute percentile on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let index = (self.percentile * (sorted.len() - 1) as f32) as usize;
        sorted[index]
    }
}

/// Binning strategy for histogram generation
#[derive(Clone, Debug, PartialEq)]
pub enum BinningStrategy {
    /// Equal-width bins across the data range
    EqualWidth,
    /// Equal-frequency bins (each bin has approximately same count)
    EqualFrequency,
}

/// Histogram generation shader function
#[derive(Clone, Debug)]
pub struct Histogram {
    /// Data values to compute histogram over
    pub values: Vec<f32>,
    /// Number of bins
    pub bin_count: usize,
    /// Custom bin edges (if None, will auto-detect from min/max)
    pub custom_edges: Option<Vec<f32>>,
    /// Binning strategy
    pub strategy: BinningStrategy,
    /// Whether to normalize to probabilities
    pub normalize: bool,
}

impl Histogram {
    /// Create a new histogram with equal-width bins
    pub fn new(values: Vec<f32>, bin_count: usize) -> Self {
        Self {
            values,
            bin_count,
            custom_edges: None,
            strategy: BinningStrategy::EqualWidth,
            normalize: false,
        }
    }

    /// Set custom bin edges
    pub fn with_edges(mut self, edges: Vec<f32>) -> Self {
        self.custom_edges = Some(edges);
        self
    }

    /// Set binning strategy
    pub fn with_strategy(mut self, strategy: BinningStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Enable probability normalization
    pub fn with_normalization(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Compute histogram on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> HistogramResult {
        if self.values.is_empty() {
            return HistogramResult {
                bins: vec![0; self.bin_count],
                edges: vec![0.0; self.bin_count + 1],
                min: 0.0,
                max: 0.0,
                count: 0,
            };
        }

        // Determine bin edges
        let (min, max) = if let Some(ref edges) = self.custom_edges {
            (*edges.first().unwrap(), *edges.last().unwrap())
        } else {
            let min = self.values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = self.values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            (min, max)
        };

        let edges = self.compute_bin_edges(min, max);
        let mut bins = vec![0u32; self.bin_count];

        // Bin the data
        for &value in &self.values {
            if value < min || value > max {
                continue;
            }
            let range = max - min;
            if range == 0.0 {
                bins[0] += 1;
            } else {
                let normalized = (value - min) / range;
                let bin_index = (normalized * self.bin_count as f32) as usize;
                let bin_index = bin_index.min(self.bin_count - 1);
                bins[bin_index] += 1;
            }
        }

        // Normalize if requested
        if self.normalize {
            let total: u32 = bins.iter().sum();
            if total > 0 {
                // Convert to f32 for normalization, then back to u32 (storing as bits)
                bins = bins
                    .iter()
                    .map(|&count| {
                        let prob = count as f32 / total as f32;
                        prob.to_bits()
                    })
                    .collect();
            }
        }

        HistogramResult {
            bins,
            edges,
            min,
            max,
            count: self.values.len(),
        }
    }

    /// Compute bin edges based on strategy
    fn compute_bin_edges(&self, min: f32, max: f32) -> Vec<f32> {
        if let Some(ref edges) = self.custom_edges {
            return edges.clone();
        }

        match self.strategy {
            BinningStrategy::EqualWidth => {
                let range = max - min;
                let step = range / self.bin_count as f32;
                (0..=self.bin_count)
                    .map(|i| min + i as f32 * step)
                    .collect()
            }
            BinningStrategy::EqualFrequency => {
                // For equal frequency, we need to sort and find quantiles
                let mut sorted = self.values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                let mut edges = Vec::with_capacity(self.bin_count + 1);
                edges.push(min);

                for i in 1..self.bin_count {
                    let quantile = i as f32 / self.bin_count as f32;
                    let index = (quantile * (sorted.len() - 1) as f32) as usize;
                    edges.push(sorted[index]);
                }

                edges.push(max);
                edges
            }
        }
    }
}

/// Result of histogram computation
#[derive(Clone, Debug)]
pub struct HistogramResult {
    /// Bin counts (or normalized probabilities if normalize=true)
    pub bins: Vec<u32>,
    /// Bin edges (length = bin_count + 1)
    pub edges: Vec<f32>,
    /// Minimum value in dataset
    pub min: f32,
    /// Maximum value in dataset
    pub max: f32,
    /// Total count of values
    pub count: usize,
}

impl HistogramResult {
    /// Get bin counts as f32 (handles normalized histograms)
    pub fn bin_values(&self) -> Vec<f32> {
        self.bins.iter().map(|&bits| f32::from_bits(bits)).collect()
    }

    /// Check if this is a normalized histogram
    pub fn is_normalized(&self) -> bool {
        // If any bin value is less than 1.0, it's likely normalized
        self.bins
            .iter()
            .any(|&bits| f32::from_bits(bits) < 1.0 && bits != 0)
    }
}

/// Streaming statistical aggregation for datasets larger than GPU memory
///
/// Uses Welford's online algorithm for numerically stable variance computation
/// and processes data in configurable chunks to handle arbitrarily large datasets.
///
/// # Examples
///
/// ```rust,ignore
/// use gup::StreamingStatistics;
///
/// // Process 1 billion points in chunks
/// let mut stats = StreamingStatistics::with_chunk_size(1_000_000);
///
/// for chunk in data_source.chunks() {
///     stats.push_chunk(&chunk);
/// }
///
/// let result = stats.finalize();
/// println!("Mean: {}, Std Dev: {}", result.mean, result.std_dev);
/// ```
#[derive(Clone, Debug)]
pub struct StreamingStatistics {
    /// Running count of elements processed
    count: u64,
    /// Running mean (Welford's algorithm)
    mean: f64,
    /// Running M2 value for variance computation (Welford's algorithm)
    m2: f64,
    /// Running minimum value
    min: f32,
    /// Running maximum value
    max: f32,
    /// Running sum (for verification)
    sum: f64,
    /// Chunk size for processing (default: 1M elements)
    chunk_size: usize,
    /// Total chunks processed
    chunks_processed: usize,
}

impl Default for StreamingStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Callback type for progress reporting in streaming statistics: (processed, total).
type ProgressCallback = Box<dyn Fn(usize, Option<usize>)>;

impl StreamingStatistics {
    /// Create a new streaming statistics aggregator with default chunk size (1M elements)
    pub fn new() -> Self {
        Self::with_chunk_size(1_000_000)
    }

    /// Create a new streaming statistics aggregator with custom chunk size
    ///
    /// # Arguments
    /// * `chunk_size` - Number of elements to process per chunk (affects GPU buffer size)
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            sum: 0.0,
            chunk_size,
            chunks_processed: 0,
        }
    }

    /// Push a single value into the stream (uses Welford's algorithm)
    ///
    /// # Arguments
    /// * `value` - Single f32 value to aggregate
    pub fn push(&mut self, value: f32) {
        self.count += 1;
        let value_f64 = value as f64;

        // Update sum
        self.sum += value_f64;

        // Update min/max
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Welford's online algorithm for mean and variance
        let delta = value_f64 - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value_f64 - self.mean;
        self.m2 += delta * delta2;
    }

    /// Push a chunk of values into the stream
    ///
    /// # Arguments
    /// * `chunk` - Slice of f32 values to aggregate
    pub fn push_chunk(&mut self, chunk: &[f32]) {
        for &value in chunk {
            self.push(value);
        }
        self.chunks_processed += 1;
    }

    /// Process data from an iterator in chunks
    ///
    /// This is the recommended way to process large datasets as it handles
    /// chunking automatically and provides progress reporting.
    ///
    /// # Arguments
    /// * `data` - Iterator providing f32 values
    /// * `progress_callback` - Optional callback for progress reporting (processed, total)
    pub fn process_iter<I>(&mut self, data: I, progress_callback: Option<ProgressCallback>)
    where
        I: Iterator<Item = f32>,
    {
        let mut chunk = Vec::with_capacity(self.chunk_size);
        let mut total_processed = 0;

        for value in data {
            chunk.push(value);
            if chunk.len() >= self.chunk_size {
                self.push_chunk(&chunk);
                total_processed += chunk.len();
                chunk.clear();

                if let Some(ref callback) = progress_callback {
                    callback(total_processed, None);
                }
            }
        }

        // Process remaining values
        if !chunk.is_empty() {
            self.push_chunk(&chunk);
            total_processed += chunk.len();

            if let Some(ref callback) = progress_callback {
                callback(total_processed, None);
            }
        }
    }

    /// Process data from a slice in chunks with progress reporting
    ///
    /// # Arguments
    /// * `data` - Slice of f32 values to process
    /// * `progress_callback` - Optional callback for progress reporting (processed, total)
    pub fn process_slice(
        &mut self,
        data: &[f32],
        progress_callback: Option<Box<dyn Fn(usize, usize)>>,
    ) {
        let total = data.len();
        let mut processed = 0;

        for chunk in data.chunks(self.chunk_size) {
            self.push_chunk(chunk);
            processed += chunk.len();

            if let Some(ref callback) = progress_callback {
                callback(processed, total);
            }
        }
    }

    /// Merge statistics from another streaming aggregator
    ///
    /// This enables parallel processing where multiple StreamingStatistics
    /// instances process different parts of the dataset and then merge.
    ///
    /// # Arguments
    /// * `other` - Another StreamingStatistics instance to merge
    pub fn merge(&mut self, other: &StreamingStatistics) {
        if other.count == 0 {
            return;
        }

        if self.count == 0 {
            *self = other.clone();
            return;
        }

        // Merge using parallel algorithm
        let total_count = self.count + other.count;
        let delta = other.mean - self.mean;

        // Update mean
        let new_mean =
            (self.count as f64 * self.mean + other.count as f64 * other.mean) / total_count as f64;

        // Update M2 (variance component)
        let new_m2 = self.m2
            + other.m2
            + delta * delta * (self.count as f64 * other.count as f64) / total_count as f64;

        self.mean = new_mean;
        self.m2 = new_m2;
        self.count = total_count;
        self.sum += other.sum;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.chunks_processed += other.chunks_processed;
    }

    /// Get current count of processed elements
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get current mean (available before finalization)
    pub fn mean(&self) -> f32 {
        self.mean as f32
    }

    /// Get current variance (available before finalization)
    pub fn variance(&self) -> f32 {
        if self.count < 2 {
            0.0
        } else {
            (self.m2 / self.count as f64) as f32
        }
    }

    /// Get current standard deviation (available before finalization)
    pub fn std_dev(&self) -> f32 {
        self.variance().sqrt()
    }

    /// Get current min/max (available before finalization)
    pub fn min_max(&self) -> (f32, f32) {
        (self.min, self.max)
    }

    /// Finalize and get complete statistics result
    ///
    /// Returns a `StatisticsResult` compatible with GPU compute results.
    pub fn finalize(&self) -> StatisticsResult {
        let variance = self.variance();
        let std_dev = variance.sqrt();

        StatisticsResult {
            count: self.count as u32,
            sum: self.sum as f32,
            min: self.min,
            max: self.max,
            mean: self.mean as f32,
            variance,
            std_dev,
            _padding: 0,
        }
    }

    /// Reset the aggregator to initial state
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
        self.min = f32::INFINITY;
        self.max = f32::NEG_INFINITY;
        self.sum = 0.0;
        self.chunks_processed = 0;
    }

    /// Get number of chunks processed
    pub fn chunks_processed(&self) -> usize {
        self.chunks_processed
    }

    /// Get configured chunk size
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

/// Kernel function for density estimation
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum KernelFunction {
    /// Gaussian kernel (most common) - K(u) = (1/√(2π)) * exp(-u²/2)
    Gaussian,
    /// Epanechnikov kernel (optimal for MSE) - K(u) = (3/4) * (1 - u²) for |u| ≤ 1
    Epanechnikov,
    /// Uniform kernel (rectangular) - K(u) = 1/2 for |u| ≤ 1
    Uniform,
    /// Triangular kernel - K(u) = (1 - |u|) for |u| ≤ 1
    Triangular,
}

impl KernelFunction {
    /// Evaluate the kernel function at point u
    pub fn evaluate(&self, u: f32) -> f32 {
        match self {
            KernelFunction::Gaussian => {
                let factor = 1.0 / (2.0 * std::f32::consts::PI).sqrt();
                factor * (-0.5 * u * u).exp()
            }
            KernelFunction::Epanechnikov => {
                if u.abs() <= 1.0 {
                    0.75 * (1.0 - u * u)
                } else {
                    0.0
                }
            }
            KernelFunction::Uniform => {
                if u.abs() <= 1.0 {
                    0.5
                } else {
                    0.0
                }
            }
            KernelFunction::Triangular => {
                let abs_u = u.abs();
                if abs_u <= 1.0 { 1.0 - abs_u } else { 0.0 }
            }
        }
    }

    /// Get the WGSL function code for this kernel
    #[allow(dead_code)]
    fn wgsl_code(&self) -> &'static str {
        match self {
            KernelFunction::Gaussian => {
                r#"
fn gaussian_kernel(u: f32) -> f32 {
    let factor = 1.0 / sqrt(2.0 * 3.14159265359);
    return factor * exp(-0.5 * u * u);
}
"#
            }
            KernelFunction::Epanechnikov => {
                r#"
fn epanechnikov_kernel(u: f32) -> f32 {
    let abs_u = abs(u);
    if (abs_u <= 1.0) {
        return 0.75 * (1.0 - u * u);
    } else {
        return 0.0;
    }
}
"#
            }
            KernelFunction::Uniform => {
                r#"
fn uniform_kernel(u: f32) -> f32 {
    if (abs(u) <= 1.0) {
        return 0.5;
    } else {
        return 0.0;
    }
}
"#
            }
            KernelFunction::Triangular => {
                r#"
fn triangular_kernel(u: f32) -> f32 {
    let abs_u = abs(u);
    if (abs_u <= 1.0) {
        return 1.0 - abs_u;
    } else {
        return 0.0;
    }
}
"#
            }
        }
    }

    /// Get the WGSL function name for this kernel
    #[allow(dead_code)]
    fn wgsl_function_name(&self) -> &'static str {
        match self {
            KernelFunction::Gaussian => "gaussian_kernel",
            KernelFunction::Epanechnikov => "epanechnikov_kernel",
            KernelFunction::Uniform => "uniform_kernel",
            KernelFunction::Triangular => "triangular_kernel",
        }
    }
}

/// Bandwidth estimation method for kernel density estimation
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum BandwidthMethod {
    /// Manual bandwidth specification
    Manual(f32),
    /// Silverman's rule of thumb - bandwidth = 0.9 * min(std, IQR/1.34) * n^(-1/5)
    Silverman,
    /// Scott's rule - bandwidth = std * n^(-1/5)
    Scott,
}

/// 1D Kernel Density Estimation
#[derive(Clone, Debug)]
pub struct KernelDensity1D {
    /// Sample data points
    pub samples: Vec<f32>,
    /// Kernel function to use
    pub kernel: KernelFunction,
    /// Bandwidth (smoothing parameter)
    pub bandwidth: BandwidthMethod,
    /// Evaluation points (if None, will auto-generate)
    pub eval_points: Option<Vec<f32>>,
    /// Number of evaluation points for auto-generation
    pub n_eval_points: usize,
}

impl KernelDensity1D {
    /// Create a new 1D KDE with default settings (Gaussian kernel, Silverman bandwidth, 1000 eval points)
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            kernel: KernelFunction::Gaussian,
            bandwidth: BandwidthMethod::Silverman,
            eval_points: None,
            n_eval_points: 1000,
        }
    }

    /// Set the kernel function
    pub fn with_kernel(mut self, kernel: KernelFunction) -> Self {
        self.kernel = kernel;
        self
    }

    /// Set manual bandwidth
    pub fn with_bandwidth(mut self, bandwidth: f32) -> Self {
        self.bandwidth = BandwidthMethod::Manual(bandwidth);
        self
    }

    /// Set bandwidth method
    pub fn with_bandwidth_method(mut self, method: BandwidthMethod) -> Self {
        self.bandwidth = method;
        self
    }

    /// Set custom evaluation points
    pub fn with_eval_points(mut self, points: Vec<f32>) -> Self {
        self.eval_points = Some(points);
        self
    }

    /// Set number of evaluation points for auto-generation
    pub fn with_n_eval_points(mut self, n: usize) -> Self {
        self.n_eval_points = n;
        self
    }

    /// Estimate optimal bandwidth using the specified method
    fn estimate_bandwidth(&self) -> f32 {
        match self.bandwidth {
            BandwidthMethod::Manual(bw) => bw,
            BandwidthMethod::Silverman => {
                // Silverman's rule: 0.9 * min(std, IQR/1.34) * n^(-1/5)
                let n = self.samples.len() as f32;
                let std_dev = StandardDeviation::new(self.samples.clone()).compute_cpu();

                // Compute IQR (interquartile range)
                let q1 = Percentile::new(self.samples.clone(), 0.25).compute_cpu();
                let q3 = Percentile::new(self.samples.clone(), 0.75).compute_cpu();
                let iqr = q3 - q1;

                let scale = std_dev.min(iqr / 1.34);
                0.9 * scale * n.powf(-0.2)
            }
            BandwidthMethod::Scott => {
                // Scott's rule: std * n^(-1/5)
                let n = self.samples.len() as f32;
                let std_dev = StandardDeviation::new(self.samples.clone()).compute_cpu();
                std_dev * n.powf(-0.2)
            }
        }
    }

    /// Generate evaluation points across the data range
    fn generate_eval_points(&self) -> Vec<f32> {
        if let Some(ref points) = self.eval_points {
            return points.clone();
        }

        let (min, max) = MinMax::new(self.samples.clone()).compute_cpu();
        let bandwidth = self.estimate_bandwidth();

        // Extend range slightly beyond data bounds
        let padding = bandwidth * 3.0;
        let start = min - padding;
        let end = max + padding;
        let step = (end - start) / (self.n_eval_points - 1) as f32;

        (0..self.n_eval_points)
            .map(|i| start + i as f32 * step)
            .collect()
    }

    /// Compute KDE on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> KDEResult {
        if self.samples.is_empty() {
            return KDEResult {
                densities: vec![],
                eval_points: vec![],
                bandwidth: 0.0,
                kernel: self.kernel,
            };
        }

        let bandwidth = self.estimate_bandwidth();
        let eval_points = self.generate_eval_points();
        let n = self.samples.len() as f32;

        // Compute density at each evaluation point
        let densities: Vec<f32> = eval_points
            .iter()
            .map(|&x| {
                // Sum kernel contributions from all samples
                let sum: f32 = self
                    .samples
                    .iter()
                    .map(|&xi| {
                        let u = (x - xi) / bandwidth;
                        self.kernel.evaluate(u)
                    })
                    .sum();

                // Normalize by sample count and bandwidth
                sum / (n * bandwidth)
            })
            .collect();

        KDEResult {
            densities,
            eval_points,
            bandwidth,
            kernel: self.kernel,
        }
    }
}

/// 2D Kernel Density Estimation
#[derive(Clone, Debug)]
pub struct KernelDensity2D {
    /// Sample data points (x, y)
    pub samples: Vec<(f32, f32)>,
    /// Kernel function to use
    pub kernel: KernelFunction,
    /// Bandwidth for x dimension
    pub bandwidth_x: BandwidthMethod,
    /// Bandwidth for y dimension
    pub bandwidth_y: BandwidthMethod,
    /// Evaluation grid points (if None, will auto-generate)
    pub eval_grid: Option<(Vec<f32>, Vec<f32>)>,
    /// Number of evaluation points per dimension for auto-generation
    pub n_eval_points: usize,
}

impl KernelDensity2D {
    /// Create a new 2D KDE with default settings
    pub fn new(samples: Vec<(f32, f32)>) -> Self {
        Self {
            samples,
            kernel: KernelFunction::Gaussian,
            bandwidth_x: BandwidthMethod::Silverman,
            bandwidth_y: BandwidthMethod::Silverman,
            eval_grid: None,
            n_eval_points: 100, // 100x100 = 10,000 points
        }
    }

    /// Set the kernel function
    pub fn with_kernel(mut self, kernel: KernelFunction) -> Self {
        self.kernel = kernel;
        self
    }

    /// Set manual bandwidth for both dimensions
    pub fn with_bandwidth(mut self, bandwidth: f32) -> Self {
        self.bandwidth_x = BandwidthMethod::Manual(bandwidth);
        self.bandwidth_y = BandwidthMethod::Manual(bandwidth);
        self
    }

    /// Set bandwidths separately for x and y
    pub fn with_bandwidths(mut self, bandwidth_x: f32, bandwidth_y: f32) -> Self {
        self.bandwidth_x = BandwidthMethod::Manual(bandwidth_x);
        self.bandwidth_y = BandwidthMethod::Manual(bandwidth_y);
        self
    }

    /// Set custom evaluation grid
    pub fn with_eval_grid(mut self, x_points: Vec<f32>, y_points: Vec<f32>) -> Self {
        self.eval_grid = Some((x_points, y_points));
        self
    }

    /// Set number of evaluation points per dimension
    pub fn with_n_eval_points(mut self, n: usize) -> Self {
        self.n_eval_points = n;
        self
    }

    /// Estimate bandwidth for a single dimension
    fn estimate_bandwidth_dim(&self, values: &[f32], method: &BandwidthMethod) -> f32 {
        match method {
            BandwidthMethod::Manual(bw) => *bw,
            BandwidthMethod::Silverman => {
                let n = values.len() as f32;
                let std_dev = StandardDeviation::new(values.to_vec()).compute_cpu();
                let q1 = Percentile::new(values.to_vec(), 0.25).compute_cpu();
                let q3 = Percentile::new(values.to_vec(), 0.75).compute_cpu();
                let iqr = q3 - q1;
                let scale = std_dev.min(iqr / 1.34);
                0.9 * scale * n.powf(-0.2)
            }
            BandwidthMethod::Scott => {
                let n = values.len() as f32;
                let std_dev = StandardDeviation::new(values.to_vec()).compute_cpu();
                std_dev * n.powf(-0.2)
            }
        }
    }

    /// Generate evaluation grid across the data range
    fn generate_eval_grid(&self) -> (Vec<f32>, Vec<f32>, f32, f32) {
        if let Some((ref x_points, ref y_points)) = self.eval_grid {
            let x_values: Vec<f32> = self.samples.iter().map(|(x, _)| *x).collect();
            let y_values: Vec<f32> = self.samples.iter().map(|(_, y)| *y).collect();
            let bw_x = self.estimate_bandwidth_dim(&x_values, &self.bandwidth_x);
            let bw_y = self.estimate_bandwidth_dim(&y_values, &self.bandwidth_y);
            return (x_points.clone(), y_points.clone(), bw_x, bw_y);
        }

        let x_values: Vec<f32> = self.samples.iter().map(|(x, _)| *x).collect();
        let y_values: Vec<f32> = self.samples.iter().map(|(_, y)| *y).collect();

        let (x_min, x_max) = MinMax::new(x_values.clone()).compute_cpu();
        let (y_min, y_max) = MinMax::new(y_values.clone()).compute_cpu();

        let bw_x = self.estimate_bandwidth_dim(&x_values, &self.bandwidth_x);
        let bw_y = self.estimate_bandwidth_dim(&y_values, &self.bandwidth_y);

        // Extend range slightly beyond data bounds
        let x_padding = bw_x * 3.0;
        let y_padding = bw_y * 3.0;

        let x_start = x_min - x_padding;
        let x_end = x_max + x_padding;
        let x_step = (x_end - x_start) / (self.n_eval_points - 1) as f32;

        let y_start = y_min - y_padding;
        let y_end = y_max + y_padding;
        let y_step = (y_end - y_start) / (self.n_eval_points - 1) as f32;

        let x_points: Vec<f32> = (0..self.n_eval_points)
            .map(|i| x_start + i as f32 * x_step)
            .collect();

        let y_points: Vec<f32> = (0..self.n_eval_points)
            .map(|i| y_start + i as f32 * y_step)
            .collect();

        (x_points, y_points, bw_x, bw_y)
    }

    /// Compute 2D KDE on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> KDEResult2D {
        if self.samples.is_empty() {
            return KDEResult2D {
                densities: vec![],
                x_points: vec![],
                y_points: vec![],
                bandwidth_x: 0.0,
                bandwidth_y: 0.0,
                kernel: self.kernel,
            };
        }

        let (x_points, y_points, bw_x, bw_y) = self.generate_eval_grid();
        let n = self.samples.len() as f32;

        // Compute density at each grid point
        let mut densities = Vec::with_capacity(x_points.len() * y_points.len());

        for &y in &y_points {
            for &x in &x_points {
                // Sum kernel contributions from all samples
                let sum: f32 = self
                    .samples
                    .iter()
                    .map(|&(xi, yi)| {
                        let ux = (x - xi) / bw_x;
                        let uy = (y - yi) / bw_y;
                        // Product kernel: K(ux, uy) = K(ux) * K(uy)
                        self.kernel.evaluate(ux) * self.kernel.evaluate(uy)
                    })
                    .sum();

                // Normalize by sample count and bandwidth product
                densities.push(sum / (n * bw_x * bw_y));
            }
        }

        KDEResult2D {
            densities,
            x_points,
            y_points,
            bandwidth_x: bw_x,
            bandwidth_y: bw_y,
            kernel: self.kernel,
        }
    }
}

/// Result of 1D kernel density estimation
#[derive(Clone, Debug)]
pub struct KDEResult {
    /// Density values at evaluation points
    pub densities: Vec<f32>,
    /// Evaluation points
    pub eval_points: Vec<f32>,
    /// Bandwidth used
    pub bandwidth: f32,
    /// Kernel function used
    pub kernel: KernelFunction,
}

impl KDEResult {
    /// Check if density is properly normalized (integral ≈ 1.0)
    pub fn is_normalized(&self) -> bool {
        if self.densities.is_empty() {
            return false;
        }

        // Numerical integration using trapezoidal rule
        let integral: f32 = self
            .densities
            .windows(2)
            .zip(self.eval_points.windows(2))
            .map(|(d, x)| {
                let dx = x[1] - x[0];
                0.5 * (d[0] + d[1]) * dx
            })
            .sum();

        // Allow 10% tolerance for numerical integration error
        (integral - 1.0).abs() < 0.1
    }

    /// Find the peak density value
    pub fn peak_density(&self) -> f32 {
        self.densities
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    /// Find the mode (point with maximum density)
    pub fn mode(&self) -> Option<f32> {
        if self.densities.is_empty() {
            return None;
        }

        let max_idx = self
            .densities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?
            .0;

        Some(self.eval_points[max_idx])
    }
}

/// Result of 2D kernel density estimation
#[derive(Clone, Debug)]
pub struct KDEResult2D {
    /// Density values at grid points (row-major order: y varies faster)
    pub densities: Vec<f32>,
    /// X-axis evaluation points
    pub x_points: Vec<f32>,
    /// Y-axis evaluation points
    pub y_points: Vec<f32>,
    /// Bandwidth used for x dimension
    pub bandwidth_x: f32,
    /// Bandwidth used for y dimension
    pub bandwidth_y: f32,
    /// Kernel function used
    pub kernel: KernelFunction,
}

impl KDEResult2D {
    /// Get density at grid position (i, j)
    pub fn density_at(&self, x_idx: usize, y_idx: usize) -> Option<f32> {
        if x_idx >= self.x_points.len() || y_idx >= self.y_points.len() {
            return None;
        }
        let idx = y_idx * self.x_points.len() + x_idx;
        self.densities.get(idx).copied()
    }

    /// Find the peak density value
    pub fn peak_density(&self) -> f32 {
        self.densities
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    /// Find the mode (point with maximum density)
    pub fn mode(&self) -> Option<(f32, f32)> {
        if self.densities.is_empty() {
            return None;
        }

        let max_idx = self
            .densities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?
            .0;

        let y_idx = max_idx / self.x_points.len();
        let x_idx = max_idx % self.x_points.len();

        Some((self.x_points[x_idx], self.y_points[y_idx]))
    }
}
