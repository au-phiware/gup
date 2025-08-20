// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atlas Viewer - Display the raw 1024x1024 font atlas texture

use gup::{GupContext, GupResult, SurfaceId, text::FontAtlas};
use std::sync::Arc;
use wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

struct AtlasViewerApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    font_atlas: Option<FontAtlas>,
    render_pipeline: Option<RenderPipeline>,
    #[allow(dead_code)]
    vertex_buffer: Option<Buffer>,
    bind_group: Option<BindGroup>,
}

impl AtlasViewerApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            font_atlas: None,
            render_pipeline: None,
            vertex_buffer: None,
            bind_group: None,
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            println!("🔧 Creating GPU context...");
            let context = GupContext::headless().await?;
            self.context = Some(context);
            println!("✅ GPU context created");
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let title = "Font Atlas Viewer (1024x1024 Raw)";
        let window_attributes = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(1024, 1024));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        println!("🖼️ Creating window...");

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
            println!("✅ Surface {surface_id} added to context");
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);
        Ok(())
    }

    async fn initialize_atlas(&mut self) -> GupResult<()> {
        if let Some(context) = &self.context {
            if self.font_atlas.is_none() {
                let device = &context.device;
                let queue = &context.queue;

                // Create font atlas at 64px using embedded font
                let font_atlas = FontAtlas::new(device, queue, 64.0)?;

                // Create simple fullscreen quad shader
                let shader_source = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle (simpler and more reliable)
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), // Bottom-left
        vec2<f32>( 3.0, -1.0), // Bottom-right (extends past screen)
        vec2<f32>(-1.0,  3.0)  // Top-left (extends past screen)
    );

    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0), // Bottom-left of texture
        vec2<f32>(2.0, 1.0), // Bottom-right (extends past texture)
        vec2<f32>(0.0, -1.0) // Top-left (extends past texture)
    );

    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = uv[vertex_index];

    return out;
}

@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the raw atlas texture
    let atlas_value = textureSample(atlas_texture, atlas_sampler, in.tex_coords).r;

    // Display as grayscale (raw values from atlas)
    return vec4<f32>(atlas_value, atlas_value, atlas_value, 1.0);
}
"#;

                let shader = device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("Atlas Viewer Shader"),
                    source: ShaderSource::Wgsl(shader_source.into()),
                });

                // Create bind group layout
                let bind_group_layout =
                    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label: Some("Atlas Bind Group Layout"),
                        entries: &[
                            BindGroupLayoutEntry {
                                binding: 0,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Texture {
                                    sample_type: TextureSampleType::Float { filterable: true },
                                    view_dimension: TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            BindGroupLayoutEntry {
                                binding: 1,
                                visibility: ShaderStages::FRAGMENT,
                                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

                // Create pipeline layout
                let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Atlas Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

                // Create render pipeline
                let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some("Atlas Render Pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: PipelineCompilationOptions::default(),
                        targets: &[Some(ColorTargetState {
                            format: TextureFormat::Bgra8UnormSrgb,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });

                // Create sampler for nearest neighbor (no filtering)
                let sampler = device.create_sampler(&SamplerDescriptor {
                    label: Some("Atlas Sampler"),
                    address_mode_u: AddressMode::ClampToEdge,
                    address_mode_v: AddressMode::ClampToEdge,
                    address_mode_w: AddressMode::ClampToEdge,
                    mag_filter: FilterMode::Nearest, // No filtering for raw pixel inspection
                    min_filter: FilterMode::Nearest,
                    mipmap_filter: FilterMode::Nearest,
                    ..Default::default()
                });

                // Create bind group
                let atlas_texture_view = font_atlas
                    .texture()
                    .create_view(&TextureViewDescriptor::default());
                let bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Atlas Bind Group"),
                    layout: &bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&atlas_texture_view),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Sampler(&sampler),
                        },
                    ],
                });

                self.font_atlas = Some(font_atlas);
                self.render_pipeline = Some(render_pipeline);
                self.bind_group = Some(bind_group);

                println!("✅ Atlas viewer initialized");
            }
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.render_pipeline.is_none() {
            match pollster::block_on(self.initialize_atlas()) {
                Ok(()) => {
                    println!("✅ Atlas viewer components initialized");
                }
                Err(e) => {
                    eprintln!("❌ Failed to initialize atlas viewer: {e}");
                    return Err(e.into());
                }
            }
        }

        if let Some(surface_id) = self.surface_id {
            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        let clear_color = wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        };

                        let mut render_pass = frame.render_pass(Some(clear_color));

                        if let (Some(pipeline), Some(bind_group)) =
                            (&self.render_pipeline, &self.bind_group)
                        {
                            render_pass.set_pipeline(pipeline);
                            render_pass.set_bind_group(0, bind_group, &[]);
                            render_pass.draw(0..3, 0..1); // Draw fullscreen triangle
                        }

                        drop(render_pass);
                        frame.finish()?;
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to render frame: {e}");
                    }
                }

                self.context = Some(Arc::new(ctx));
            }
        }
        Ok(())
    }
}

impl ApplicationHandler for AtlasViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("❌ Failed to create context: {e}");
                event_loop.exit();
                return;
            }

            if let Err(e) = self.create_window(event_loop) {
                eprintln!("❌ Failed to create window: {e}");
                event_loop.exit();
                return;
            }

            println!("✅ Window created! Press ESC to exit");
            println!("📊 Displaying raw 1024x1024 font atlas texture");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("👋 Goodbye!");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if key_code == KeyCode::Escape {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("❌ Failed to render frame: {e}");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🎨 Font Atlas Viewer");
    println!("===================");
    println!();
    println!("This displays the raw 1024x1024 font atlas texture.");
    println!("Expected: Black=outside, Gray=edge, White=inside");
    println!("Each glyph rasterized at 64px using embedded font.");
    println!();
    println!("Controls:");
    println!("• ESC - Exit the viewer");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AtlasViewerApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
