// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Atlas Viewer - Display the raw 1024x1024 font atlas texture with MSDF channel visualization
//!
//! Press 1-5 to switch between views:
//! 1. Red channel only
//! 2. Green channel only
//! 3. Blue channel only
//! 4. Raw RGB combined
//! 5. Median of three (actual SDF reconstruction)

use gup::{GupContext, GupResult, SurfaceId, text::FontAtlas};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Display mode for the atlas viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    RedChannel = 0,
    GreenChannel = 1,
    BlueChannel = 2,
    RgbCombined = 3,
    Median = 4,
}

impl ViewMode {
    fn name(&self) -> &'static str {
        match self {
            ViewMode::RedChannel => "Red Channel",
            ViewMode::GreenChannel => "Green Channel",
            ViewMode::BlueChannel => "Blue Channel",
            ViewMode::RgbCombined => "RGB Combined",
            ViewMode::Median => "Median (SDF)",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            ViewMode::RedChannel => "First edge color direction (grayscale)",
            ViewMode::GreenChannel => "Second edge color direction (grayscale)",
            ViewMode::BlueChannel => "Third edge color direction (grayscale)",
            ViewMode::RgbCombined => "Raw MSDF colors (Yellow=RG, Cyan=GB, Magenta=RB)",
            ViewMode::Median => "Actual SDF reconstruction: median(R, G, B)",
        }
    }
}

struct AtlasViewerApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    font_atlas: Option<FontAtlas>,
    render_pipeline: Option<RenderPipeline>,
    bind_group: Option<BindGroup>,
    view_mode: ViewMode,
    mode_buffer: Option<Buffer>,
    bind_group_layout: Option<BindGroupLayout>,
    sampler: Option<Sampler>,
}

impl AtlasViewerApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            font_atlas: None,
            render_pipeline: None,
            bind_group: None,
            view_mode: ViewMode::Median, // Start with the most useful view
            mode_buffer: None,
            bind_group_layout: None,
            sampler: None,
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
        let title = format!(
            "Font Atlas Viewer - [{}] {}",
            self.view_mode as u32 + 1,
            self.view_mode.name()
        );
        let window_attributes = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(768, 768));

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

    fn update_window_title(&self) {
        if let Some(window) = &self.window {
            let title = format!(
                "Font Atlas Viewer - [{}] {}",
                self.view_mode as u32 + 1,
                self.view_mode.name()
            );
            window.set_title(&title);
        }
    }

    fn set_view_mode(&mut self, mode: ViewMode) {
        if self.view_mode != mode {
            self.view_mode = mode;
            self.update_window_title();
            println!(
                "📺 View: [{}] {} - {}",
                mode as u32 + 1,
                mode.name(),
                mode.description()
            );

            // Update the mode buffer
            if let Some(context) = &self.context
                && let Some(mode_buffer) = &self.mode_buffer
            {
                let mode_data = [mode as u32, 0, 0, 0]; // Pad to 16 bytes for alignment
                context
                    .queue
                    .write_buffer(mode_buffer, 0, bytemuck::cast_slice(&mode_data));
            }

            // Request redraw
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    async fn initialize_atlas(&mut self) -> GupResult<()> {
        if let Some(context) = &self.context
            && self.font_atlas.is_none()
        {
            let device = &context.device;
            let queue = &context.queue;

            // Create font atlas at 64px using embedded font
            let font_atlas = FontAtlas::new(device, queue, 64.0)?;

            // Create shader for single-panel MSDF visualization with mode selection
            let shader_source = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle (oversized to cover screen)
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), // Bottom-left
        vec2<f32>( 3.0, -1.0), // Bottom-right (extends past screen)
        vec2<f32>(-1.0,  3.0)  // Top-left (extends past screen)
    );

    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0)
    );

    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = uv[vertex_index];

    return out;
}

@group(0) @binding(0) var atlas_texture: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(0) @binding(2) var<uniform> view_mode: u32;

