// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute engine for force-directed graph layout.
//!
//! The engine compiles four WGSL compute shaders at construction time and
//! orchestrates an async iteration loop that dispatches them in sequence:
//!
//! 1. **Repulsion pass** — O(n²) pairwise Coulomb-like repulsion.
//! 2. **Spring pass** — Hooke-law attraction along edges.
//! 3. **Integration pass** — Euler integration with gravity and damping.
//! 4. **Convergence pass** — Parallel reduction of max displacement.

use super::types::*;
use crate::error::GupResult;
use crate::render::RenderContext;
use wgpu::util::DeviceExt;
use wgpu::*;

/// Workgroup size for all compute shaders (must match WGSL).
const WORKGROUP_SIZE: u32 = 256;

/// WGSL source for all force-layout compute shaders.
const FORCE_LAYOUT_SHADER: &str = include_str!("force_layout.wgsl");

/// GPU-accelerated graph layout engine.
///
/// Compiles compute shaders at creation time so that subsequent layout
/// calls do not pay compilation cost.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::layout::{LayoutEngine, ForceDirected, LayoutNode, LayoutEdge};
/// use gup::render::RenderContext;
///
/// # async fn example() -> gup::error::GupResult<()> {
/// let ctx = RenderContext::new().await?;
/// let engine = LayoutEngine::new(&ctx)?;
/// let result = engine.force_directed_layout(
///     &[LayoutNode { id: 0, x: 0.0, y: 0.0 }, LayoutNode { id: 1, x: 1.0, y: 1.0 }],
///     &[LayoutEdge { source: 0, target: 1 }],
///     &ForceDirected::new(),
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub struct LayoutEngine {
    device: Device,
    queue: Queue,
    repulsion_pipeline: ComputePipeline,
    spring_pipeline: ComputePipeline,
    integrate_pipeline: ComputePipeline,
    convergence_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl std::fmt::Debug for LayoutEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutEngine")
            .field("device", &"<wgpu::Device>")
            .finish()
    }
}

