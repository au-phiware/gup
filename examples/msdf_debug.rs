// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! MSDF Debug Tool - Visualize glyph geometry and distance fields
//!
//! Controls:
//! - Type any character to display its glyph
//! - Number keys (0-9) to select an edge segment
//! - 'd' - Show signed_distance (true distance)
//! - 'p' - Show pseudo_distance
//! - 'm' - Show MSDF median result
//! - 'c' - Toggle contour display
//! - ESC - Exit

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

use gup::text::msdf::{EdgeColor, GlyphOutline, MsdfConfig, MsdfGenerator, Point};

/// Default font data embedded in the binary
const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../assets/fonts/default.ttf");

const WINDOW_WIDTH: u32 = 900;
const WINDOW_HEIGHT: u32 = 700;
const _GLYPH_DISPLAY_SIZE: f32 = 400.0;
const DISTANCE_FIELD_SIZE: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DistanceMode {
    SignedDistance,
    PseudoDistance,
    MsdfMedian,
}

impl std::fmt::Display for DistanceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceMode::SignedDistance => write!(f, "Signed Distance (true)"),
            DistanceMode::PseudoDistance => write!(f, "Pseudo Distance"),
            DistanceMode::MsdfMedian => write!(f, "MSDF Median"),
        }
    }
}

struct MsdfDebugApp {
    window: Option<Arc<Window>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    render_pipeline: Option<wgpu::RenderPipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
    distance_texture: Option<wgpu::Texture>,
    distance_bind_group: Option<wgpu::BindGroup>,

    msdf_generator: Option<MsdfGenerator>,
    current_char: char,
    current_outline: Option<GlyphOutline>,
    selected_edge: Option<usize>,
    distance_mode: DistanceMode,
    show_contours: bool,
    edge_count: usize,
}

impl MsdfDebugApp {
    fn new() -> Self {
        Self {
            window: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            render_pipeline: None,
            vertex_buffer: None,
            distance_texture: None,
            distance_bind_group: None,
            msdf_generator: None,
            current_char: 'A',
            current_outline: None,
            selected_edge: None,
            distance_mode: DistanceMode::MsdfMedian,
            show_contours: true,
            edge_count: 0,
        }
    }

    fn print_help(&self) {
        println!("\n🔍 MSDF Debug Tool");
        println!("==================");
        println!("Current character: '{}'", self.current_char);
        println!("Distance mode: {}", self.distance_mode);
        println!(
            "Selected edge: {}",
            self.selected_edge
                .map(|e| e.to_string())
                .unwrap_or_else(|| "None (showing all)".to_string())
        );
        println!("Edge count: {}", self.edge_count);
        println!("\nControls:");
        println!("  Type any character - Change glyph");
        println!("  0-9 - Select edge (0 = deselect)");
        println!("  d - Signed distance mode");
        println!("  p - Pseudo distance mode");
        println!("  m - MSDF median mode");
        println!("  c - Toggle contour display");
        println!("  ESC - Exit");
    }

