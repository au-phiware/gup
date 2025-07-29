# Gup Technical Design and Implementation Details

> **Key Architecture Update**: Gup uses a unified shader function system where
> all data transformations (scales, color mappings, coordinate transforms,
> etc.) are composable WGSL functions that run on the GPU. See
> `C3_UNIFIED_SHADER_ARCHITECTURE.md` for complete details.

## Core Technical Architecture

### Memory Layout and Data Structures

#### Vertex Buffer Organization

```rust
// Optimized vertex layouts for different mark types
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    position: [f32; 2],      // Screen coordinates
    radius: f32,             // Circle radius
    color: [f32; 4],         // RGBA color
    border_width: f32,       // Border thickness
    border_color: [f32; 4],  // Border RGBA
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    start: [f32; 2],         // Line start point
    end: [f32; 2],           // Line end point
    thickness: f32,          // Line thickness
    color: [f32; 4],         // Line color
    dash_pattern: u32,       // Bit pattern for dashing
}

// Efficient instanced rendering for large datasets
#[repr(C)]
pub struct InstanceData {
    transform: [[f32; 4]; 4], // 4x4 transformation matrix
    color_multiplier: [f32; 4], // Per-instance color variation
    custom_params: [f32; 4],   // Mark-specific parameters
}
```

#### Data Management System

```rust
pub struct DataManager<T> {
    // CPU-side data for bookkeeping
    data: Vec<T>,
    dirty_ranges: Vec<Range<usize>>,

    // GPU resources
    vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,

    // Transformation pipeline
    data_transform: Box<dyn DataTransform<T>>,
    vertex_factory: Box<dyn VertexFactory<T>>,
}

impl<T> DataManager<T> {
    pub fn update_data(&mut self, range: Range<usize>, new_data: &[T]) {
        // Update CPU-side data
        self.data[range.clone()].copy_from_slice(new_data);

        // Mark range as dirty for next GPU sync
        self.dirty_ranges.push(range);

        // Batch small updates to reduce GPU transfers
        if self.dirty_ranges.len() > MAX_DIRTY_RANGES {
            self.flush_to_gpu();
        }
    }

    fn flush_to_gpu(&mut self) {
        // Merge overlapping dirty ranges
        let merged_ranges = merge_ranges(&self.dirty_ranges);

        // Transform data to vertex format
        for range in merged_ranges {
            let vertices: Vec<_> = self.data[range.clone()]
                .iter()
                .map(|d| self.vertex_factory.create_vertex(d))
                .collect();

            // Efficient partial buffer update
            self.vertex_buffer.write_range(
                range.start * size_of::<Vertex>(),
                bytemuck::cast_slice(&vertices)
            );
        }

        self.dirty_ranges.clear();
    }
}
```

### Shader System Architecture

#### Modular Shader Generation

```rust
pub struct ShaderBuilder {
    vertex_stages: Vec<ShaderStage>,
    fragment_stages: Vec<ShaderStage>,
    compute_stages: Vec<ShaderStage>,
    uniforms: HashMap<String, UniformType>,
}

impl ShaderBuilder {
    pub fn add_scale_transform(&mut self, scale_type: ScaleType) -> &mut Self {
        match scale_type {
            ScaleType::Linear => {
                self.vertex_stages.push(ShaderStage::new(
                    "linear_scale",
                    include_str!("shaders/linear_scale.wgsl")
                ));
                self.uniforms.insert("linear_scale".to_string(),
                    UniformType::Struct("LinearScaleUniforms"));
            }
            ScaleType::Log => {
                self.vertex_stages.push(ShaderStage::new(
                    "log_scale",
                    include_str!("shaders/log_scale.wgsl")
                ));
            }
            // ... other scale types
        }
        self
    }

    pub fn build(&self, device: &wgpu::Device) -> wgpu::ShaderModule {
        let vertex_shader = self.combine_vertex_stages();
        let fragment_shader = self.combine_fragment_stages();

        let full_shader = format!(
            "// Auto-generated Gup shader\n{}\n{}",
            vertex_shader, fragment_shader
        );

        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gup Generated Shader"),
            source: wgpu::ShaderSource::Wgsl(full_shader.into()),
        })
    }
}
```

#### WGSL Shader Templates

