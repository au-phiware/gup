# GUP-020: WebGPU Integration for RenderContext

## Story Overview

**Title**: Implement Full WebGPU Integration for RenderContext and Mixable Rendering  
**Epic**: Phase 1 Initiative 1 - Core GPU Primitives and Selection API  
**Priority**: Critical  
**Story Points**: 8  

## Context

The current RenderContext contains placeholder WebGPU resources (`Option<wgpu::Device>`) that are never initialized, making the Mixable trait's `render()` methods essentially non-functional. This story implements full WebGPU integration to enable actual GPU-accelerated rendering of composed visualizations, providing the foundation for all subsequent Phase 1 development.

## User Story

**As a** visualization developer using Gup  
**I want** the RenderContext to provide fully functional WebGPU resources  
**So that** I can create Mixable visualizations that perform actual GPU rendering and composition  

## Acceptance Criteria

### Core WebGPU Integration

- [ ] **Device Initialization**: RenderContext successfully initializes WebGPU device and queue
- [ ] **Surface Management**: Support for both native window surfaces and offscreen rendering
- [ ] **Cross-Platform Support**: Works on Windows, macOS, Linux, and WebAssembly
- [ ] **Error Handling**: Comprehensive error handling for GPU resource creation and management

### Render Context Functionality

- [ ] **Command Encoding**: Efficient command encoder lifecycle management
- [ ] **Resource Management**: GPU buffer and texture creation and cleanup
- [ ] **Synchronization**: Proper queue submission and device polling
- [ ] **Multi-Frame Support**: Frame-to-frame resource management and recycling

### Mixable Integration

- [ ] **Real GPU Rendering**: Mixable trait implementations perform actual GPU operations
- [ ] **Composition Support**: Composed visualizations render correctly to same render target
- [ ] **Resource Sharing**: Multiple Mixable components share GPU resources efficiently
- [ ] **Error Propagation**: GPU errors propagate properly through composition chain

## Technical Tasks

### 1. Core WebGPU Infrastructure

- [ ] Implement async device initialization with adapter selection
- [ ] Create surface management for native windows and canvas elements
- [ ] Add GPU feature detection and capability validation
- [ ] Implement graceful fallback for unsupported hardware

### 2. Enhanced RenderContext

- [ ] Replace placeholder GPU resources with fully functional implementations
- [ ] Add command encoder pool for efficient command buffer management
- [ ] Implement render target management (swapchain, offscreen textures)
- [ ] Create GPU memory management and resource cleanup

### 3. Basic Rendering Pipeline

- [ ] Implement fundamental vertex/fragment shader pipeline
- [ ] Create basic geometry rendering (points, lines, triangles)
- [ ] Add uniform buffer management for render parameters
- [ ] Implement basic depth testing and blending

### 4. Mixable Trait Integration

- [ ] Update existing Mixable implementations to perform real GPU rendering
- [ ] Ensure composition works with shared render targets
- [ ] Add render state management between composed components
- [ ] Implement proper resource cleanup for composed visualizations

## Detailed Requirements

### WebGPU Device Initialization

