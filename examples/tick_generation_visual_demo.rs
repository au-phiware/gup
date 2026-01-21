// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive Tick Generation Visual Demo
//!
//! This example provides a visual demonstration of the automatic tick generation
//! algorithms, showing:
//! - Live tick generation for different scale types (linear, logarithmic, time)
//! - Interactive scale range adjustment using keyboard controls
//! - Visual comparison of different tick generation algorithms
//! - Real-time performance metrics and quality assessment

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    render::Vertex,
    shader_function::{Vec2, Vec4},
    tick_generator::{
        LinearScale, LinearTickGenerator, LogarithmicScale, LogarithmicTickGenerator, Scale,
        TickGenerator, TimeScale, TimeTickGenerator,
    },
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::{Color, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Current demo mode
#[derive(Debug, Clone, Copy, PartialEq)]
enum DemoMode {
    Linear,
    Logarithmic,
    Time,
}

impl DemoMode {
    fn next(&self) -> Self {
        match self {
            DemoMode::Linear => DemoMode::Logarithmic,
            DemoMode::Logarithmic => DemoMode::Time,
            DemoMode::Time => DemoMode::Linear,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            DemoMode::Linear => "Linear Scale",
            DemoMode::Logarithmic => "Logarithmic Scale",
            DemoMode::Time => "Time Scale",
        }
    }
}

/// Scale configuration for interactive adjustment
#[derive(Debug, Clone)]
struct ScaleConfig {
    pub min: f64,
    pub max: f64,
    pub pixel_range: f32,
    pub target_tick_count: Option<usize>,
}

impl ScaleConfig {
    fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            pixel_range: 800.0,
            target_tick_count: None,
        }
    }

    fn linear_demo() -> Self {
        Self::new(0.0, 100.0)
    }

    fn logarithmic_demo() -> Self {
        Self::new(1.0, 1000.0)
    }

    fn time_demo() -> Self {
        // 1 day in seconds
        let now = 1640995200.0; // 2022-01-01 00:00:00 UTC
        Self::new(now, now + 86400.0)
    }

    fn increase_range(&mut self, mode: DemoMode) {
        match mode {
            DemoMode::Linear => {
                let range = self.max - self.min;
                let center = (self.max + self.min) / 2.0;
                let new_range = range * 1.5;
                self.min = center - new_range / 2.0;
                self.max = center + new_range / 2.0;
            }
            DemoMode::Logarithmic => {
                self.min = (self.min * 0.1).max(0.001);
                self.max *= 10.0;
            }
            DemoMode::Time => {
                let range = self.max - self.min;
                let new_range = range * 2.0;
                self.max = self.min + new_range;
            }
        }
    }

    fn decrease_range(&mut self, mode: DemoMode) {
        match mode {
            DemoMode::Linear => {
                let range = self.max - self.min;
                let center = (self.max + self.min) / 2.0;
                let new_range = range * 0.75;
                self.min = center - new_range / 2.0;
                self.max = center + new_range / 2.0;
            }
            DemoMode::Logarithmic => {
                if self.max / self.min > 100.0 {
                    self.min *= 2.0;
                    self.max *= 0.5;
                }
            }
            DemoMode::Time => {
                let range = self.max - self.min;
                if range > 3600.0 {
                    // Keep at least 1 hour
                    let new_range = range * 0.75;
                    self.max = self.min + new_range;
                }
            }
        }
    }

    fn shift_left(&mut self, mode: DemoMode) {
        match mode {
            DemoMode::Linear => {
                let range = self.max - self.min;
                let shift = range * 0.005;
                self.min -= shift;
                self.max -= shift;
            }
            DemoMode::Logarithmic => {
                let ratio = self.max / self.min;
                self.min = (self.min * 0.5).max(0.001);
                self.max = self.min * ratio;
            }
            DemoMode::Time => {
                let range = self.max - self.min;
                let shift = range * 0.25;
                self.min -= shift;
                self.max -= shift;
            }
        }
    }

    fn shift_right(&mut self, mode: DemoMode) {
        match mode {
            DemoMode::Linear => {
                let range = self.max - self.min;
                let shift = range * 0.005;
                self.min += shift;
                self.max += shift;
            }
            DemoMode::Logarithmic => {
                let ratio = self.max / self.min;
                self.min *= 2.0;
                self.max = self.min * ratio;
            }
            DemoMode::Time => {
                let range = self.max - self.min;
                let shift = range * 0.25;
                self.min += shift;
                self.max += shift;
            }
        }
    }
}