    fn load_glyph(&mut self, c: char) {
        self.current_char = c;
        println!("\n📝 Loading glyph: '{}'", c);

        if let Some(generator) = &self.msdf_generator {
            match generator.generate_outline(c) {
                Ok(mut outline) => {
                    // Apply edge coloring
                    let config = MsdfConfig::default();
                    outline.apply_edge_coloring(config.angle_threshold);

                    // Count edges
                    self.edge_count = outline.contours.iter().map(|c| c.edges.len()).sum();

                    println!("  Contours: {}", outline.contours.len());
                    println!("  Total edges: {}", self.edge_count);

                    // Print edge info
                    let mut edge_idx = 0;
                    for (contour_idx, contour) in outline.contours.iter().enumerate() {
                        println!("  Contour {}:", contour_idx);
                        for edge in &contour.edges {
                            let color_name = if edge.color == EdgeColor::WHITE {
                                "White"
                            } else if edge.color == EdgeColor::YELLOW {
                                "Yellow (RG)"
                            } else if edge.color == EdgeColor::CYAN {
                                "Cyan (GB)"
                            } else if edge.color == EdgeColor::MAGENTA {
                                "Magenta (RB)"
                            } else if edge.color.r && !edge.color.g && !edge.color.b {
                                "Red"
                            } else if !edge.color.r && edge.color.g && !edge.color.b {
                                "Green"
                            } else if !edge.color.r && !edge.color.g && edge.color.b {
                                "Blue"
                            } else {
                                "Unknown"
                            };
                            let edge_type = match &edge.edge_type {
                                gup::text::msdf::EdgeType::Line { .. } => "Line",
                                gup::text::msdf::EdgeType::QuadCurve { .. } => "Quad",
                                gup::text::msdf::EdgeType::CubicCurve { .. } => "Cubic",
                            };
                            println!("    Edge {}: {} - {}", edge_idx, edge_type, color_name);
                            edge_idx += 1;
                        }
                    }

                    self.current_outline = Some(outline);
                    self.selected_edge = None;
                    self.update_display();
                }
                Err(e) => {
                    println!("  Error loading glyph: {:?}", e);
                    self.current_outline = None;
                }
            }
        }
    }

    fn select_edge(&mut self, edge_num: usize) {
        if edge_num == 0 {
            self.selected_edge = None;
            println!("🎯 Deselected edge (showing all)");
        } else if edge_num <= self.edge_count {
            self.selected_edge = Some(edge_num - 1);
            println!("🎯 Selected edge {}", edge_num - 1);
        } else {
            println!(
                "⚠️  Edge {} doesn't exist (max: {})",
                edge_num, self.edge_count
            );
        }
        self.update_display();
    }

    fn set_distance_mode(&mut self, mode: DistanceMode) {
        self.distance_mode = mode;
        println!("📏 Distance mode: {}", mode);
        self.update_display();
    }

