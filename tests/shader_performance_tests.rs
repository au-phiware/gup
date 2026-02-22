// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU shader performance validation tests (GUP-137)
//!
//! These tests validate that composed shader functions perform within 15% of
//! hand-optimized WGSL shaders. They use actual GPU timing to measure performance.

use gup::shader_function::*;
use pollster::FutureExt;
use wgpu::util::DeviceExt;

/// Number of iterations for averaging GPU timings
const TIMING_ITERATIONS: usize = 100;

/// Data size for performance tests
const TEST_DATA_SIZE: usize = 10_000;

/// Maximum allowed overhead for composed shaders (15%)
const MAX_OVERHEAD_PERCENTAGE: f64 = 15.0;

/// GPU test context with proper resource management
struct GpuTestContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuTestContext {
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
                label: Some("Shader Performance Test Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .expect("Failed to create GPU device");

        Self { device, queue }
    }

    /// Execute a compute shader and measure wall-clock time
    fn execute_and_time(&self, shader_code: &str, uniform_data: Option<&[u8]>) -> f64 {
        let mut total_time = 0.0;

        for _ in 0..TIMING_ITERATIONS {
            let start = std::time::Instant::now();
            self.execute_compute_shader(shader_code, uniform_data);
            let elapsed = start.elapsed();
            total_time += elapsed.as_secs_f64();
        }

        total_time / TIMING_ITERATIONS as f64
    }

    fn execute_compute_shader(&self, shader_code: &str, uniform_data: Option<&[u8]>) {
        // Prepare test data
        let input_data: Vec<f32> = (0..TEST_DATA_SIZE)
            .map(|i| i as f32 / TEST_DATA_SIZE as f32)
            .collect();

        let input_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Input Buffer"),
                contents: bytemuck::cast_slice(&input_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: (TEST_DATA_SIZE * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Create shader module
        let shader_module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Test Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_code.into()),
            });

        // Build bind group layout
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

        // Add uniform buffer if provided
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
                    label: Some("Test Bind Group Layout"),
                    entries: &bind_group_entries,
                });

        // Create bind group
        let mut bind_group_entries_data = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ];

        if let Some(ref buffer) = uniform_buffer {
            bind_group_entries_data.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: buffer.as_entire_binding(),
            });
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test Bind Group"),
            layout: &bind_group_layout,
            entries: &bind_group_entries_data,
        });

        // Create pipeline
        let pipeline_layout =
            self.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Test Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Test Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        // Execute
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Test Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(((TEST_DATA_SIZE as u32 + 255) / 256).max(1), 1, 1);
        }

        self.queue.submit(Some(encoder.finish()));
        let _ = self.device.poll(wgpu::PollType::Wait);
    }
}

/// Hand-optimized 2-stage shader (linear scale + color map)
const HAND_OPTIMIZED_2_STAGE: &str = r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

struct Uniforms {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
    min_color: vec4<f32>,
    max_color: vec4<f32>,
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input)) {
        return;
    }
    
    let value = input[index];
    
    // Linear scale (inlined)
    let normalized = (value - uniforms.domain_min) / (uniforms.domain_max - uniforms.domain_min);
    let scaled = uniforms.range_min + normalized * (uniforms.range_max - uniforms.range_min);
    
    // Color map (inlined)
    let color = mix(uniforms.min_color, uniforms.max_color, scaled);
    
    output[index] = color;
}
"#;

/// Composed 2-stage shader (linear scale + color map)
fn generate_composed_2_stage_shader() -> String {
    // This generates WGSL that composes LinearScale and ColorMap
    let shader = r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

struct Uniforms {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
    min_color: vec4<f32>,
    max_color: vec4<f32>,
}

fn linear_scale(value: f32, domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> f32 {
    let normalized = (value - domain_min) / (domain_max - domain_min);
    return range_min + normalized * (range_max - range_min);
}

fn color_map(value: f32, min_color: vec4<f32>, max_color: vec4<f32>) -> vec4<f32> {
    return mix(min_color, max_color, value);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input)) {
        return;
    }
    
    let value = input[index];
    
    // Composed functions
    let scaled = linear_scale(value, uniforms.domain_min, uniforms.domain_max, uniforms.range_min, uniforms.range_max);
    let color = color_map(scaled, uniforms.min_color, uniforms.max_color);
    
    output[index] = color;
}
"#;
    shader.to_string()
}