```wgsl
// Linear scale transformation (shaders/linear_scale.wgsl)
struct LinearScaleUniforms {
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
}

fn apply_linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    return scale.range_min + normalized * (scale.range_max - scale.range_min);
}

// Color scale with interpolation (shaders/color_scale.wgsl)
struct ColorScaleUniforms {
    colors: array<vec4<f32>, 8>,  // Support up to 8 color stops
    stops: array<f32, 8>,         // Corresponding stop positions
    count: u32,                   // Number of active stops
}

fn apply_color_scale(t: f32, scale: ColorScaleUniforms) -> vec4<f32> {
    // Find interpolation range
    var i: u32 = 0u;
    while (i < scale.count - 1u && t > scale.stops[i + 1u]) {
        i = i + 1u;
    }

    if (i >= scale.count - 1u) {
        return scale.colors[scale.count - 1u];
    }

    // Linear interpolation between color stops
    let t0 = scale.stops[i];
    let t1 = scale.stops[i + 1u];
    let local_t = (t - t0) / (t1 - t0);

    return mix(scale.colors[i], scale.colors[i + 1u], local_t);
}

// SDF-based circle rendering (shaders/circle.wgsl)
@vertex
fn vs_circle(
    @location(0) position: vec2<f32>,
    @location(1) radius: f32,
    @location(2) color: vec4<f32>,
    @location(3) border_width: f32,
    @location(4) border_color: vec4<f32>,
) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.world_position = position;
    output.radius = radius;
    output.color = color;
    output.border_width = border_width;
    output.border_color = border_color;
    return output;
}

@fragment
fn fs_circle(input: VertexOutput) -> @location(0) vec4<f32> {
    let center = input.world_position;
    let pixel_pos = input.clip_position.xy;
    let distance = length(pixel_pos - center);

    // SDF-based anti-aliased circle
    let outer_radius = input.radius;
    let inner_radius = outer_radius - input.border_width;

    // Smooth edges using derivative for anti-aliasing
    let edge_softness = fwidth(distance);

    // Calculate alpha for main circle
    let circle_alpha = 1.0 - smoothstep(
        outer_radius - edge_softness,
        outer_radius + edge_softness,
        distance
    );

    // Calculate border
    let border_alpha = smoothstep(
        inner_radius - edge_softness,
        inner_radius + edge_softness,
        distance
    );

    // Blend colors
    let final_color = mix(input.color, input.border_color, border_alpha);
    return vec4<f32>(final_color.rgb, final_color.a * circle_alpha);
}
```

### Scale System Implementation

#### GPU-Resident Scale System

```rust
pub trait Scale {
    type Domain;
    type Range;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable;

    fn create_uniforms(&self) -> Self::Uniforms;
    fn shader_function() -> &'static str;

    fn apply_cpu(&self, value: Self::Domain) -> Self::Range;
    fn invert_cpu(&self, value: Self::Range) -> Self::Domain;
}

pub struct LinearScale {
    domain: [f32; 2],
    range: [f32; 2],
    clamped: bool,

    // GPU resources
    uniform_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

impl Scale for LinearScale {
    type Domain = f32;
    type Range = f32;
    type Uniforms = LinearScaleUniforms;

    fn create_uniforms(&self) -> Self::Uniforms {
        LinearScaleUniforms {
            domain_min: self.domain[0],
            domain_max: self.domain[1],
            range_min: self.range[0],
            range_max: self.range[1],
        }
    }

    fn shader_function() -> &'static str {
        "apply_linear_scale"
    }

    fn apply_cpu(&self, value: f32) -> f32 {
        let t = (value - self.domain[0]) / (self.domain[1] - self.domain[0]);
        let result = self.range[0] + t * (self.range[1] - self.range[0]);

        if self.clamped {
            result.clamp(self.range[0], self.range[1])
        } else {
            result
        }
    }
}

impl LinearScale {
    pub fn upload_to_gpu(&mut self, device: &wgpu::Device) {
        let uniforms = self.create_uniforms();

        self.uniform_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Linear Scale Uniforms"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        ));

        // Create bind group for shader access
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Linear Scale Bind Group"),
            layout: &create_scale_bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_ref().unwrap().as_entire_binding(),
            }],
        }));
    }
}
```

### Interaction System Design

#### GPU-Based Spatial Queries

```rust
// Compute shader for efficient hit testing
pub struct SpatialQuerySystem {
    compute_pipeline: wgpu::ComputePipeline,
    query_buffer: wgpu::Buffer,
    result_buffer: wgpu::Buffer,
    vertex_buffer_view: wgpu::Buffer,
}

impl SpatialQuerySystem {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Spatial Query Compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/spatial_query.wgsl").into()),
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Spatial Query Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        // ... buffer creation

        Self { compute_pipeline, query_buffer, result_buffer, vertex_buffer_view }
    }

    pub async fn query_point(&mut self,
        encoder: &mut wgpu::CommandEncoder,
        point: [f32; 2],
        radius: f32
    ) -> Vec<u32> {
        // Upload query parameters
        let query = SpatialQuery { point, radius, ..Default::default() };
        self.query_buffer.write(&[query]);

        // Dispatch compute shader
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Spatial Query Pass"),
        });
        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.query_bind_group, &[]);
        compute_pass.dispatch_workgroups(
            (self.vertex_count + 63) / 64,  // 64 threads per workgroup
            1,
            1
        );
        drop(compute_pass);

        // Read results
        let results = self.result_buffer.read().await;
        results.into_iter()
            .enumerate()
            .filter_map(|(i, hit)| if hit != 0 { Some(i as u32) } else { None })
            .collect()
    }
}
```