```rust
// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Enhanced RenderContext with full WebGPU integration.

use crate::error::{GupResult, GupError};
use std::sync::Arc;
use wgpu::*;

pub struct RenderContext {
    /// WebGPU instance
    instance: Instance,
    /// GPU adapter
    adapter: Adapter,
    /// GPU device handle
    device: Device,
    /// Command queue for GPU operations
    queue: Queue,
    /// Current surface (for window rendering)
    surface: Option<Surface<'static>>,
    /// Surface configuration
    surface_config: Option<SurfaceConfiguration>,
    /// Current viewport dimensions
    viewport: Viewport,
    /// Command encoder pool for efficient reuse
    encoder_pool: CommandEncoderPool,
    /// Resource manager for cleanup
    resource_manager: ResourceManager,
}

impl RenderContext {
    /// Create a new render context with WebGPU initialization
    pub async fn new() -> GupResult<Self> {
        Self::with_viewport(Viewport::default()).await
    }

    /// Create a new render context with specific viewport
    pub async fn with_viewport(viewport: Viewport) -> GupResult<Self> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GupError::WebGpuError("Failed to find suitable GPU adapter".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("gup_device"),
                    required_features: Features::TIMESTAMP_QUERY | Features::PIPELINE_STATISTICS_QUERY,
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| GupError::WebGpuError(format!("Failed to create device: {}", e)))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            viewport,
            encoder_pool: CommandEncoderPool::new(),
            resource_manager: ResourceManager::new(),
        })
    }

    /// Initialize surface for window rendering
    pub fn init_surface<W>(&mut self, window: Arc<W>) -> GupResult<()>
    where
        W: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + 'static,
    {
        let surface = self.instance.create_surface(window)
            .map_err(|e| GupError::WebGpuError(format!("Failed to create surface: {}", e)))?;

        let surface_caps = surface.get_capabilities(&self.adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: self.viewport.width,
            height: self.viewport.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    /// Get device reference
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get viewport
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Update viewport and reconfigure surface if needed
    pub fn set_viewport(&mut self, viewport: Viewport) -> GupResult<()> {
        self.viewport = viewport;

        if let (Some(surface), Some(config)) = (&self.surface, &mut self.surface_config) {
            config.width = viewport.width;
            config.height = viewport.height;
            surface.configure(&self.device, config);
        }

        Ok(())
    }

    /// Begin a new render pass with automatic surface handling
    pub fn begin_render_pass(&mut self) -> GupResult<ActiveRenderPass> {
        let encoder = self.encoder_pool.get(&self.device)?;

        let (view, present_after) = if let Some(surface) = &self.surface {
            let output = surface
                .get_current_texture()
                .map_err(|e| GupError::WebGpuError(format!("Failed to acquire surface texture: {}", e)))?;
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());
            (view, Some(output))
        } else {
            // Create offscreen render target
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("offscreen_render_target"),
                size: Extent3d {
                    width: self.viewport.width,
                    height: self.viewport.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            (view, None)
        };

        Ok(ActiveRenderPass {
            encoder,
            render_target: view,
            present_after,
            viewport: self.viewport,
            device: &self.device,
            queue: &self.queue,
        })
    }
}

/// Active render pass with automatic resource management
pub struct ActiveRenderPass<'a> {
    encoder: CommandEncoder,
    render_target: TextureView,
    present_after: Option<SurfaceTexture>,
    viewport: Viewport,
    device: &'a Device,
    queue: &'a Queue,
}

impl<'a> ActiveRenderPass<'a> {
    /// Create a render pass targeting the render target
    pub fn render_pass(&mut self, clear_color: Option<Color>) -> RenderPass {
        let clear_value = clear_color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        self.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("main_render_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &self.render_target,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(clear_value),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Get the current viewport
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Submit the render pass and present if rendering to surface
    pub fn submit(self) -> GupResult<()> {
        let command_buffer = self.encoder.finish();
        self.queue.submit(Some(command_buffer));

        if let Some(output) = self.present_after {
            output.present();
        }

        Ok(())
    }
}

/// Command encoder pool for efficient reuse
struct CommandEncoderPool {
    available: Vec<CommandEncoder>,
}

impl CommandEncoderPool {
    fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    fn get(&mut self, device: &Device) -> GupResult<CommandEncoder> {
        Ok(self.available.pop().unwrap_or_else(|| {
            device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("gup_command_encoder"),
            })
        }))
    }
}

/// Resource manager for cleanup tracking
struct ResourceManager {
    // TODO: Implement resource tracking for proper cleanup
}

impl ResourceManager {
    fn new() -> Self {
        Self {}
    }
}
```

### Basic Rendering Pipeline

```rust
/// Basic rendering pipeline for geometric primitives
pub struct BasicPipeline {
    render_pipeline: RenderPipeline,
    vertex_buffer_layout: VertexBufferLayout<'static>,
    uniform_bind_group_layout: BindGroupLayout,
}

impl BasicPipeline {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("basic_shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/basic.wgsl").into()),
        });

        let uniform_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("uniform_bind_group_layout"),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("basic_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x4,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("basic_render_pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::PointList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            vertex_buffer_layout,
            uniform_bind_group_layout,
        }
    }

    pub fn render_points(
        &self,
        render_pass: &mut RenderPass,
        vertices: &[Vertex],
        device: &Device,
    ) -> GupResult<()> {
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);

        Ok(())
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}
```

### Updated Mixable Implementation Example

```rust
/// Example of a real GPU-accelerated Mixable implementation
#[derive(Debug, Clone)]
pub struct GpuScatterPlot {
    points: Vec<Vertex>,
    pipeline: Option<Arc<BasicPipeline>>,
}

impl GpuScatterPlot {
    pub fn new(data: Vec<(f32, f32)>, color: [f32; 4]) -> Self {
        let points = data
            .into_iter()
            .map(|(x, y)| Vertex {
                position: [x, y],
                color,
            })
            .collect();

        Self {
            points,
            pipeline: None,
        }
    }

    fn ensure_pipeline(&mut self, context: &RenderContext) -> GupResult<Arc<BasicPipeline>> {
        if let Some(pipeline) = &self.pipeline {
            return Ok(pipeline.clone());
        }

        let surface_format = context
            .surface_config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(TextureFormat::Bgra8UnormSrgb);

        let pipeline = Arc::new(BasicPipeline::new(context.device(), surface_format));
        self.pipeline = Some(pipeline.clone());
        Ok(pipeline)
    }
}

impl Mixable for GpuScatterPlot {
    type Output = ();

    fn render(&self, context: &mut RenderContext) -> GupResult<()> {
        let mut render_pass = context.begin_render_pass()?;
        let mut rpass = render_pass.render_pass(None);

        // Get or create rendering pipeline
        let pipeline = self.ensure_pipeline(context)?;

        // Render points using GPU pipeline
        pipeline.render_points(&mut rpass, &self.points, context.device())?;

        drop(rpass);
        render_pass.submit()?;

        Ok(())
    }

    fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    fn description(&self) -> String {
        format!("GpuScatterPlot({} points)", self.points.len())
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-001: Build Mixable Trait (provides the trait interface to implement)

### Enables Stories

- GUP-019: Meaningful Mixable Performance Validation (needs real GPU work to benchmark)
- GUP-002: Core Selection Type (needs RenderContext for GPU operations)
- GUP-003: GPU Buffer Management (builds on WebGPU infrastructure)
- GUP-004: Basic Render Context (this story IS the render context implementation)

## Testing Strategy

### WebGPU Integration Tests

```rust
#[tokio::test]
async fn test_render_context_initialization() {
    let context = RenderContext::new().await;
    assert!(context.is_ok());
    
    let context = context.unwrap();
    assert!(context.device().limits().max_texture_dimension_2d > 0);
}