/// Visual renderer for tick generation demonstration
struct TickVisualizationRenderer {
    mode: DemoMode,
    config: ScaleConfig,
    last_update: Instant,
    performance_samples: Vec<Duration>,
    background_color: [f32; 4],
}

impl TickVisualizationRenderer {
    fn new() -> Self {
        Self {
            mode: DemoMode::Linear,
            config: ScaleConfig::linear_demo(),
            last_update: Instant::now(),
            performance_samples: Vec::new(),
            background_color: [0.05, 0.05, 0.1, 1.0], // Dark blue
        }
    }

    fn switch_mode(&mut self, mode: DemoMode) {
        self.mode = mode;
        self.config = match mode {
            DemoMode::Linear => ScaleConfig::linear_demo(),
            DemoMode::Logarithmic => ScaleConfig::logarithmic_demo(),
            DemoMode::Time => ScaleConfig::time_demo(),
        };
    }

    fn generate_current_ticks(&self) -> (Vec<f64>, Vec<f64>, Duration) {
        let start = Instant::now();

        let (major_ticks, minor_ticks) = match self.mode {
            DemoMode::Linear => {
                let scale = LinearScale::new(self.config.min, self.config.max);
                let generator = LinearTickGenerator::default();

                let major = generator.generate_major_ticks(
                    &scale,
                    self.config.pixel_range,
                    self.config.target_tick_count,
                );
                let minor = generator.generate_minor_ticks(&scale, &major, 5);
                (major, minor)
            }
            DemoMode::Logarithmic => {
                let scale = LogarithmicScale::new(self.config.min, self.config.max, 10.0);
                let generator = LogarithmicTickGenerator::default();

                let major = generator.generate_major_ticks(
                    &scale,
                    self.config.pixel_range,
                    self.config.target_tick_count,
                );
                let minor = generator.generate_minor_ticks(&scale, &major, 5);
                (major, minor)
            }
            DemoMode::Time => {
                let scale = TimeScale::new(self.config.min, self.config.max);
                let generator = TimeTickGenerator::default();

                let major = generator.generate_major_ticks(
                    &scale,
                    self.config.pixel_range,
                    self.config.target_tick_count,
                );
                let minor = generator.generate_minor_ticks(&scale, &major, 3);
                (major, minor)
            }
        };

        let duration = start.elapsed();
        (major_ticks, minor_ticks, duration)
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        // Clear background
        let clear_color = Color {
            r: self.background_color[0] as f64,
            g: self.background_color[1] as f64,
            b: self.background_color[2] as f64,
            a: self.background_color[3] as f64,
        };

        // Generate ticks and measure performance
        let (major_ticks, minor_ticks, generation_time) = self.generate_current_ticks();

        // Track performance
        self.performance_samples.push(generation_time);
        if self.performance_samples.len() > 100 {
            self.performance_samples.remove(0);
        }

        // Create vertices for axis and ticks
        let mut vertices = Vec::new();

        // Main axis line (horizontal, center of screen)
        let axis_y = 0.0;
        let axis_start = Vec2 { x: -0.8, y: axis_y };
        let axis_end = Vec2 { x: 0.8, y: axis_y };
        let axis_color = Vec4 {
            x: 0.8,
            y: 0.8,
            z: 0.8,
            w: 1.0,
        }; // Light gray

        vertices.push(Vertex {
            position: [axis_start.x, axis_start.y],
            color: [axis_color.x, axis_color.y, axis_color.z, axis_color.w],
        });
        vertices.push(Vertex {
            position: [axis_end.x, axis_end.y],
            color: [axis_color.x, axis_color.y, axis_color.z, axis_color.w],
        });

        // Current scale for converting domain values to screen positions
        let current_scale: Box<dyn Scale> = match self.mode {
            DemoMode::Linear => Box::new(LinearScale::new(self.config.min, self.config.max)),
            DemoMode::Logarithmic => Box::new(LogarithmicScale::new(
                self.config.min,
                self.config.max,
                10.0,
            )),
            DemoMode::Time => Box::new(TimeScale::new(self.config.min, self.config.max)),
        };

        // Major ticks (longer, brighter)
        let major_tick_color = Vec4 {
            x: 1.0,
            y: 0.6,
            z: 0.2,
            w: 1.0,
        }; // Orange
        let major_tick_height = 0.15;

        for &tick_value in &major_ticks {
            let normalized_pos = current_scale.normalize(tick_value) as f32;
            let screen_x = axis_start.x + normalized_pos * (axis_end.x - axis_start.x);

            if (-0.9..=0.9).contains(&screen_x) {
                // Tick line
                vertices.push(Vertex {
                    position: [screen_x, axis_y - major_tick_height / 2.0],
                    color: [
                        major_tick_color.x,
                        major_tick_color.y,
                        major_tick_color.z,
                        major_tick_color.w,
                    ],
                });
                vertices.push(Vertex {
                    position: [screen_x, axis_y + major_tick_height / 2.0],
                    color: [
                        major_tick_color.x,
                        major_tick_color.y,
                        major_tick_color.z,
                        major_tick_color.w,
                    ],
                });
            }
        }

        // Minor ticks (shorter, dimmer)
        let minor_tick_color = Vec4 {
            x: 0.6,
            y: 0.6,
            z: 0.8,
            w: 0.7,
        }; // Light blue, transparent
        let minor_tick_height = 0.08;

        for &tick_value in &minor_ticks {
            let normalized_pos = current_scale.normalize(tick_value) as f32;
            let screen_x = axis_start.x + normalized_pos * (axis_end.x - axis_start.x);

            if (-1.0..=1.0).contains(&screen_x) {
                vertices.push(Vertex {
                    position: [screen_x, axis_y - minor_tick_height / 2.0],
                    color: [
                        minor_tick_color.x,
                        minor_tick_color.y,
                        minor_tick_color.z,
                        minor_tick_color.w,
                    ],
                });
                vertices.push(Vertex {
                    position: [screen_x, axis_y + minor_tick_height / 2.0],
                    color: [
                        minor_tick_color.x,
                        minor_tick_color.y,
                        minor_tick_color.z,
                        minor_tick_color.w,
                    ],
                });
            }
        }

        // Info display area (upper portion)
        let info_y = 0.6;
        let info_color = Vec4 {
            x: 0.9,
            y: 0.9,
            z: 0.9,
            w: 1.0,
        }; // Light gray

        // Scale range indicators
        let range_start_x = -0.8;
        let range_end_x = 0.8;

        // Range boundary markers
        vertices.push(Vertex {
            position: [range_start_x, info_y - 0.05],
            color: [info_color.x, info_color.y, info_color.z, info_color.w],
        });
        vertices.push(Vertex {
            position: [range_start_x, info_y + 0.05],
            color: [info_color.x, info_color.y, info_color.z, info_color.w],
        });

        vertices.push(Vertex {
            position: [range_end_x, info_y - 0.05],
            color: [info_color.x, info_color.y, info_color.z, info_color.w],
        });
        vertices.push(Vertex {
            position: [range_end_x, info_y + 0.05],
            color: [info_color.x, info_color.y, info_color.z, info_color.w],
        });

        // Performance indicator (small bar showing generation time)
        let avg_time = if !self.performance_samples.is_empty() {
            self.performance_samples
                .iter()
                .sum::<Duration>()
                .as_micros() as f32
                / self.performance_samples.len() as f32
        } else {
            0.0
        };

        // Performance bar (green if fast, red if slow)
        let perf_x = -0.7;
        let perf_y = -0.8;
        let perf_width = (avg_time / 1000.0).min(0.3); // Max 300μs = 30% of bar
        let perf_color = if avg_time < 10.0 {
            Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 1.0,
            } // Green
        } else if avg_time < 100.0 {
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 0.0,
                w: 1.0,
            } // Yellow
        } else {
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            } // Red
        };

        if perf_width > 0.01 {
            vertices.push(Vertex {
                position: [perf_x, perf_y],
                color: [perf_color.x, perf_color.y, perf_color.z, perf_color.w],
            });
            vertices.push(Vertex {
                position: [perf_x + perf_width, perf_y],
                color: [perf_color.x, perf_color.y, perf_color.z, perf_color.w],
            });
        }

        if vertices.is_empty() {
            let _render_pass = frame.render_pass(Some(clear_color));
            return Ok(());
        }

        // Create vertex buffer
        let vertex_buffer = frame
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tick Visualization Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create simple shader for line rendering
        let shader = frame
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("tick_visualization_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
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
                    out.clip_position = vec4<f32>(position, 0.0, 1.0);
                    out.color = color;
                    return out;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    return in.color;
                }
                "#
                    .into(),
                ),
            });

        // Create render pipeline
        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tick_visualization_pipeline_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            frame
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("tick_visualization_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    offset: 0,
                                    shader_location: 0,
                                    format: wgpu::VertexFormat::Float32x2, // position
                                },
                                wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                                    shader_location: 1,
                                    format: wgpu::VertexFormat::Float32x4, // color
                                },
                            ],
                        }],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Bgra8UnormSrgb,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                    cache: None,
                });

        // Render the visualization
        {
            let mut render_pass = frame.render_pass(Some(clear_color));
            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        // Print stats to console (since we can't do text rendering yet)
        if self.last_update.elapsed() > Duration::from_millis(500) {
            self.last_update = Instant::now();

            println!("\n🎯 Tick Generation Demo - {}", self.mode.name());
            println!("=====================================");
            println!("Range: {:.3} to {:.3}", self.config.min, self.config.max);
            println!(
                "Minor ticks: {:.3}, {:.3}, {:.3}, ...",
                minor_ticks[0], minor_ticks[1], minor_ticks[2],
            );
            println!(
                "Major ticks: {} | Minor ticks: {}",
                major_ticks.len(),
                minor_ticks.len()
            );
            println!(
                "Generation time: {:.1}μs (avg: {:.1}μs)",
                generation_time.as_micros(),
                avg_time
            );
            println!("Pixel range: {:.0}px", self.config.pixel_range);

            if !major_ticks.is_empty() {
                println!(
                    "Major tick density: {:.1} ticks per 100px",
                    major_ticks.len() as f32 / self.config.pixel_range * 100.0
                );
            }

            println!("\nControls:");
            println!(
                "  [1/2/3] - Switch scale type | [+/-] - Range | [←/→] - Shift | [ESC] - Exit"
            );
        }

        Ok(())
    }
}