```wgsl
// Spatial query compute shader (shaders/spatial_query.wgsl)
struct SpatialQuery {
    point: vec2<f32>,
    radius: f32,
    _padding: f32,
}

struct Vertex {
    position: vec2<f32>,
    radius: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> query: SpatialQuery;
@group(0) @binding(1) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(2) var<storage, read_write> results: array<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    if (index >= arrayLength(&vertices)) {
        return;
    }

    let vertex = vertices[index];
    let distance = length(vertex.position - query.point);

    // Check if point is within vertex bounds (considering vertex radius)
    let hit_distance = query.radius + vertex.radius;
    results[index] = select(0u, 1u, distance <= hit_distance);
}
```

### Animation and Transition System

#### Timeline-Based Animation Engine

```rust
pub struct AnimationSystem {
    active_animations: Vec<Animation>,
    timeline: f64,

    // GPU resources for interpolation
    interpolation_compute: wgpu::ComputePipeline,
    keyframe_buffer: wgpu::Buffer,
    target_buffer: wgpu::Buffer,
}

pub struct Animation {
    pub id: AnimationId,
    pub start_time: f64,
    pub duration: f64,
    pub ease_function: EaseFunction,
    pub keyframes: KeyframeSequence,
    pub target_attributes: Vec<AttributeTarget>,
}

pub enum KeyframeSequence {
    TwoKey { from: AttributeValues, to: AttributeValues },
    MultiKey { keyframes: Vec<(f64, AttributeValues)> },
    Procedural { generator: Box<dyn Fn(f64) -> AttributeValues> },
}

impl AnimationSystem {
    pub fn update(&mut self, delta_time: f64, encoder: &mut wgpu::CommandEncoder) {
        self.timeline += delta_time;

        // Filter active animations
        self.active_animations.retain(|anim| {
            let elapsed = self.timeline - anim.start_time;
            elapsed < anim.duration
        });

        // Batch compute shader updates
        if !self.active_animations.is_empty() {
            self.update_animations_gpu(encoder);
        }
    }

    fn update_animations_gpu(&mut self, encoder: &mut wgpu::CommandEncoder) {
        // Prepare animation data for GPU
        let animation_uniforms: Vec<AnimationUniforms> = self.active_animations
            .iter()
            .map(|anim| {
                let elapsed = self.timeline - anim.start_time;
                let t = (elapsed / anim.duration).clamp(0.0, 1.0);
                let eased_t = anim.ease_function.apply(t);

                AnimationUniforms {
                    animation_id: anim.id.0,
                    progress: eased_t as f32,
                    ..Default::default()
                }
            })
            .collect();

        self.animation_uniform_buffer.write(&animation_uniforms);

        // Dispatch interpolation compute shader
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Animation Update Pass"),
        });
        compute_pass.set_pipeline(&self.interpolation_compute);
        compute_pass.set_bind_group(0, &self.animation_bind_group, &[]);
        compute_pass.dispatch_workgroups(
            (self.animated_vertex_count + 63) / 64,
            1,
            1
        );
    }
}

#[derive(Clone, Copy)]
pub enum EaseFunction {
    Linear,
    QuadIn, QuadOut, QuadInOut,
    CubicIn, CubicOut, CubicInOut,
    ElasticIn, ElasticOut, ElasticInOut,
    BounceIn, BounceOut, BounceInOut,
    Custom(fn(f64) -> f64),
}

impl EaseFunction {
    pub fn apply(&self, t: f64) -> f64 {
        match self {
            EaseFunction::Linear => t,
            EaseFunction::QuadIn => t * t,
            EaseFunction::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            EaseFunction::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            EaseFunction::CubicIn => t * t * t,
            EaseFunction::CubicOut => 1.0 - (1.0 - t).powi(3),
            EaseFunction::Custom(f) => f(t),
            // ... other easing functions
        }
    }
}
```

### Performance Optimization Strategies

#### Level-of-Detail System