// Median of three values - core MSDF reconstruction
fn median(a: f32, b: f32, c: f32) -> f32 {
    return max(min(a, b), min(max(a, b), c));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the MSDF atlas
    let msdf = textureSample(atlas_texture, atlas_sampler, in.tex_coords);

    // Display based on view_mode:
    // 0: Red channel only
    // 1: Green channel only
    // 2: Blue channel only
    // 3: Raw RGB combined
    // 4: Median (actual SDF reconstruction)

    var color: vec4<f32>;

    if view_mode == 0u {
        // Red channel as grayscale
        color = vec4<f32>(msdf.r, msdf.r, msdf.r, 1.0);
    } else if view_mode == 1u {
        // Green channel as grayscale
        color = vec4<f32>(msdf.g, msdf.g, msdf.g, 1.0);
    } else if view_mode == 2u {
        // Blue channel as grayscale
        color = vec4<f32>(msdf.b, msdf.b, msdf.b, 1.0);
    } else if view_mode == 3u {
        // Raw RGB values
        color = vec4<f32>(msdf.r, msdf.g, msdf.b, 1.0);
    } else {
        // Median of three (SDF reconstruction)
        let m = median(msdf.r, msdf.g, msdf.b);
        color = vec4<f32>(m, m, m, 1.0);
    }

    return color;
}
"#;

            let shader = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Atlas Viewer Shader"),
                source: ShaderSource::Wgsl(shader_source.into()),
            });

            // Create mode uniform buffer
            let mode_data = [self.view_mode as u32, 0, 0, 0]; // Pad to 16 bytes
            let mode_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some("View Mode Buffer"),
                contents: bytemuck::cast_slice(&mode_data),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Create bind group layout
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
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

            // Create sampler for nearest neighbor (no filtering for raw pixel inspection)
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
                    BindGroupEntry {
                        binding: 2,
                        resource: mode_buffer.as_entire_binding(),
                    },
                ],
            });

            self.font_atlas = Some(font_atlas);
            self.render_pipeline = Some(render_pipeline);
            self.bind_group = Some(bind_group);
            self.mode_buffer = Some(mode_buffer);
            self.bind_group_layout = Some(bind_group_layout);
            self.sampler = Some(sampler);

            println!("✅ Atlas viewer initialized");
            println!(
                "📺 View: [{}] {} - {}",
                self.view_mode as u32 + 1,
                self.view_mode.name(),
                self.view_mode.description()
            );
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

        if let Some(surface_id) = self.surface_id
            && let Some(context) = self.context.take()
        {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    let clear_color = wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.1,
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

            println!("✅ Window created!");
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
            } => match key_code {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::Digit1 | KeyCode::Numpad1 => self.set_view_mode(ViewMode::RedChannel),
                KeyCode::Digit2 | KeyCode::Numpad2 => self.set_view_mode(ViewMode::GreenChannel),
                KeyCode::Digit3 | KeyCode::Numpad3 => self.set_view_mode(ViewMode::BlueChannel),
                KeyCode::Digit4 | KeyCode::Numpad4 => self.set_view_mode(ViewMode::RgbCombined),
                KeyCode::Digit5 | KeyCode::Numpad5 => self.set_view_mode(ViewMode::Median),
                _ => {}
            },
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

    println!("🎨 Font Atlas Viewer - MSDF Channel Visualization");
    println!("=================================================");
    println!();
    println!("Press 1-5 to switch between views:");
    println!();
    println!("  1  Red channel     - First edge color direction");
    println!("  2  Green channel   - Second edge color direction");
    println!("  3  Blue channel    - Third edge color direction");
    println!("  4  RGB combined    - Raw MSDF color values");
    println!("  5  Median          - Actual SDF reconstruction");
    println!();
    println!("MSDF uses edge coloring (Yellow=RG, Cyan=GB, Magenta=RB)");
    println!("to preserve sharp corners. The median reconstruction");
    println!("selects the correct distance at corners.");
    println!();
    println!("Controls:");
    println!("  1-5  Switch view mode");
    println!("  ESC  Exit the viewer");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AtlasViewerApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