    fn update_display(&mut self) {
        let Some(outline) = &self.current_outline else {
            return;
        };
        let Some(queue) = &self.queue else {
            return;
        };
        let Some(texture) = &self.distance_texture else {
            return;
        };

        // Compute distance field
        let mut distance_data = vec![0u8; DISTANCE_FIELD_SIZE * DISTANCE_FIELD_SIZE * 4];

        // Get glyph bounds
        let bounds = &outline.bounds;
        let glyph_width = bounds.1.x - bounds.0.x;
        let glyph_height = bounds.1.y - bounds.0.y;
        let padding = 0.1 * glyph_width.max(glyph_height);

        let min_x = bounds.0.x - padding;
        let max_x = bounds.1.x + padding;
        let min_y = bounds.0.y - padding;
        let max_y = bounds.1.y + padding;

        let scale_x = (max_x - min_x) / DISTANCE_FIELD_SIZE as f32;
        let scale_y = (max_y - min_y) / DISTANCE_FIELD_SIZE as f32;

        // Distance range for normalization
        let distance_range = 0.1 * glyph_width.max(glyph_height);

        for y in 0..DISTANCE_FIELD_SIZE {
            for x in 0..DISTANCE_FIELD_SIZE {
                let px = min_x + (x as f32 + 0.5) * scale_x;
                // Flip Y: texture y=0 is top, glyph y increases upward
                let py = max_y - (y as f32 + 0.5) * scale_y;
                let point = Point::new(px, py);

                let (r, g, b) = match self.distance_mode {
                    DistanceMode::MsdfMedian => {
                        let msdf = outline.msdf_at(&point);
                        let median = median3(msdf[0], msdf[1], msdf[2]);
                        let normalized = (median / distance_range + 0.5).clamp(0.0, 1.0);
                        let v = (normalized * 255.0) as u8;
                        (v, v, v)
                    }
                    DistanceMode::SignedDistance | DistanceMode::PseudoDistance => {
                        if let Some(edge_idx) = self.selected_edge {
                            // Show distance for selected edge only
                            if let Some(edge) = get_edge_by_index(outline, edge_idx) {
                                let sd = if self.distance_mode == DistanceMode::SignedDistance {
                                    edge.signed_distance(&point)
                                } else {
                                    edge.pseudo_distance(&point)
                                };
                                let normalized =
                                    (sd.distance / distance_range + 0.5).clamp(0.0, 1.0);
                                let v = (normalized * 255.0) as u8;

                                // Color by edge color
                                let (r, g, b) = (
                                    if edge.color.r { v } else { 0 },
                                    if edge.color.g { v } else { 0 },
                                    if edge.color.b { v } else { 0 },
                                );
                                (r, g, b)
                            } else {
                                (128, 128, 128)
                            }
                        } else {
                            // Show all edges combined
                            let msdf = outline.msdf_at(&point);
                            let r_norm = (msdf[0] / distance_range + 0.5).clamp(0.0, 1.0);
                            let g_norm = (msdf[1] / distance_range + 0.5).clamp(0.0, 1.0);
                            let b_norm = (msdf[2] / distance_range + 0.5).clamp(0.0, 1.0);
                            (
                                (r_norm * 255.0) as u8,
                                (g_norm * 255.0) as u8,
                                (b_norm * 255.0) as u8,
                            )
                        }
                    }
                };

                let idx = (y * DISTANCE_FIELD_SIZE + x) * 4;
                distance_data[idx] = r;
                distance_data[idx + 1] = g;
                distance_data[idx + 2] = b;
                distance_data[idx + 3] = 255;
            }
        }

        // Upload to texture
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &distance_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(DISTANCE_FIELD_SIZE as u32 * 4),
                rows_per_image: Some(DISTANCE_FIELD_SIZE as u32),
            },
            wgpu::Extent3d {
                width: DISTANCE_FIELD_SIZE as u32,
                height: DISTANCE_FIELD_SIZE as u32,
                depth_or_array_layers: 1,
            },
        );

        // Update vertex buffer for contour lines
        if self.show_contours {
            self.update_contour_vertices(min_x, max_x, min_y, max_y);
        }

        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn update_contour_vertices(&mut self, min_x: f32, max_x: f32, min_y: f32, max_y: f32) {
        let Some(outline) = &self.current_outline else {
            return;
        };
        let Some(queue) = &self.queue else {
            return;
        };
        let Some(buffer) = &self.vertex_buffer else {
            return;
        };

        let mut vertices: Vec<f32> = Vec::new();

        // Map glyph coordinates to NDC (-1 to 1)
        let map_x = |x: f32| -> f32 { 2.0 * (x - min_x) / (max_x - min_x) - 1.0 };
        let map_y = |y: f32| -> f32 { 2.0 * (y - min_y) / (max_y - min_y) - 1.0 };

        let color_to_rgb = |color: EdgeColor| -> [f32; 3] {
            [
                if color.r { 1.0 } else { 0.0 },
                if color.g { 1.0 } else { 0.0 },
                if color.b { 1.0 } else { 0.0 },
            ]
        };

        let mut edge_idx = 0;
        for contour in &outline.contours {
            for edge in &contour.edges {
                let is_selected = self.selected_edge == Some(edge_idx);
                let color = if is_selected {
                    [1.0, 1.0, 1.0] // White for selected
                } else {
                    color_to_rgb(edge.color)
                };

                // Sample points along the edge
                let num_samples = 20;
                for i in 0..num_samples {
                    let t0 = i as f32 / num_samples as f32;
                    let t1 = (i + 1) as f32 / num_samples as f32;

                    let p0 = edge.point_at(t0);
                    let p1 = edge.point_at(t1);

                    // Line segment vertices
                    // Position (x, y), Color (r, g, b)
                    vertices.extend_from_slice(&[
                        map_x(p0.x),
                        map_y(p0.y),
                        color[0],
                        color[1],
                        color[2],
                    ]);
                    vertices.extend_from_slice(&[
                        map_x(p1.x),
                        map_y(p1.y),
                        color[0],
                        color[1],
                        color[2],
                    ]);
                }

                edge_idx += 1;
            }
        }

        // Upload vertices
        if !vertices.is_empty() {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }

    fn render(&mut self) {
        let Some(surface) = &self.surface else {
            return;
        };
        let Some(device) = &self.device else {
            return;
        };
        let Some(queue) = &self.queue else {
            return;
        };
        let Some(pipeline) = &self.render_pipeline else {
            return;
        };

        let output = match surface.get_current_texture() {
            Ok(output) => output,
            Err(_) => return,
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);

            // Draw distance field texture
            if let Some(bind_group) = &self.distance_bind_group {
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }

            // Draw contour lines would need a separate pipeline
            // For simplicity, we'll just show the distance field
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

fn get_edge_by_index(outline: &GlyphOutline, idx: usize) -> Option<&gup::text::msdf::EdgeSegment> {
    let mut current_idx = 0;
    for contour in &outline.contours {
        for edge in &contour.edges {
            if current_idx == idx {
                return Some(edge);
            }
            current_idx += 1;
        }
    }
    None
}

fn median3(a: f32, b: f32, c: f32) -> f32 {
    a.max(b.min(c)).min(a.min(b).max(c))
}

impl ApplicationHandler for MsdfDebugApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        println!("🔧 Initializing MSDF Debug Tool...");

        // Create window
        let window_attrs = WindowAttributes::default()
            .with_title("MSDF Debug Tool")
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        self.window = Some(window.clone());

        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .unwrap();

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .unwrap();

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create distance field texture
        let distance_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Distance Texture"),
            size: wgpu::Extent3d {
                width: DISTANCE_FIELD_SIZE as u32,
                height: DISTANCE_FIELD_SIZE as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = distance_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout and bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertex buffer for contour lines
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 1024 * 1024, // 1MB should be enough
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Initialize MSDF generator
        let msdf_generator =
            MsdfGenerator::new(DEFAULT_FONT_DATA.to_vec(), MsdfConfig::default()).ok();

        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.render_pipeline = Some(render_pipeline);
        self.vertex_buffer = Some(vertex_buffer);
        self.distance_texture = Some(distance_texture);
        self.distance_bind_group = Some(bind_group);
        self.msdf_generator = msdf_generator;

        println!("✅ Initialization complete!");
        self.print_help();
        self.load_glyph(self.current_char);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                match &logical_key {
                    Key::Named(NamedKey::Escape) => {
                        event_loop.exit();
                    }
                    Key::Character(c) => {
                        let c = c.chars().next().unwrap_or('A');
                        match c {
                            '0'..='9' => {
                                let num = c.to_digit(10).unwrap() as usize;
                                self.select_edge(num);
                            }
                            'd' | 'D' => {
                                self.set_distance_mode(DistanceMode::SignedDistance);
                            }
                            'p' | 'P' => {
                                self.set_distance_mode(DistanceMode::PseudoDistance);
                            }
                            'm' | 'M' => {
                                self.set_distance_mode(DistanceMode::MsdfMedian);
                            }
                            'c' | 'C' => {
                                self.show_contours = !self.show_contours;
                                println!(
                                    "📐 Contours: {}",
                                    if self.show_contours { "ON" } else { "OFF" }
                                );
                                self.update_display();
                            }
                            'h' | 'H' | '?' => {
                                self.print_help();
                            }
                            _ => {
                                // Load this character as glyph
                                self.load_glyph(c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

const SHADER_SOURCE: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen quad
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );

    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.tex_coord = tex_coords[vertex_index];
    return output;
}

@group(0) @binding(0) var distance_texture: texture_2d<f32>;
@group(0) @binding(1) var distance_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(distance_texture, distance_sampler, input.tex_coord);
    return color;
}
"#;

fn main() {
    println!("🎨 MSDF Debug Tool - Glyph Geometry and Distance Field Visualizer");
    println!("==================================================================\n");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = MsdfDebugApp::new();
    event_loop.run_app(&mut app).unwrap();
}