```rust
pub struct LevelOfDetailManager {
    lod_levels: Vec<LodLevel>,
    current_viewport: Viewport,

    // GPU-based culling
    culling_compute: wgpu::ComputePipeline,
    visibility_buffer: wgpu::Buffer,
    indirect_buffer: wgpu::Buffer,
}

pub struct LodLevel {
    pub min_pixel_size: f32,
    pub vertex_reduction: f32,      // 0.0 = full detail, 1.0 = minimal detail
    pub shader_simplification: ShaderLod,
}

impl LevelOfDetailManager {
    pub fn update_lod(&mut self, encoder: &mut wgpu::CommandEncoder) {
        // GPU-based frustum culling and LOD selection
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("LOD Update Pass"),
        });

        compute_pass.set_pipeline(&self.culling_compute);
        compute_pass.set_bind_group(0, &self.culling_bind_group, &[]);
        compute_pass.dispatch_workgroups(
            (self.total_elements + 63) / 64,
            1,
            1
        );
    }

    pub fn get_render_commands(&self) -> Vec<wgpu::RenderBundleEncoder> {
        // Generate render commands based on visibility and LOD decisions
        self.lod_levels.iter()
            .enumerate()
            .filter_map(|(level, lod)| {
                if self.has_visible_elements(level) {
                    Some(self.create_render_bundle(level, lod))
                } else {
                    None
                }
            })
            .collect()
    }
}
```

#### Memory Pool Management

```rust
pub struct GpuMemoryPool {
    vertex_pools: Vec<BufferPool>,
    uniform_pools: Vec<BufferPool>,
    texture_pools: Vec<TexturePool>,

    allocation_strategy: AllocationStrategy,
}

pub struct BufferPool {
    buffer: wgpu::Buffer,
    allocator: BuddyAllocator,
    usage: wgpu::BufferUsages,
    size: u64,
}

impl GpuMemoryPool {
    pub fn allocate_vertex_buffer(&mut self, size: u64) -> BufferAllocation {
        // Find suitable pool or create new one
        for pool in &mut self.vertex_pools {
            if let Some(allocation) = pool.allocator.allocate(size) {
                return BufferAllocation {
                    buffer: &pool.buffer,
                    offset: allocation.offset,
                    size: allocation.size,
                };
            }
        }

        // Create new pool if needed
        let new_pool_size = (size * 2).max(64 * 1024 * 1024); // At least 64MB
        self.create_vertex_pool(new_pool_size)
            .allocate(size)
            .expect("Failed to allocate from new pool")
    }

    pub fn defragment(&mut self, device: &wgpu::Device) {
        // Periodically defragment memory pools
        for pool in &mut self.vertex_pools {
            if pool.allocator.fragmentation_ratio() > 0.3 {
                pool.defragment(device);
            }
        }
    }
}
```

#### Batch Rendering System

```rust
pub struct BatchRenderer {
    render_bundles: HashMap<RenderKey, wgpu::RenderBundle>,
    batch_groups: Vec<BatchGroup>,

    // State tracking
    current_pipeline: Option<wgpu::RenderPipeline>,
    current_bind_groups: Vec<Option<wgpu::BindGroup>>,
}

#[derive(Hash, Eq, PartialEq)]
pub struct RenderKey {
    pipeline_id: PipelineId,
    vertex_buffer_id: BufferId,
    bind_group_hash: u64,
}

pub struct BatchGroup {
    key: RenderKey,
    draw_commands: Vec<DrawCommand>,
    vertex_range: Range<u32>,
    instance_range: Range<u32>,
}

impl BatchRenderer {
    pub fn submit_draw(&mut self,
        pipeline: &wgpu::RenderPipeline,
        vertex_buffer: &wgpu::Buffer,
        instance_data: &[InstanceData],
        bind_groups: &[&wgpu::BindGroup]
    ) {
        let key = RenderKey {
            pipeline_id: pipeline.global_id(),
            vertex_buffer_id: vertex_buffer.global_id(),
            bind_group_hash: self.hash_bind_groups(bind_groups),
        };

        // Add to appropriate batch group
        self.batch_groups
            .entry(key)
            .or_insert_with(|| BatchGroup::new(key))
            .add_instances(instance_data);
    }

    pub fn flush(&mut self, encoder: &mut wgpu::CommandEncoder) {
        // Sort batch groups for optimal rendering order
        self.batch_groups.sort_by_key(|group| group.key);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Gup Batch Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
        });

        for group in &self.batch_groups {
            // Set pipeline and bind groups only when they change
            if self.current_pipeline.as_ref() != Some(&group.pipeline) {
                render_pass.set_pipeline(&group.pipeline);
                self.current_pipeline = Some(group.pipeline.clone());
            }

            // Batch multiple draw calls with same state
            render_pass.draw_indexed(
                group.index_range.clone(),
                0,
                group.instance_range.clone()
            );
        }

        self.batch_groups.clear();
    }
}
```

This technical design provides the foundation for a
high-performance, GPU-accelerated data visualization library that
can handle massive datasets while maintaining the declarative
elegance that made D3.js so successful.
