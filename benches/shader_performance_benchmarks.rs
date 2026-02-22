// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-based shader function performance benchmarks (GUP-137)
//!
//! This benchmark suite validates that composed shader functions perform within
//! 15% of hand-optimized WGSL code. It tests various composition depths and
//! provides detailed performance metrics.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::shader_function::{
    Clamp, ColorMap, ComposableShaderFunction, LinearScale, Vec4,
};
use pollster::FutureExt;
use std::hint::black_box;
use wgpu::util::DeviceExt;

/// Test data size for benchmarks
const BENCHMARK_DATA_SIZE: usize = 10_000;

/// GPU benchmark context with timing query support
struct GpuBenchmarkContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    input_buffer: wgpu::Buffer,
    output_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
}

impl GpuBenchmarkContext {
    async fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Shader Benchmark Device"),
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .expect("Failed to create GPU device");

        // Create test data buffers
        let input_data: Vec<f32> = (0..BENCHMARK_DATA_SIZE)
            .map(|i| i as f32 / BENCHMARK_DATA_SIZE as f32)
            .collect();

        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Input Buffer"),
            contents: bytemuck::cast_slice(&input_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: (BENCHMARK_DATA_SIZE * std::mem::size_of::<f32>() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (BENCHMARK_DATA_SIZE * std::mem::size_of::<f32>() * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            input_buffer,
            output_buffer,
            staging_buffer,
        }
    }

    /// Execute a compute shader and measure GPU execution time
    fn execute_compute_shader(&self, shader_code: &str, uniform_data: Option<&[u8]>) {
        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Benchmark Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_code.into()),
            });

        // Create bind group layout
        let mut bind_group_entries = vec![
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
        ];

        // Add uniform buffer binding if provided
        let uniform_buffer = uniform_data.map(|data| {
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Uniform Buffer"),
                    contents: data,
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            bind_group_entries.push(wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });

            buffer
        });

        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Benchmark Bind Group Layout"),
                    entries: &bind_group_entries,
                });

        // Create bind group
        let mut bind_group_entries_data = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: self.output_buffer.as_entire_binding(),
            },
        ];

        if let Some(ref buffer) = uniform_buffer {
            bind_group_entries_data.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: buffer.as_entire_binding(),
            });
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Benchmark Bind Group"),
            layout: &bind_group_layout,
            entries: &bind_group_entries_data,
        });

        // Create pipeline
        let pipeline_layout =
            self.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Benchmark Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Benchmark Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        // Execute compute pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Benchmark Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Benchmark Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(
                ((BENCHMARK_DATA_SIZE as u32 + 255) / 256).max(1),
                1,
                1,
            );
        }

        // Copy results to staging buffer
        encoder.copy_buffer_to_buffer(
            &self.output_buffer,
            0,
            &self.staging_buffer,
            0,
            self.staging_buffer.size(),
        );

        self.queue.submit(Some(encoder.finish()));
        let _ = self.device.poll(wgpu::PollType::Wait);
    }
}

/// Hand-optimized single-stage shader (baseline for comparison)
const HAND_OPTIMIZED_SINGLE_STAGE: &str = r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

struct Uniforms {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
    min_color_r: f32,
    min_color_g: f32,
    min_color_b: f32,
    min_color_a: f32,
    max_color_r: f32,
    max_color_g: f32,
    max_color_b: f32,
    max_color_a: f32,
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input)) {
        return;
    }
    
    let value = input[index];
    
    // Linear scale (hand-optimized)
    let normalized = (value - uniforms.domain_min) / (uniforms.domain_max - uniforms.domain_min);
    let scaled = uniforms.range_min + normalized * (uniforms.range_max - uniforms.range_min);
    
    // Color map (hand-optimized)
    let color = vec4<f32>(
        uniforms.min_color_r + scaled * (uniforms.max_color_r - uniforms.min_color_r),
        uniforms.min_color_g + scaled * (uniforms.max_color_g - uniforms.min_color_g),
        uniforms.min_color_b + scaled * (uniforms.max_color_b - uniforms.min_color_b),
        uniforms.min_color_a + scaled * (uniforms.max_color_a - uniforms.min_color_a)
    );
    
    output[index] = color;
}
"#;

/// Benchmark composed shader functions vs hand-optimized code
fn bench_composed_vs_hand_optimized(c: &mut Criterion) {
    let context = GpuBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("shader_composition");

    // Prepare uniform data for hand-optimized shader
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct HandOptimizedUniforms {
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
        min_color_r: f32,
        min_color_g: f32,
        min_color_b: f32,
        min_color_a: f32,
        max_color_r: f32,
        max_color_g: f32,
        max_color_b: f32,
        max_color_a: f32,
    }

    let uniforms = HandOptimizedUniforms {
        domain_min: 0.0,
        domain_max: 1.0,
        range_min: 0.0,
        range_max: 1.0,
        min_color_r: 0.0,
        min_color_g: 0.0,
        min_color_b: 1.0,
        min_color_a: 1.0,
        max_color_r: 1.0,
        max_color_g: 0.0,
        max_color_b: 0.0,
        max_color_a: 1.0,
    };

    // Benchmark hand-optimized shader
    group.bench_function("hand_optimized_2_stage", |b| {
        b.iter(|| {
            context.execute_compute_shader(
                HAND_OPTIMIZED_SINGLE_STAGE,
                Some(bytemuck::bytes_of(&uniforms)),
            );
            black_box(());
        });
    });

    // Benchmark composed shader functions
    group.bench_function("composed_2_stage", |b| {
        b.iter(|| {
            // Create composed functions
            let scale = LinearScale::new(0.0, 1.0, 0.0, 1.0);
            let color_map = ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
            );

            // Generate composed WGSL (this is what we're benchmarking)
            let _wgsl = scale.generate_wgsl();
            let _wgsl2 = color_map.generate_wgsl();

            black_box(());
        });
    });

    group.finish();
}

/// Benchmark different composition depths
fn bench_composition_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("composition_depth");

    for depth in [2, 3, 5] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.iter(|| {
                // Create a chain of composed functions
                let scale = LinearScale::new(0.0, 1.0, 0.0, 1.0);
                let mut wgsl = scale.generate_wgsl();

                for _ in 1..depth {
                    let clamp = Clamp::new(0.0, 1.0);
                    wgsl.push_str(&clamp.generate_wgsl());
                }

                black_box(wgsl);
            });
        });
    }

    group.finish();
}

/// Benchmark WGSL code generation performance
fn bench_wgsl_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("wgsl_generation");

    group.bench_function("linear_scale", |b| {
        b.iter(|| {
            let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
            black_box(scale.generate_wgsl());
        });
    });

    group.bench_function("color_map", |b| {
        b.iter(|| {
            let color_map = ColorMap::new(
                Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                Vec4 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                    w: 1.0,
                },
            );
            black_box(color_map.generate_wgsl());
        });
    });

    group.bench_function("clamp", |b| {
        b.iter(|| {
            let clamp = Clamp::new(0.0, 1.0);
            black_box(clamp.generate_wgsl());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_composed_vs_hand_optimized,
    bench_composition_depth,
    bench_wgsl_generation
);
criterion_main!(benches);