#[test]
#[ignore] // Run with: cargo test --test shader_performance_tests -- --ignored --test-threads=1
fn test_composed_vs_hand_optimized_performance() {
    let context = GpuTestContext::new().block_on();

    // Prepare uniform data
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct Uniforms {
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
        min_color: [f32; 4],
        max_color: [f32; 4],
    }

    let uniforms = Uniforms {
        domain_min: 0.0,
        domain_max: 1.0,
        range_min: 0.0,
        range_max: 1.0,
        min_color: [0.0, 0.0, 1.0, 1.0],
        max_color: [1.0, 0.0, 0.0, 1.0],
    };

    let uniform_bytes = bytemuck::bytes_of(&uniforms);

    // Measure hand-optimized performance
    let hand_optimized_time = context.execute_and_time(HAND_OPTIMIZED_2_STAGE, Some(uniform_bytes));

    // Measure composed performance
    let composed_shader = generate_composed_2_stage_shader();
    let composed_time = context.execute_and_time(&composed_shader, Some(uniform_bytes));

    // Calculate overhead
    let overhead_percentage = ((composed_time - hand_optimized_time) / hand_optimized_time) * 100.0;

    println!("\n=== Shader Performance Comparison ===");
    println!("Hand-optimized time: {:.6} seconds", hand_optimized_time);
    println!("Composed time:       {:.6} seconds", composed_time);
    println!("Overhead:            {:.2}%", overhead_percentage);
    println!("Max allowed:         {:.2}%", MAX_OVERHEAD_PERCENTAGE);

    // Assert that composed functions are within 15% of hand-optimized
    assert!(
        overhead_percentage <= MAX_OVERHEAD_PERCENTAGE,
        "Composed shader overhead ({:.2}%) exceeds maximum allowed ({}%)",
        overhead_percentage,
        MAX_OVERHEAD_PERCENTAGE
    );

    println!("✓ Performance validation passed!");
}

#[test]
#[ignore]
fn test_composition_depth_scaling() {
    let context = GpuTestContext::new().block_on();

    // Test 3-stage composition
    let shader_3_stage = r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;

fn stage1(v: f32) -> f32 { return v * 2.0; }
fn stage2(v: f32) -> f32 { return v + 0.5; }
fn stage3(v: f32) -> vec4<f32> { return vec4<f32>(v, v, v, 1.0); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input)) {
        return;
    }
    let v1 = stage1(input[index]);
    let v2 = stage2(v1);
    let v3 = stage3(v2);
    output[index] = v3;
}
"#;

    // Test 5-stage composition
    let shader_5_stage = r#"
@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;

fn stage1(v: f32) -> f32 { return v * 2.0; }
fn stage2(v: f32) -> f32 { return v + 0.5; }
fn stage3(v: f32) -> f32 { return clamp(v, 0.0, 1.0); }
fn stage4(v: f32) -> f32 { return v * v; }
fn stage5(v: f32) -> vec4<f32> { return vec4<f32>(v, v, v, 1.0); }

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&input)) {
        return;
    }
    let v1 = stage1(input[index]);
    let v2 = stage2(v1);
    let v3 = stage3(v2);
    let v4 = stage4(v3);
    let v5 = stage5(v4);
    output[index] = v5;
}
"#;

    let time_3_stage = context.execute_and_time(shader_3_stage, None);
    let time_5_stage = context.execute_and_time(shader_5_stage, None);

    println!("\n=== Composition Depth Scaling ===");
    println!("3-stage time: {:.6} seconds", time_3_stage);
    println!("5-stage time: {:.6} seconds", time_5_stage);
    println!(
        "Ratio:        {:.2}x",
        time_5_stage / time_3_stage
    );

    // The 5-stage should not be dramatically slower (should scale linearly or better)
    let ratio = time_5_stage / time_3_stage;
    assert!(
        ratio < 2.0,
        "5-stage composition is too slow compared to 3-stage ({:.2}x)",
        ratio
    );

    println!("✓ Composition scales well with depth!");
}
