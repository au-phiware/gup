// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute engine for force-directed graph layout.
//!
//! The engine compiles WGSL compute shaders at construction time and
//! orchestrates an async iteration loop that dispatches them in sequence.
//!
//! Two repulsion strategies are available:
//!
//! * **Exact** (theta = 0) — O(n²) pairwise Coulomb-like repulsion.
//! * **Barnes-Hut** (theta > 0) — O(n log n) quadtree approximation.
//!
//! Both share the same spring, integration, and convergence passes.

use super::quadtree::{apply_adaptive_theta, build_quadtree};
use super::types::*;
use crate::error::GupResult;
use crate::render::RenderContext;
use wgpu::util::DeviceExt;
use wgpu::*;

/// Workgroup size for all compute shaders (must match WGSL).
const WORKGROUP_SIZE: u32 = 256;

/// WGSL source for all force-layout compute shaders.
const FORCE_LAYOUT_SHADER: &str = include_str!("force_layout.wgsl");

/// WGSL source for the Barnes-Hut tree-traversal repulsion shader.
const BARNES_HUT_SHADER: &str = include_str!("barnes_hut.wgsl");

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
    pub(crate) device: Device,
    pub(crate) queue: Queue,
    // Exact-mode pipelines (shared with BH for spring/integrate/convergence).
    repulsion_pipeline: ComputePipeline,
    spring_pipeline: ComputePipeline,
    integrate_pipeline: ComputePipeline,
    convergence_pipeline: ComputePipeline,
    clear_forces_pipeline: ComputePipeline,
    clear_convergence_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    // Barnes-Hut pipelines & layouts.
    bh_repulsion_pipeline: ComputePipeline,
    bh_tree_bind_group_layout: BindGroupLayout,
    // Treemap compute pipelines.
    treemap_pipelines: super::treemap::TreemapPipelines,
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
        let clear_forces_pipeline = make_pipeline("clear_forces_pass", "clear_forces_pipeline");
        let clear_convergence_pipeline =
            make_pipeline("clear_convergence_pass", "clear_convergence_pipeline");

        // ---- Barnes-Hut pipeline (separate shader module + pipeline layout) ----

        let bh_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("barnes_hut_shader"),
            source: ShaderSource::Wgsl(BARNES_HUT_SHADER.into()),
        });

        let bh_tree_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("bh_tree_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bh_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("bh_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout, &bh_tree_bind_group_layout],
            push_constant_ranges: &[],
        });

        let bh_repulsion_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("bh_repulsion_pipeline"),
            layout: Some(&bh_pipeline_layout),
            module: &bh_shader_module,
            entry_point: Some("bh_repulsion_pass"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ---- Treemap pipelines ----
        let treemap_pipelines = super::treemap::TreemapPipelines::new(device);

        // We clone device/queue handles so the engine is self-contained.
        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            repulsion_pipeline,
            spring_pipeline,
            integrate_pipeline,
            convergence_pipeline,
            clear_forces_pipeline,
            clear_convergence_pipeline,
            bind_group_layout,
            bh_repulsion_pipeline,
            bh_tree_bind_group_layout,
            treemap_pipelines,
        })
    }

    /// Return a reference to the treemap compute pipelines.
    pub(crate) fn treemap_pipelines(&self) -> &super::treemap::TreemapPipelines {
        &self.treemap_pipelines
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
            theta: config.approximation_theta,
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
            vec![GpuEdge { src: 0, tgt: 0 }]
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

        // Bind group (group 0 — shared by all passes) -------------------------

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

        // Dispatch constants.
        let node_workgroups = (node_count as u32).div_ceil(WORKGROUP_SIZE);
        let edge_workgroups = (edge_count as u32).div_ceil(WORKGROUP_SIZE);
        let has_edges = !edges.is_empty();

        let use_barnes_hut = config.approximation_theta > 0.0;

        let (iterations_performed, converged) = if use_barnes_hut {
            self.run_barnes_hut_loop(
                config,
                &gpu_nodes,
                &node_buffer,
                &bind_group,
                &convergence_buffer,
                &staging_buffer,
                node_count,
                node_workgroups,
                edge_workgroups,
                has_edges,
            )
            .await?
        } else {
            self.run_exact_loop(
                config,
                &bind_group,
                &convergence_buffer,
                &staging_buffer,
                node_workgroups,
                edge_workgroups,
                has_edges,
            )
            .await?
        };

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
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
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

    /// Exact O(n²) iteration loop with batched dispatch.
    #[allow(clippy::too_many_arguments)]
    async fn run_exact_loop(
        &self,
        config: &ForceDirected,
        bind_group: &BindGroup,
        convergence_buffer: &Buffer,
        staging_buffer: &Buffer,
        node_workgroups: u32,
        edge_workgroups: u32,
        has_edges: bool,
    ) -> GupResult<(u32, bool)> {
        let mut iterations_performed: u32 = 0;
        let mut converged = false;
        let interval = config.convergence_check_interval.max(1);
        let max_batch = interval.min(5);

        let mut iter = 0u32;
        while iter < config.iterations {
            let batch_end = (iter + max_batch).min(config.iterations);
            let check_convergence = batch_end >= iter + interval
                || batch_end == config.iterations
                || batch_end.is_multiple_of(interval);

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("force_layout_batch_encoder"),
                });

            for _ in iter..batch_end {
                self.encode_clear_forces(&mut encoder, bind_group, node_workgroups);
                self.encode_exact_repulsion(&mut encoder, bind_group, node_workgroups);
                self.encode_spring_integrate(
                    &mut encoder,
                    bind_group,
                    node_workgroups,
                    edge_workgroups,
                    has_edges,
                );
            }

            iterations_performed = batch_end;

            if check_convergence {
                self.encode_convergence_check(
                    &mut encoder,
                    bind_group,
                    convergence_buffer,
                    staging_buffer,
                    node_workgroups,
                );
            }

            self.queue.submit(std::iter::once(encoder.finish()));

            if check_convergence {
                let max_disp = self.read_convergence(staging_buffer).await?;
                if max_disp < config.convergence_threshold {
                    converged = true;
                    break;
                }
            }

            iter = batch_end;
        }

        Ok((iterations_performed, converged))
    }

    /// Barnes-Hut O(n log n) iteration loop.
    ///
    /// Each iteration reads back node positions, builds a quadtree on the
    /// CPU, uploads it, and dispatches the tree-traversal repulsion shader.
    #[allow(clippy::too_many_arguments)]
    async fn run_barnes_hut_loop(
        &self,
        config: &ForceDirected,
        initial_gpu_nodes: &[GpuNode],
        node_buffer: &Buffer,
        bind_group: &BindGroup,
        convergence_buffer: &Buffer,
        staging_buffer: &Buffer,
        node_count: usize,
        node_workgroups: u32,
        edge_workgroups: u32,
        has_edges: bool,
    ) -> GupResult<(u32, bool)> {
        let node_buf_size = (node_count * std::mem::size_of::<GpuNode>()) as u64;

        // Staging buffer for per-iteration position readback.
        let pos_staging = self.device.create_buffer(&BufferDescriptor {
            label: Some("bh_pos_staging"),
            size: node_buf_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Current positions on CPU (start with initial positions).
        let mut cpu_positions: Vec<(f32, f32)> = initial_gpu_nodes
            .iter()
            .map(|n| (n.pos_x, n.pos_y))
            .collect();

        let mut iterations_performed: u32 = 0;
        let mut converged = false;
        let interval = config.convergence_check_interval.max(1);

        for iter in 0..config.iterations {
            // 1. Build quadtree from current CPU positions.
            let mut tree_cells = build_quadtree(&cpu_positions, config.approximation_theta);
            if config.adaptive_theta {
                apply_adaptive_theta(&mut tree_cells, config.approximation_theta);
            }
            let tree_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bh_tree"),
                    contents: bytemuck::cast_slice(&tree_cells),
                    usage: BufferUsages::STORAGE,
                });

            // 2. Create a bind group for the tree (group 1).
            let tree_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("bh_tree_bind_group"),
                layout: &self.bh_tree_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: tree_buffer.as_entire_binding(),
                }],
            });

            // 3. Encode compute passes for one iteration.
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("bh_iteration_encoder"),
                });

            self.encode_clear_forces(&mut encoder, bind_group, node_workgroups);

            // BH repulsion (uses both group 0 and group 1).
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("bh_repulsion"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.bh_repulsion_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.set_bind_group(1, &tree_bind_group, &[]);
                pass.dispatch_workgroups(node_workgroups, 1, 1);
            }

            self.encode_spring_integrate(
                &mut encoder,
                bind_group,
                node_workgroups,
                edge_workgroups,
                has_edges,
            );

            iterations_performed = iter + 1;

            // 4. Decide whether to check convergence.
            let check_convergence = iterations_performed.is_multiple_of(interval)
                || iterations_performed == config.iterations;

            if check_convergence {
                self.encode_convergence_check(
                    &mut encoder,
                    bind_group,
                    convergence_buffer,
                    staging_buffer,
                    node_workgroups,
                );
            }

            // 5. Copy node positions to staging for next iteration's tree build.
            let need_readback = check_convergence || (iter + 1 < config.iterations);
            if need_readback {
                encoder.copy_buffer_to_buffer(node_buffer, 0, &pos_staging, 0, node_buf_size);
            }

            self.queue.submit(std::iter::once(encoder.finish()));

            // 6. Read back convergence and/or positions.
            if check_convergence {
                let max_disp = self.read_convergence(staging_buffer).await?;
                if max_disp < config.convergence_threshold {
                    converged = true;
                    break;
                }
            }

            if need_readback && !converged {
                cpu_positions = self.read_node_positions(&pos_staging, node_count).await?;
            }
        }

        Ok((iterations_performed, converged))
    }

    // ----- Encoder helpers (shared by both loops) ---------------------------

    fn encode_clear_forces(
        &self,
        encoder: &mut CommandEncoder,
        bind_group: &BindGroup,
        node_workgroups: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("clear_forces"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.clear_forces_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(node_workgroups, 1, 1);
    }

    fn encode_exact_repulsion(
        &self,
        encoder: &mut CommandEncoder,
        bind_group: &BindGroup,
        node_workgroups: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("repulsion"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.repulsion_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(node_workgroups, 1, 1);
    }

    fn encode_spring_integrate(
        &self,
        encoder: &mut CommandEncoder,
        bind_group: &BindGroup,
        node_workgroups: u32,
        edge_workgroups: u32,
        has_edges: bool,
    ) {
        if has_edges {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("spring"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.spring_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(edge_workgroups, 1, 1);
        }

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("integrate"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.integrate_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(node_workgroups, 1, 1);
    }

    fn encode_convergence_check(
        &self,
        encoder: &mut CommandEncoder,
        bind_group: &BindGroup,
        convergence_buffer: &Buffer,
        staging_buffer: &Buffer,
        node_workgroups: u32,
    ) {
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("clear_convergence"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.clear_convergence_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("convergence"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.convergence_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(node_workgroups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(convergence_buffer, 0, staging_buffer, 0, 4);
    }

    /// Read back node positions from a staging buffer.
    async fn read_node_positions(
        &self,
        staging: &Buffer,
        node_count: usize,
    ) -> GupResult<Vec<(f32, f32)>> {
        let slice = staging.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel::<()>();
        slice.map_async(MapMode::Read, move |result| {
            result.expect("Failed to map position staging buffer");
            let _ = sender.send(());
        });
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let _ = receiver.await;

        let data = slice.get_mapped_range();
        let gpu_nodes: &[GpuNode] = bytemuck::cast_slice(&data);
        let positions: Vec<(f32, f32)> = gpu_nodes[..node_count]
            .iter()
            .map(|n| (n.pos_x, n.pos_y))
            .collect();
        drop(data);
        staging.unmap();

        Ok(positions)
    }

    /// Read back the convergence scalar from the staging buffer.
    async fn read_convergence(&self, staging: &Buffer) -> GupResult<f32> {
        let slice = staging.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel::<()>();
        slice.map_async(MapMode::Read, move |result| {
            result.expect("Failed to map convergence staging buffer");
            let _ = sender.send(());
        });
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
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

    // ----- Incremental (session-based) API ---------------------------------

    /// Create a [`LayoutSession`] for incremental layout stepping.
    ///
    /// The session holds all GPU buffers for a single graph.  Call
    /// [`step`](Self::step) to advance the simulation by a given number
    /// of iterations and [`read_positions`](Self::read_positions) to
    /// read back the current node positions.
    pub fn create_session(
        &self,
        nodes: &[LayoutNode],
        edges: &[LayoutEdge],
        config: &ForceDirected,
    ) -> GupResult<LayoutSession> {
        let node_count = nodes.len();

        // Assign initial positions (same logic as force_directed_layout).
        let gpu_nodes: Vec<GpuNode> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let (px, py) = if n.x == 0.0 && n.y == 0.0 {
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

        let edge_count = gpu_edges.len().max(1);
        let has_edges = !edges.is_empty();

        let params = GpuSimParams {
            repulsion_strength: config.repulsion_strength,
            spring_strength: config.spring_strength,
            spring_rest_length: config.spring_rest_length,
            gravity: config.gravity,
            damping: config.damping,
            node_count: node_count as u32,
            edge_count: edges.len() as u32,
            theta: config.approximation_theta,
        };

        let node_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("session_nodes"),
                contents: bytemuck::cast_slice(&gpu_nodes),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            });

        let edge_data = if gpu_edges.is_empty() {
            vec![GpuEdge { src: 0, tgt: 0 }]
        } else {
            gpu_edges
        };
        let edge_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("session_edges"),
                contents: bytemuck::cast_slice(&edge_data),
                usage: BufferUsages::STORAGE,
            });

        let force_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("session_forces"),
            size: (node_count * 2 * std::mem::size_of::<f32>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("session_params"),
                contents: bytemuck::bytes_of(&params),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

        let convergence_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("session_convergence"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("session_convergence_staging"),
            size: 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let readback_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("session_readback"),
            size: (node_count * std::mem::size_of::<GpuNode>()) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("session_bind_group"),
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

        let node_workgroups = (node_count as u32).div_ceil(WORKGROUP_SIZE);
        let edge_workgroups = (edge_count as u32).div_ceil(WORKGROUP_SIZE);

        Ok(LayoutSession {
            node_buffer,
            _edge_buffer: edge_buffer,
            _force_buffer: force_buffer,
            _params_buffer: params_buffer,
            _convergence_buffer: convergence_buffer,
            _staging_buffer: staging_buffer,
            readback_buffer,
            bind_group,
            node_count: node_count as u32,
            has_edges,
            node_workgroups,
            edge_workgroups,
            iterations_performed: 0,
            converged: false,
            node_ids: nodes.iter().map(|n| n.id).collect(),
        })
    }

    /// Advance the simulation by `iterations` steps.
    ///
    /// This dispatches compute passes on the GPU and returns immediately
    /// after submission.  Call [`read_positions`](Self::read_positions)
    /// to read back the updated node coordinates.
    pub fn step(&self, session: &mut LayoutSession, iterations: u32) {
        if session.converged || iterations == 0 {
            return;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("session_step_encoder"),
            });

        for _ in 0..iterations {
            self.encode_clear_forces(&mut encoder, &session.bind_group, session.node_workgroups);
            self.encode_exact_repulsion(&mut encoder, &session.bind_group, session.node_workgroups);
            self.encode_spring_integrate(
                &mut encoder,
                &session.bind_group,
                session.node_workgroups,
                session.edge_workgroups,
                session.has_edges,
            );
        }

        session.iterations_performed += iterations;

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Read back the current node positions from a session.
    pub async fn read_positions(&self, session: &LayoutSession) -> GupResult<Vec<NodePosition>> {
        let buf_size = (session.node_count as usize * std::mem::size_of::<GpuNode>()) as u64;

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("session_readback_encoder"),
            });
        encoder.copy_buffer_to_buffer(
            &session.node_buffer,
            0,
            &session.readback_buffer,
            0,
            buf_size,
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = session.readback_buffer.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel::<()>();
        slice.map_async(MapMode::Read, move |result| {
            result.expect("Failed to map session readback buffer");
            let _ = sender.send(());
        });
        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let _ = receiver.await;

        let data = slice.get_mapped_range();
        let gpu_nodes: &[GpuNode] = bytemuck::cast_slice(&data);
        let positions: Vec<NodePosition> = (0..session.node_count as usize)
            .map(|i| NodePosition {
                id: session.node_ids[i],
                x: gpu_nodes[i].pos_x,
                y: gpu_nodes[i].pos_y,
            })
            .collect();
        drop(data);
        session.readback_buffer.unmap();

        Ok(positions)
    }

    /// Pin a node to a fixed position and zero its velocity.
    ///
    /// This writes directly to the GPU node buffer so that subsequent
    /// simulation steps keep the node at the given coordinates.
    pub fn pin_node(&self, session: &LayoutSession, index: u32, x: f32, y: f32) {
        if index >= session.node_count {
            return;
        }
        let node = GpuNode {
            pos_x: x,
            pos_y: y,
            vel_x: 0.0,
            vel_y: 0.0,
        };
        let offset = (index as u64) * (std::mem::size_of::<GpuNode>() as u64);
        self.queue
            .write_buffer(&session.node_buffer, offset, bytemuck::bytes_of(&node));
    }
}

/// A live layout simulation session for incremental stepping.
///
/// Created by [`LayoutEngine::create_session`].  Holds all GPU buffers
/// needed to advance the force-directed simulation a few iterations at
/// a time and read back node positions each frame.
pub struct LayoutSession {
    node_buffer: Buffer,
    _edge_buffer: Buffer,
    _force_buffer: Buffer,
    _params_buffer: Buffer,
    _convergence_buffer: Buffer,
    _staging_buffer: Buffer,
    readback_buffer: Buffer,
    bind_group: BindGroup,
    node_count: u32,
    has_edges: bool,
    node_workgroups: u32,
    edge_workgroups: u32,
    /// Total iterations performed so far.
    pub iterations_performed: u32,
    /// Whether the layout has converged.
    pub converged: bool,
    /// Node IDs in index order (for mapping back to user-facing IDs).
    node_ids: Vec<u32>,
}

impl std::fmt::Debug for LayoutSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutSession")
            .field("node_count", &self.node_count)
            .field("iterations_performed", &self.iterations_performed)
            .field("converged", &self.converged)
            .finish()
    }
}

impl LayoutSession {
    /// Number of nodes in this session.
    pub fn node_count(&self) -> u32 {
        self.node_count
    }
}
