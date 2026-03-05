// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-based path tessellation system.
//!
//! Tessellates path commands into triangle vertices using compute shaders,
//! enabling real-time path modifications without CPU bottlenecks.

use crate::error::{GupError, GupResult};
use crate::mark::path::PathCommand;
use std::sync::Arc;
use wgpu;
use wgpu::util::DeviceExt;

/// GPU representation of a path command.
///
/// This struct is laid out to match the WGSL structure in
/// path_tessellation.compute.wgsl.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuPathCommand {
    /// Command type: 0=MoveTo, 1=LineTo, 2=QuadraticTo, 3=CubicTo, 4=Close
    cmd_type: u32,
    _padding1: u32,
    /// Primary point (endpoint for all commands)
    p0: [f32; 2],
    /// Control point 1 (quadratic control or cubic control1)
    p1: [f32; 2],
    /// Control point 2 (cubic control2 only)
    p2: [f32; 2],
}

/// GPU representation of tessellated vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuPathVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

/// Uniforms for controlling tessellation.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TessellationUniforms {
    command_count: u32,
    tolerance: f32,
    max_vertices: u32,
    vertex_count: u32, // Will be atomic in shader
    index_count: u32,  // Will be atomic in shader
    _padding: [u32; 3],
}

/// GPU-based path tessellator.
///
/// Tessellates path commands into triangles using compute shaders.
/// This enables dynamic path updates without CPU->GPU round-trips.
pub struct GpuPathTessellator {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    compute_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuPathTessellator {
    /// Create a new GPU path tessellator.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        // Load compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Path Tessellation Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/path_tessellation.compute.wgsl").into(),
            ),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Path Tessellation Bind Group Layout"),
            entries: &[
                // Input path commands
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
                // Output vertices
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
                // Output indices
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Uniforms
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
            ],
        });

        // Create compute pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Path Tessellation Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Path Tessellation Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            device,
            queue,
            compute_pipeline,
            bind_group_layout,
        }
    }

    /// Tessellate path commands into vertices and indices.
    ///
    /// Returns (vertex_buffer, index_buffer, vertex_count, index_count).
    pub async fn tessellate(
        &self,
        commands: &[PathCommand],
        tolerance: f32,
    ) -> GupResult<(wgpu::Buffer, wgpu::Buffer, u32, u32)> {
        if commands.is_empty() {
            return Err(GupError::validation_error("No path commands to tessellate"));
        }

        // Convert path commands to GPU format
        let gpu_commands: Vec<GpuPathCommand> = commands
            .iter()
            .map(|cmd| self.convert_command(cmd))
            .collect();

        // Estimate maximum vertices needed (conservative)
        let max_vertices = commands.len() * 50; // Allow up to 50 vertices per command
        let max_indices = max_vertices * 3;

        // Create GPU buffers
        let command_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Path Commands Buffer"),
                contents: bytemuck::cast_slice(&gpu_commands),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Vertices Buffer"),
            size: (max_vertices * std::mem::size_of::<GpuPathVertex>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Path Indices Buffer"),
            size: (max_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let uniforms = TessellationUniforms {
            command_count: gpu_commands.len() as u32,
            tolerance,
            max_vertices: max_vertices as u32,
            vertex_count: 0,
            index_count: 0,
            _padding: [0; 3],
        };

        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tessellation Uniforms"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Path Tessellation Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: command_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Path Tessellation Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Path Tessellation Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch one workgroup per command
            let workgroup_size = 64;
            let num_workgroups = (gpu_commands.len() as u32).div_ceil(workgroup_size);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        self.queue.submit(Some(encoder.finish()));

        // Read back the vertex and index counts
        let (vertex_count, index_count) = self.read_counts(&uniform_buffer).await?;

        Ok((vertex_buffer, index_buffer, vertex_count, index_count))
    }

    /// Convert a PathCommand to GPU format.
    fn convert_command(&self, cmd: &PathCommand) -> GpuPathCommand {
        match cmd {
            PathCommand::MoveTo(pos) => GpuPathCommand {
                cmd_type: 0,
                _padding1: 0,
                p0: [pos.x, pos.y],
                p1: [0.0, 0.0],
                p2: [0.0, 0.0],
            },
            PathCommand::LineTo(pos) => GpuPathCommand {
                cmd_type: 1,
                _padding1: 0,
                p0: [pos.x, pos.y],
                p1: [0.0, 0.0],
                p2: [0.0, 0.0],
            },
            PathCommand::QuadraticCurveTo { control, end } => GpuPathCommand {
                cmd_type: 2,
                _padding1: 0,
                p0: [end.x, end.y],
                p1: [control.x, control.y],
                p2: [0.0, 0.0],
            },
            PathCommand::CubicCurveTo {
                control1,
                control2,
                end,
            } => GpuPathCommand {
                cmd_type: 3,
                _padding1: 0,
                p0: [end.x, end.y],
                p1: [control1.x, control1.y],
                p2: [control2.x, control2.y],
            },
            PathCommand::Close => GpuPathCommand {
                cmd_type: 4,
                _padding1: 0,
                p0: [0.0, 0.0],
                p1: [0.0, 0.0],
                p2: [0.0, 0.0],
            },
        }
    }

    /// Read back vertex and index counts from uniform buffer.
    async fn read_counts(&self, uniform_buffer: &wgpu::Buffer) -> GupResult<(u32, u32)> {
        // Create staging buffer
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Count Staging Buffer"),
            size: std::mem::size_of::<TessellationUniforms>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Copy uniform buffer to staging
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Count Copy Encoder"),
            });

        encoder.copy_buffer_to_buffer(
            uniform_buffer,
            0,
            &staging_buffer,
            0,
            std::mem::size_of::<TessellationUniforms>() as u64,
        );

        self.queue.submit(Some(encoder.finish()));

        // Map and read
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        receiver
            .await
            .map_err(|e| GupError::webgpu_error(format!("Failed to map buffer: {e}")))?
            .map_err(|e| GupError::webgpu_error(format!("Buffer map error: {e:?}")))?;

        let data = buffer_slice.get_mapped_range();
        let uniforms: TessellationUniforms =
            *bytemuck::from_bytes(&data[..std::mem::size_of::<TessellationUniforms>()]);

        drop(data);
        staging_buffer.unmap();

        Ok((uniforms.vertex_count, uniforms.index_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec2;

    fn create_test_context() -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..Default::default()
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok()?;

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: Default::default(),
                    experimental_features: Default::default(),
                })
                .await
                .ok()?;

            Some((Arc::new(device), Arc::new(queue)))
        })
    }

    #[tokio::test]
    async fn test_gpu_tessellator_creation() {
        let Some((device, queue)) = create_test_context() else {
            eprintln!("Skipping test: GPU not available");
            return;
        };

        let tessellator = GpuPathTessellator::new(device, queue);
        assert!(std::ptr::addr_of!(tessellator) as usize != 0);
    }

    #[tokio::test]
    async fn test_tessellate_simple_triangle() {
        let Some((device, queue)) = create_test_context() else {
            eprintln!("Skipping test: GPU not available");
            return;
        };

        let tessellator = GpuPathTessellator::new(device, queue);

        let commands = vec![
            PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(Vec2 { x: 1.0, y: 0.0 }),
            PathCommand::LineTo(Vec2 { x: 0.5, y: 1.0 }),
            PathCommand::Close,
        ];

        println!("Tessellating {} commands...", commands.len());
        let result = tessellator.tessellate(&commands, 1.0).await;

        if let Err(ref e) = result {
            eprintln!("Tessellation error: {:?}", e);
        }
        assert!(result.is_ok());

        let (_vertex_buffer, _index_buffer, vertex_count, index_count) = result.unwrap();
        println!(
            "Triangle tessellated: {} vertices, {} indices",
            vertex_count, index_count
        );

        assert!(vertex_count > 0, "Expected at least some vertices");
    }

    #[tokio::test]
    async fn test_tessellate_quadratic_curve() {
        let Some((device, queue)) = create_test_context() else {
            eprintln!("Skipping test: GPU not available");
            return;
        };

        let tessellator = GpuPathTessellator::new(device, queue);

        let commands = vec![
            PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
            PathCommand::QuadraticCurveTo {
                control: Vec2 { x: 0.5, y: 1.0 },
                end: Vec2 { x: 1.0, y: 0.0 },
            },
        ];

        let result = tessellator.tessellate(&commands, 0.1).await;
        assert!(result.is_ok());

        let (_vertex_buffer, _index_buffer, vertex_count, _index_count) = result.unwrap();
        assert!(
            vertex_count >= 3,
            "Quadratic curve should have at least 3 vertices"
        );
        println!("Quadratic curve tessellated: {} vertices", vertex_count);
    }
}