impl LayoutEngine {
    /// Create a new layout engine, compiling compute shaders.
    ///
    /// Returns an error if shader compilation fails.
    pub fn new(context: &RenderContext) -> GupResult<Self> {
        let device = context.device();
        let queue = context.queue();

        // Compile the shader module
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("force_layout_shader"),
            source: ShaderSource::Wgsl(FORCE_LAYOUT_SHADER.into()),
        });

        // Bind group layout shared by all passes:
        //   0: nodes (storage, read-write)
        //   1: edges (storage, read-only)
        //   2: forces (storage, read-write)
        //   3: params (uniform)
        //   4: convergence (storage, read-write)
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("force_layout_bind_group_layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("force_layout_pipeline_layout"),
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

        let repulsion_pipeline = make_pipeline("repulsion_pass", "repulsion_pipeline");
        let spring_pipeline = make_pipeline("spring_pass", "spring_pipeline");
        let integrate_pipeline = make_pipeline("integrate_pass", "integrate_pipeline");
        let convergence_pipeline = make_pipeline("convergence_pass", "convergence_pipeline");

        // We clone device/queue handles so the engine is self-contained.
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            repulsion_pipeline,
            spring_pipeline,
            integrate_pipeline,
            convergence_pipeline,
            bind_group_layout,
        })
    }

    /// Run force-directed layout and return final node positions.
    ///
    /// This is an async method because it reads back convergence data from the
    /// GPU at periodic intervals.  Force computation runs entirely on the GPU
    /// between readbacks.
    pub async fn force_directed_layout(
        &self,
        nodes: &[LayoutNode],
        edges: &[LayoutEdge],
        config: &ForceDirected,
    ) -> GupResult<LayoutResult> {
        let node_count = nodes.len();
        if node_count == 0 {
            return Ok(LayoutResult {
                positions: vec![],
                iterations_performed: 0,
                converged: true,
            });
        }

        // Prepare GPU data --------------------------------------------------

        // Assign initial positions: use provided positions, but scatter nodes
        // at (0,0) to deterministic-random positions to break symmetry.
        let gpu_nodes: Vec<GpuNode> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let (px, py) = if n.x == 0.0 && n.y == 0.0 {
                    // Simple deterministic pseudo-random scatter
                    let angle = (i as f32) * 2.399_963_2; // golden angle
                    let radius = (i as f32 + 1.0).sqrt() * 10.0;
                    (angle.cos() * radius, angle.sin() * radius)
                } else {
                    (n.x, n.y)
                };
                GpuNode {
                    pos_x: px,
                    pos_y: py,
                    vel_x: 0.0,
                    vel_y: 0.0,
                }
            })
            .collect();

        let gpu_edges: Vec<GpuEdge> = edges
            .iter()
            .map(|e| GpuEdge {
                src: e.source,
                tgt: e.target,
            })
            .collect();

        let edge_count = gpu_edges.len().max(1); // ensure at least 1 for buffer

        let params = GpuSimParams {
            repulsion_strength: config.repulsion_strength,
            spring_strength: config.spring_strength,
            spring_rest_length: config.spring_rest_length,
            gravity: config.gravity,
            damping: config.damping,
            node_count: node_count as u32,
            edge_count: edges.len() as u32,
            _pad: 0,
        };

        // Create GPU buffers ------------------------------------------------

        let node_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("layout_nodes"),
                contents: bytemuck::cast_slice(&gpu_nodes),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            });

        // If there are no edges, provide a dummy single-edge buffer so bind
        // group creation succeeds.
        let edge_data = if gpu_edges.is_empty() {
            vec![GpuEdge {
                src: 0,
                tgt: 0,
            }]
        } else {
            gpu_edges
        };
        let edge_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("layout_edges"),
                contents: bytemuck::cast_slice(&edge_data),
                usage: BufferUsages::STORAGE,
            });

        // Force accumulation buffer (vec2 per node)
        let force_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("layout_forces"),
            size: (node_count * 2 * std::mem::size_of::<f32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("layout_params"),
                contents: bytemuck::bytes_of(&params),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

        // Convergence buffer: single u32 used as atomic max via atomicMax.
        // Interpretation: float bits stored as u32 (positive floats sort
        // correctly in unsigned representation).
        let convergence_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("layout_convergence"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Staging buffer for convergence readback
        let staging_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("layout_convergence_staging"),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Bind group ---------------------------------------------------------

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("force_layout_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: node_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: edge_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: force_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: convergence_buffer.as_entire_binding(),
                },
            ],
        });

        // Iteration loop -----------------------------------------------------

        let node_workgroups = (node_count as u32).div_ceil(WORKGROUP_SIZE);
        let edge_workgroups = (edge_count as u32).div_ceil(WORKGROUP_SIZE);

        let mut iterations_performed: u32 = 0;
        let mut converged = false;

        for iter in 0..config.iterations {
            iterations_performed = iter + 1;

            let check_convergence = config.convergence_check_interval > 0
                && (iter + 1) % config.convergence_check_interval == 0;

            // Zero force buffer each iteration
            self.queue.write_buffer(
                &force_buffer,
                0,
                &vec![0u8; node_count * 2 * std::mem::size_of::<f32>()],
            );

            // Zero convergence buffer before convergence pass
            if check_convergence {
                self.queue.write_buffer(&convergence_buffer, 0, &[0u8; 4]);
            }

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("force_layout_encoder"),
                });

            // Pass 1: repulsion
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("repulsion_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.repulsion_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(node_workgroups, 1, 1);
            }

            // Pass 2: spring forces (only if there are real edges)
            if !edges.is_empty() {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("spring_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.spring_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(edge_workgroups, 1, 1);
            }

            // Pass 3: integration (also computes displacement for convergence)
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("integrate_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.integrate_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(node_workgroups, 1, 1);
            }

            // Pass 4: convergence check
            if check_convergence {
                {
                    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("convergence_pass"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&self.convergence_pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.dispatch_workgroups(node_workgroups, 1, 1);
                }
                encoder.copy_buffer_to_buffer(&convergence_buffer, 0, &staging_buffer, 0, 4);
            }

            self.queue.submit(std::iter::once(encoder.finish()));

            // Readback convergence if needed
            if check_convergence {
                let max_disp = self.read_convergence(&staging_buffer).await?;
                if max_disp < config.convergence_threshold {
                    converged = true;
                    break;
                }
            }
        }

        // Read final positions -----------------------------------------------

        let readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("layout_readback"),
            size: (node_count * std::mem::size_of::<GpuNode>()) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("layout_readback_encoder"),
            });
        encoder.copy_buffer_to_buffer(
            &node_buffer,
            0,
            &readback_buffer,
            0,
            (node_count * std::mem::size_of::<GpuNode>()) as u64,
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buffer.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel::<()>();
        slice.map_async(MapMode::Read, move |result| {
            result.expect("Failed to map readback buffer");
            let _ = sender.send(());
        });
        let _ = self.device.poll(PollType::Wait);
        let _ = receiver.await;

        let data = slice.get_mapped_range();
        let final_nodes: &[GpuNode] = bytemuck::cast_slice(&data);

        let positions: Vec<NodePosition> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| NodePosition {
                id: n.id,
                x: final_nodes[i].pos_x,
                y: final_nodes[i].pos_y,
            })
            .collect();

        drop(data);
        readback_buffer.unmap();

        // Keep gpu_nodes alive for ownership purposes.
        let _ = &gpu_nodes;

        Ok(LayoutResult {
            positions,
            iterations_performed,
            converged,
        })
    }

    /// Read back the convergence scalar from the staging buffer.
    async fn read_convergence(&self, staging: &Buffer) -> GupResult<f32> {
        let slice = staging.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel::<()>();
        slice.map_async(MapMode::Read, move |result| {
            result.expect("Failed to map convergence staging buffer");
            let _ = sender.send(());
        });
        let _ = self.device.poll(PollType::Wait);
        let _ = receiver.await;

        let data = slice.get_mapped_range();
        let bits: u32 = *bytemuck::from_bytes(&data[..4]);
        let value = f32::from_bits(bits);
        drop(data);
        staging.unmap();

        // If value is NaN or negative (shouldn't happen), treat as not converged
        if value.is_nan() || value < 0.0 {
            Ok(f32::MAX)
        } else {
            Ok(value)
        }
    }
}