#[tokio::test]
async fn test_surface_initialization() {
    // This would require a test window - implementation depends on platform
    // For now, test offscreen rendering only
    let mut context = RenderContext::new().await.unwrap();
    let render_pass = context.begin_render_pass();
    assert!(render_pass.is_ok());
}

#[tokio::test]
async fn test_basic_gpu_rendering() {
    let mut context = RenderContext::new().await.unwrap();
    
    let scatter_plot = GpuScatterPlot::new(
        vec![(0.0, 0.0), (0.5, 0.5), (-0.5, -0.5)],
        [1.0, 0.0, 0.0, 1.0],
    );
    
    let result = scatter_plot.render(&mut context);
    assert!(result.is_ok());
}
```

### Cross-Platform Tests

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test]
async fn test_webgpu_in_browser() {
    let context = RenderContext::new().await;
    assert!(context.is_ok(), "WebGPU should initialize in browser");
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn test_native_webgpu() {
    let context = RenderContext::new().await;
    assert!(context.is_ok(), "WebGPU should initialize on native platforms");
}
```

### Composition Integration Tests

```rust
#[tokio::test]
async fn test_composed_gpu_rendering() {
    let mut context = RenderContext::new().await.unwrap();
    
    let plot1 = GpuScatterPlot::new(vec![(0.0, 0.0)], [1.0, 0.0, 0.0, 1.0]);
    let plot2 = GpuScatterPlot::new(vec![(0.5, 0.5)], [0.0, 1.0, 0.0, 1.0]);
    
    let composed = plot1.mix(plot2);
    let result = composed.render(&mut context);
    
    assert!(result.is_ok());
}
```

## Success Metrics

### Functional Requirements

- [ ] **Cross-Platform Initialization**: WebGPU initializes successfully on all target platforms
- [ ] **Real GPU Rendering**: Mixable components perform actual GPU operations
- [ ] **Composition Compatibility**: Composed visualizations render correctly
- [ ] **Resource Management**: No GPU resource leaks or crashes

### Performance Requirements

- [ ] **Initialization Time**: Context creation completes within 100ms on target hardware
- [ ] **Frame Rate**: Maintains 60fps for basic rendering scenarios
- [ ] **Memory Usage**: GPU memory usage scales linearly with data size
- [ ] **Error Recovery**: Graceful handling of GPU device loss and recovery

## Risk Assessment

### Technical Risks

- **High**: WebGPU compatibility issues across different browsers and drivers
- **Medium**: Performance bottlenecks in command encoder management
- **Medium**: Cross-platform surface creation complexity
- **Low**: GPU resource exhaustion with large datasets

### Mitigation Strategies

- **Extensive Testing**: Test on wide range of hardware and browser combinations
- **Graceful Fallbacks**: Implement software fallbacks for unsupported hardware
- **Resource Limits**: Implement GPU memory budgets and resource pooling
- **Error Handling**: Comprehensive error recovery for GPU operations

## Implementation Notes

### Design Decisions

- Use `wgpu` for cross-platform WebGPU abstraction
- Implement command encoder pooling for performance
- Support both windowed and offscreen rendering
- Use async initialization to handle GPU enumeration delays

### WebGPU Shader Requirements

```wgsl
// basic.wgsl - Basic vertex/fragment shader for geometric primitives
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = color;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
```

## Definition of Done

- [ ] RenderContext initializes WebGPU device and queue on all target platforms
- [ ] Surface creation works for native windows and HTML canvas elements
- [ ] Basic rendering pipeline renders geometric primitives correctly
- [ ] Mixable trait implementations perform real GPU operations
- [ ] Composed visualizations render correctly to shared render targets
- [ ] Cross-platform tests pass on Windows, macOS, Linux, and WebAssembly
- [ ] Performance meets frame rate requirements for basic scenarios
- [ ] GPU resource management prevents leaks and crashes
- [ ] Error handling provides clear diagnostics for GPU failures
- [ ] Integration tests validate composition works with real GPU rendering
- [ ] Code review completed and approved
- [ ] Documentation updated with WebGPU usage patterns and examples