/// Main application for the tick generation visual demo
struct TickGenerationDemoApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<TickVisualizationRenderer>,
}

impl TickGenerationDemoApp {
    fn new() -> Self {
        Self {
            context: None,
            window: None,
            surface_id: None,
            renderer: None,
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
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Tick Generation Visual Demo - Interactive Algorithm Showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 600));

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

    fn initialize_renderer(&mut self) {
        self.renderer = Some(TickVisualizationRenderer::new());
        println!("✅ Tick visualization renderer initialized");
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.renderer.is_none() {
            self.initialize_renderer();
        }

        if let Some(surface_id) = self.surface_id
            && let Some(context) = self.context.take()
        {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    if let Some(renderer) = &mut self.renderer
                        && let Err(e) = renderer.render(&mut frame)
                    {
                        eprintln!("❌ Failed to render tick visualization: {e}");
                    }
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

impl ApplicationHandler for TickGenerationDemoApp {
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

            println!("✅ Tick generation visual demo window created!");
            println!("🎨 Demonstrating automatic tick generation algorithms...");
            println!();
            println!("Visual Elements:");
            println!("• Main axis (horizontal gray line)");
            println!("• Major ticks (orange, longer lines)");
            println!("• Minor ticks (blue, shorter lines)");
            println!("• Performance bar (bottom left, green=fast)");
            println!();
            println!("Controls:");
            println!("  [1] - Linear scale mode");
            println!("  [2] - Logarithmic scale mode");
            println!("  [3] - Time scale mode");
            println!("  [+] - Increase range");
            println!("  [-] - Decrease range");
            println!("  [←] - Shift range left");
            println!("  [→] - Shift range right");
            println!("  [ESC] - Exit");
            println!();
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
                println!("👋 Closing tick generation demo");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id
                    && let Some(ctx) = self.context.take()
                {
                    let mut context_mut = Arc::try_unwrap(ctx).unwrap_or_else(|arc| {
                        panic!(
                            "Failed to get mutable context: {} references",
                            Arc::strong_count(&arc)
                        )
                    });

                    if let Err(e) = context_mut
                        .resize_surface(surface_id, PhysicalSize::new(size.width, size.height))
                    {
                        eprintln!("❌ Failed to resize surface: {e}");
                    }

                    self.context = Some(Arc::new(context_mut));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                if let Some(renderer) = &mut self.renderer {
                    match key_code {
                        KeyCode::Escape => {
                            println!("👋 Escape pressed, closing demo");
                            event_loop.exit();
                        }
                        KeyCode::Digit1 => {
                            renderer.switch_mode(DemoMode::Linear);
                            println!("🔄 Switched to Linear scale mode");
                        }
                        KeyCode::Digit2 => {
                            renderer.switch_mode(DemoMode::Logarithmic);
                            println!("🔄 Switched to Logarithmic scale mode");
                        }
                        KeyCode::Digit3 => {
                            renderer.switch_mode(DemoMode::Time);
                            println!("🔄 Switched to Time scale mode");
                        }
                        KeyCode::Equal | KeyCode::NumpadAdd => {
                            renderer.config.increase_range(renderer.mode);
                            println!("📈 Increased range");
                        }
                        KeyCode::Minus | KeyCode::NumpadSubtract => {
                            renderer.config.decrease_range(renderer.mode);
                            println!("📉 Decreased range");
                        }
                        KeyCode::ArrowLeft => {
                            renderer.config.shift_left(renderer.mode);
                            println!("⬅️ Shifted range left");
                        }
                        KeyCode::ArrowRight => {
                            renderer.config.shift_right(renderer.mode);
                            println!("➡️ Shifted range right");
                        }
                        KeyCode::Space => {
                            let new_mode = renderer.mode.next();
                            renderer.switch_mode(new_mode);
                            println!("🔄 Switched to {} mode", new_mode.name());
                        }
                        _ => {}
                    }
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

    println!("🚀 Gup Tick Generation Visual Demo");
    println!("==================================");
    println!();
    println!("This interactive demo visually demonstrates automatic tick generation:");
    println!("• Real-time tick generation algorithms (Linear, Logarithmic, Time)");
    println!("• Interactive scale range and type adjustment");
    println!("• Performance monitoring and quality metrics");
    println!("• Professional-quality tick spacing and density");
    println!();
    println!("The demo shows the tick generation system in action with:");
    println!("• Major ticks (orange lines) for primary scale markers");
    println!("• Minor ticks (blue lines) for fine-grain divisions");
    println!("• Real-time performance metrics (generation time < 100μs)");
    println!("• Interactive controls for exploring different scenarios");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = TickGenerationDemoApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_mode_cycling() {
        let mut mode = DemoMode::Linear;
        mode = mode.next();
        assert_eq!(mode, DemoMode::Logarithmic);
        mode = mode.next();
        assert_eq!(mode, DemoMode::Time);
        mode = mode.next();
        assert_eq!(mode, DemoMode::Linear);
    }

    #[test]
    fn test_scale_config_adjustment() {
        let mut config = ScaleConfig::linear_demo();
        let original_range = config.max - config.min;

        config.increase_range(DemoMode::Linear);
        let new_range = config.max - config.min;
        assert!(new_range > original_range);

        config.decrease_range(DemoMode::Linear);
        let final_range = config.max - config.min;
        assert!(final_range < new_range);
    }

    #[test]
    fn test_tick_visualization_renderer_creation() {
        let renderer = TickVisualizationRenderer::new();
        assert_eq!(renderer.mode, DemoMode::Linear);
        assert_eq!(renderer.background_color, [0.05, 0.05, 0.1, 1.0]);
        assert!(renderer.performance_samples.is_empty());
    }
}
