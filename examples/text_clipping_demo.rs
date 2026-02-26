// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text Clipping Visual Demo
//!
//! Demonstrates all text clipping strategies provided by the layout engine:
//!
//! 1. **Truncation with Ellipsis** — text shortened with "…" at various widths
//! 2. **Dynamic Font Scaling** — automatic font-size reduction to fit
//! 3. **Text Repositioning** — nudging text back into a container
//! 4. **Strategy Cascade** — truncation → scaling → hide, applied in order
//! 5. **Side-by-side** — unclipped vs clipped rendering for comparison
//!
//! Container bounds are drawn as coloured rectangles so you can see exactly
//! where the constraints are.
//!
//! Controls:
//! - **C** — toggle clipping on/off globally
//! - **Esc** — exit

use gup::shader_function::Vec2;
use gup::text::{
    ClippingStrategy, ClippingStrategyConfig, FontAtlas, TextBounds, TextLayoutEngine, TextMargins,
    TextRenderConfig, TextRenderer, TextStyle, ViewportBounds,
};
use gup::{GupContext, PhysicalSize, SurfaceId};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

// ── Rectangle drawing helpers ────────────────────────────────────────────

/// GPU vertex for a simple coloured rectangle.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// A rectangle to be drawn as an outline.
#[derive(Clone, Debug)]
struct RectOutline {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    color: [f32; 4],
}

impl RectOutline {
    fn new(left: f32, top: f32, right: f32, bottom: f32, color: [f32; 4]) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
            color,
        }
    }

    /// Generate line-list vertices (8 lines = 16 vertices) forming the outline.
    fn to_vertices(&self, screen_w: f32, screen_h: f32) -> Vec<RectVertex> {
        // Convert pixel coordinates to NDC (-1..1).
        let to_ndc = |px: f32, py: f32| -> [f32; 2] {
            [
                px / screen_w * 2.0 - 1.0,
                -(py / screen_h * 2.0 - 1.0), // y flipped
            ]
        };
        let tl = to_ndc(self.left, self.top);
        let tr = to_ndc(self.right, self.top);
        let br = to_ndc(self.right, self.bottom);
        let bl = to_ndc(self.left, self.bottom);
        let c = self.color;

        // Four edges as line segments (LineList)
        vec![
            RectVertex {
                position: tl,
                color: c,
            },
            RectVertex {
                position: tr,
                color: c,
            },
            RectVertex {
                position: tr,
                color: c,
            },
            RectVertex {
                position: br,
                color: c,
            },
            RectVertex {
                position: br,
                color: c,
            },
            RectVertex {
                position: bl,
                color: c,
            },
            RectVertex {
                position: bl,
                color: c,
            },
            RectVertex {
                position: tl,
                color: c,
            },
        ]
    }
}

/// Manages a simple line-based rectangle pipeline.
struct RectPipeline {
    pipeline: Option<wgpu::RenderPipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,
}

impl RectPipeline {
    fn new() -> Self {
        Self {
            pipeline: None,
            vertex_buffer: None,
            vertex_count: 0,
        }
    }

    fn ensure_pipeline(&mut self, device: &wgpu::Device, surface_format: wgpu::TextureFormat) {
        if self.pipeline.is_some() {
            return;
        }

        let shader_src = r#"
            struct VSOut {
                @builtin(position) pos: vec4<f32>,
                @location(0) color: vec4<f32>,
            };

            @vertex
            fn vs_main(@location(0) position: vec2<f32>,
                       @location(1) color: vec4<f32>) -> VSOut {
                var out: VSOut;
                out.pos = vec4<f32>(position, 0.0, 1.0);
                out.color = color;
                return out;
            }

            @fragment
            fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
                return in.color;
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_outline_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect_outline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        self.pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rect_outline_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<RectVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            wgpu::VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                        ],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }),
        );
    }

    fn upload(&mut self, device: &wgpu::Device, vertices: &[RectVertex]) {
        use wgpu::util::DeviceExt;
        self.vertex_count = vertices.len() as u32;
        if vertices.is_empty() {
            self.vertex_buffer = None;
            return;
        }
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rect_outline_vb"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
    }

    fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if let (Some(pipeline), Some(vb)) = (&self.pipeline, &self.vertex_buffer)
            && self.vertex_count > 0
        {
            render_pass.set_pipeline(pipeline);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.draw(0..self.vertex_count, 0..1);
        }
    }
}

// ── Demo data ────────────────────────────────────────────────────────────

/// A labelled demo section placed at a fixed screen position.
struct Section {
    title: &'static str,
    y_offset: f32,
}

/// Individual demo item: a piece of text inside a bounded container.
struct DemoItem {
    text: &'static str,
    /// Container rectangle in pixel coordinates.
    bounds: TextBounds,
    /// Which strategy (or None = unclipped) to apply.
    strategy: Option<ClippingStrategyConfig>,
    /// Label displayed above the container.
    label: &'static str,
}

// ── Application ──────────────────────────────────────────────────────────

struct App {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    text_renderer: Option<TextRenderer>,
    font_atlas: Option<FontAtlas>,
    layout_engine: Option<TextLayoutEngine>,
    rect_pipeline: RectPipeline,
    clipping_enabled: bool,
}

impl App {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            text_renderer: None,
            font_atlas: None,
            layout_engine: None,
            rect_pipeline: RectPipeline::new(),
            clipping_enabled: true,
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            println!("🔧 Creating GPU context…");
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
        let attrs = WindowAttributes::default()
            .with_title("Gup — Text Clipping Visual Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 700));

        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface_id = SurfaceId::new();

        if let Some(ctx_arc) = self.context.take() {
            let mut ctx = Arc::try_unwrap(ctx_arc).map_err(|_| "Failed to get mutable context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;

            let text_renderer = TextRenderer::new(&ctx.device)?;
            let font_atlas = FontAtlas::new(&ctx.device, &ctx.queue, 32.0)?;
            let layout_engine = TextLayoutEngine::new();

            self.text_renderer = Some(text_renderer);
            self.font_atlas = Some(font_atlas);
            self.layout_engine = Some(layout_engine);
            self.context = Some(Arc::new(ctx));
        }

        self.surface_id = Some(surface_id);
        self.window = Some(window);
        Ok(())
    }

    // ── Build demo items ─────────────────────────────────────────────────

    fn build_sections() -> Vec<Section> {
        vec![
            Section {
                title: "1. Truncation with Ellipsis",
                y_offset: 50.0,
            },
            Section {
                title: "2. Dynamic Font Scaling",
                y_offset: 180.0,
            },
            Section {
                title: "3. Repositioning Near Edges",
                y_offset: 310.0,
            },
            Section {
                title: "4. Strategy Cascade (Truncate → Scale → Hide)",
                y_offset: 440.0,
            },
            Section {
                title: "5. Side-by-side: Unclipped vs Clipped",
                y_offset: 570.0,
            },
        ]
    }

    fn build_demo_items() -> Vec<DemoItem> {
        let sample = "Temperature readings across monitoring stations";
        let truncate_cfg = |_width: f32| ClippingStrategyConfig {
            primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                ellipsis_text: "…".to_string(),
                preserve_words: true,
            },
            fallback_strategies: vec![],
            minimum_visible_percentage: 0.0,
            enable_hover_reveal: false,
        };

        let mut items = Vec::new();

        // ── Section 1: Truncation at different widths ────────────────────
        let base_y = 80.0;
        for (i, width) in [280.0f32, 200.0, 140.0, 90.0].iter().enumerate() {
            let x = 30.0 + i as f32 * 240.0;
            items.push(DemoItem {
                text: sample,
                bounds: TextBounds::new(x, base_y, x + width, base_y + 30.0),
                strategy: Some(truncate_cfg(*width)),
                label: match i {
                    0 => "Wide (280 px)",
                    1 => "Medium (200 px)",
                    2 => "Narrow (140 px)",
                    _ => "Tiny (90 px)",
                },
            });
        }

        // ── Section 2: Dynamic font scaling ──────────────────────────────
        let base_y = 210.0;
        // Before (no clipping — overflows)
        items.push(DemoItem {
            text: sample,
            bounds: TextBounds::new(30.0, base_y, 200.0, base_y + 30.0),
            strategy: None,
            label: "Before (overflows)",
        });
        // After (dynamic font scaling)
        items.push(DemoItem {
            text: sample,
            bounds: TextBounds::new(270.0, base_y, 470.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::DynamicFontScaling {
                    min_font_size: 6.0,
                    scale_factor: 0.1,
                },
                fallback_strategies: vec![],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "After (scaled to fit)",
        });
        // Another scaling comparison
        items.push(DemoItem {
            text: "Average daily energy consumption per household (kWh)",
            bounds: TextBounds::new(510.0, base_y, 710.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::DynamicFontScaling {
                    min_font_size: 8.0,
                    scale_factor: 0.15,
                },
                fallback_strategies: vec![],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Scaled (min 8 px)",
        });

        // ── Section 3: Repositioning ─────────────────────────────────────
        let base_y = 340.0;
        // Text positioned near the right edge — container forces it left
        items.push(DemoItem {
            text: "Edge label pushed left",
            bounds: TextBounds::new(600.0, base_y, 800.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::RepositionText {
                    prefer_directions: vec![Vec2 { x: -1.0, y: 0.0 }, Vec2 { x: 0.0, y: -1.0 }],
                    max_offset_distance: 80.0,
                },
                fallback_strategies: vec![],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Repositioned (← preferred)",
        });
        // Text near the bottom
        items.push(DemoItem {
            text: "Pushed upward",
            bounds: TextBounds::new(30.0, base_y, 200.0, base_y + 25.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::RepositionText {
                    prefer_directions: vec![Vec2 { x: 0.0, y: -1.0 }],
                    max_offset_distance: 60.0,
                },
                fallback_strategies: vec![],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Repositioned (↑ preferred)",
        });

        // ── Section 4: Strategy cascade ──────────────────────────────────
        let base_y = 470.0;
        let cascade_text = "Quarterly revenue breakdown by product category and region";
        // Wide enough for truncation to succeed
        items.push(DemoItem {
            text: cascade_text,
            bounds: TextBounds::new(30.0, base_y, 230.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                    ellipsis_text: "…".to_string(),
                    preserve_words: true,
                },
                fallback_strategies: vec![
                    ClippingStrategy::DynamicFontScaling {
                        min_font_size: 8.0,
                        scale_factor: 0.1,
                    },
                    ClippingStrategy::HideIfClipped {
                        min_visible_threshold: 0.3,
                    },
                ],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Cascade: truncated",
        });
        // Narrow — falls through to scaling
        items.push(DemoItem {
            text: cascade_text,
            bounds: TextBounds::new(270.0, base_y, 370.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                    ellipsis_text: "…".to_string(),
                    preserve_words: true,
                },
                fallback_strategies: vec![
                    ClippingStrategy::DynamicFontScaling {
                        min_font_size: 6.0,
                        scale_factor: 0.15,
                    },
                    ClippingStrategy::HideIfClipped {
                        min_visible_threshold: 0.3,
                    },
                ],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Cascade: scaled",
        });
        // Very narrow — hidden
        items.push(DemoItem {
            text: cascade_text,
            bounds: TextBounds::new(410.0, base_y, 440.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                    ellipsis_text: "…".to_string(),
                    preserve_words: true,
                },
                fallback_strategies: vec![
                    ClippingStrategy::DynamicFontScaling {
                        min_font_size: 10.0,
                        scale_factor: 0.1,
                    },
                    ClippingStrategy::HideIfClipped {
                        min_visible_threshold: 0.3,
                    },
                ],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Cascade: hidden",
        });

        // ── Section 5: Side-by-side unclipped vs clipped ─────────────────
        let base_y = 600.0;
        let compare_text = "Infrastructure costs — cloud services breakdown";
        items.push(DemoItem {
            text: compare_text,
            bounds: TextBounds::new(30.0, base_y, 230.0, base_y + 30.0),
            strategy: None,
            label: "Unclipped (overflows)",
        });
        items.push(DemoItem {
            text: compare_text,
            bounds: TextBounds::new(500.0, base_y, 700.0, base_y + 30.0),
            strategy: Some(ClippingStrategyConfig {
                primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                    ellipsis_text: "…".to_string(),
                    preserve_words: true,
                },
                fallback_strategies: vec![],
                minimum_visible_percentage: 0.0,
                enable_hover_reveal: false,
            }),
            label: "Clipped (truncated)",
        });

        items
    }

    // ── Render frame ─────────────────────────────────────────────────────

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let surface_id = match self.surface_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let ctx_arc = match self.context.take() {
            Some(c) => c,
            None => return Ok(()),
        };
        let mut ctx = Arc::try_unwrap(ctx_arc).map_err(|_| "Failed to get mutable context")?;

        let surface_format = ctx
            .surface_format_for(surface_id)
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
        let (screen_w, screen_h) = ctx
            .surface_size(surface_id)
            .map(|s| (s.width as f32, s.height as f32))
            .unwrap_or((1000.0, 700.0));

        match ctx.begin_frame_for_surface(surface_id) {
            Ok(mut frame) => {
                let device = frame.device_arc();
                let queue = frame.queue_arc();

                self.rect_pipeline.ensure_pipeline(&device, surface_format);

                if let (Some(text_renderer), Some(font_atlas), Some(layout_engine)) = (
                    &mut self.text_renderer,
                    &mut self.font_atlas,
                    &mut self.layout_engine,
                ) {
                    text_renderer.begin_frame();

                    let sections = Self::build_sections();
                    let items = Self::build_demo_items();

                    let title_style = TextStyle::new(16.0).with_rgba(0.15, 0.15, 0.4, 1.0);
                    let label_style = TextStyle::new(10.0).with_rgba(0.4, 0.4, 0.4, 1.0);
                    let body_style = TextStyle::new(14.0).with_rgba(0.1, 0.1, 0.1, 1.0);
                    let status_style = TextStyle::new(12.0).with_rgba(0.2, 0.5, 0.2, 1.0);

                    // ── Queue section titles ────────────────────────────
                    for section in &sections {
                        let mut cfg = TextRenderConfig {
                            text: section.title,
                            position: Vec2 {
                                x: 30.0,
                                y: section.y_offset,
                            },
                            style: &title_style,
                            font_atlas,
                            layout_engine,
                            screen_width: screen_w,
                            screen_height: screen_h,
                            viewport_bounds: None,
                            clipping_config: None,
                        };
                        let _ = text_renderer.queue_text(&frame, &mut cfg);
                    }

                    // ── Status bar ───────────────────────────────────────
                    let status_text = if self.clipping_enabled {
                        "Clipping: ON  (press C to toggle)"
                    } else {
                        "Clipping: OFF (press C to toggle)"
                    };
                    let mut cfg = TextRenderConfig {
                        text: status_text,
                        position: Vec2 { x: 30.0, y: 15.0 },
                        style: &status_style,
                        font_atlas,
                        layout_engine,
                        screen_width: screen_w,
                        screen_height: screen_h,
                        viewport_bounds: None,
                        clipping_config: None,
                    };
                    let _ = text_renderer.queue_text(&frame, &mut cfg);

                    // ── Collect rectangle outlines ───────────────────────
                    let mut rects: Vec<RectOutline> = Vec::new();
                    let container_color = [0.55, 0.55, 0.8, 0.7];

                    // Clipping statistics
                    let mut stats_total = 0u32;
                    let mut stats_clipped = 0u32;
                    let mut stats_truncated = 0u32;
                    let mut stats_scaled = 0u32;
                    let mut stats_hidden = 0u32;
                    let mut stats_unclipped = 0u32;

                    // ── Queue demo items ─────────────────────────────────
                    for item in &items {
                        // Item label
                        let mut label_cfg = TextRenderConfig {
                            text: item.label,
                            position: Vec2 {
                                x: item.bounds.left,
                                y: item.bounds.top - 14.0,
                            },
                            style: &label_style,
                            font_atlas,
                            layout_engine,
                            screen_width: screen_w,
                            screen_height: screen_h,
                            viewport_bounds: None,
                            clipping_config: None,
                        };
                        let _ = text_renderer.queue_text(&frame, &mut label_cfg);

                        // Container outline
                        rects.push(RectOutline::new(
                            item.bounds.left,
                            item.bounds.top,
                            item.bounds.right,
                            item.bounds.bottom,
                            container_color,
                        ));

                        stats_total += 1;

                        // Decide whether to apply clipping
                        let use_clipping = self.clipping_enabled && item.strategy.is_some();

                        if use_clipping {
                            let strategy = item.strategy.as_ref().unwrap();
                            let viewport = ViewportBounds::from_container(item.bounds)
                                .with_margins(TextMargins::zero());
                            let mut text_cfg = TextRenderConfig {
                                text: item.text,
                                position: Vec2 {
                                    x: item.bounds.left,
                                    y: item.bounds.top,
                                },
                                style: &body_style,
                                font_atlas,
                                layout_engine,
                                screen_width: screen_w,
                                screen_height: screen_h,
                                viewport_bounds: Some(&viewport),
                                clipping_config: Some(strategy),
                            };
                            if let Ok(result) = text_renderer.queue_text(&frame, &mut text_cfg) {
                                if result.clipped {
                                    stats_clipped += 1;
                                    // Classify strategy used (approximate from label)
                                    if result.glyphs.is_empty() {
                                        stats_hidden += 1;
                                    } else if item.label.contains("scale")
                                        || item.label.contains("Scale")
                                        || item.label.contains("scaled")
                                    {
                                        stats_scaled += 1;
                                    } else {
                                        stats_truncated += 1;
                                    }
                                } else {
                                    stats_unclipped += 1;
                                }
                            }
                        } else {
                            // Render without any clipping
                            stats_unclipped += 1;
                            let mut text_cfg = TextRenderConfig {
                                text: item.text,
                                position: Vec2 {
                                    x: item.bounds.left,
                                    y: item.bounds.top,
                                },
                                style: &body_style,
                                font_atlas,
                                layout_engine,
                                screen_width: screen_w,
                                screen_height: screen_h,
                                viewport_bounds: None,
                                clipping_config: None,
                            };
                            let _ = text_renderer.queue_text(&frame, &mut text_cfg);
                        }
                    }

                    // ── Queue statistics overlay ─────────────────────────
                    let stats_text = format!(
                        "Items: {}  |  Clipped: {}  (truncated: {}, scaled: {}, hidden: {})  |  Unclipped: {}",
                        stats_total,
                        stats_clipped,
                        stats_truncated,
                        stats_scaled,
                        stats_hidden,
                        stats_unclipped
                    );
                    let stats_display_style = TextStyle::new(10.0).with_rgba(0.35, 0.35, 0.35, 1.0);
                    let mut stats_cfg = TextRenderConfig {
                        text: &stats_text,
                        position: Vec2 {
                            x: 30.0,
                            y: screen_h - 20.0,
                        },
                        style: &stats_display_style,
                        font_atlas,
                        layout_engine,
                        screen_width: screen_w,
                        screen_height: screen_h,
                        viewport_bounds: None,
                        clipping_config: None,
                    };
                    let _ = text_renderer.queue_text(&frame, &mut stats_cfg);

                    // ── Upload rectangle vertices ────────────────────────
                    let rect_verts: Vec<RectVertex> = rects
                        .iter()
                        .flat_map(|r| r.to_vertices(screen_w, screen_h))
                        .collect();
                    self.rect_pipeline.upload(&device, &rect_verts);

                    // ── Render pass ──────────────────────────────────────
                    let clear_color = wgpu::Color {
                        r: 0.97,
                        g: 0.97,
                        b: 0.98,
                        a: 1.0,
                    };
                    {
                        let mut render_pass = frame.render_pass(Some(clear_color));

                        // Draw container outlines first
                        self.rect_pipeline.draw(&mut render_pass);

                        // Then text on top
                        let _ = text_renderer.render_queued_text(
                            &mut render_pass,
                            &device,
                            &queue,
                            font_atlas,
                            screen_w,
                            screen_h,
                        );
                    }

                    frame.finish()?;
                }
            }
            Err(e) => {
                eprintln!("Frame error: {e}");
            }
        }

        self.context = Some(Arc::new(ctx));
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("Failed to create GPU context: {e}");
                event_loop.exit();
                return;
            }
            if let Err(e) = self.create_window(event_loop) {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match code {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::KeyC => {
                    self.clipping_enabled = !self.clipping_enabled;
                    println!(
                        "Clipping {}",
                        if self.clipping_enabled { "ON" } else { "OFF" }
                    );
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                _ => {}
            },
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id
                    && let Some(ctx_arc) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(ctx_arc)
                {
                    let _ =
                        ctx.resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(ctx));
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("Render error: {e}");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📐 Text Clipping Visual Demo");
    println!("   Demonstrates all text clipping strategies.");
    println!("   Press C to toggle clipping on/off, Esc to exit.\n");

    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_initialises() {
        let app = App::new();
        assert!(app.clipping_enabled);
        assert!(app.context.is_none());
    }

    #[test]
    fn test_sections_defined() {
        let sections = App::build_sections();
        assert_eq!(sections.len(), 5);
    }

    #[test]
    fn test_demo_items_defined() {
        let items = App::build_demo_items();
        // Sections 1-5 should produce items
        assert!(
            items.len() >= 10,
            "Expected ≥10 demo items, got {}",
            items.len()
        );

        // Verify mix of clipped and unclipped
        let clipped = items.iter().filter(|i| i.strategy.is_some()).count();
        let unclipped = items.iter().filter(|i| i.strategy.is_none()).count();
        assert!(clipped > 0, "Should have clipped items");
        assert!(unclipped > 0, "Should have unclipped items");
    }

    #[test]
    fn test_rect_outline_vertices() {
        let rect = RectOutline::new(100.0, 100.0, 300.0, 200.0, [1.0, 0.0, 0.0, 1.0]);
        let verts = rect.to_vertices(800.0, 600.0);
        // 4 edges × 2 vertices per line segment = 8
        assert_eq!(verts.len(), 8);
        for v in &verts {
            assert!(v.position[0] >= -1.0 && v.position[0] <= 1.0);
            assert!(v.position[1] >= -1.0 && v.position[1] <= 1.0);
        }
    }

    #[test]
    fn test_toggle_clipping() {
        let mut app = App::new();
        assert!(app.clipping_enabled);
        app.clipping_enabled = !app.clipping_enabled;
        assert!(!app.clipping_enabled);
    }
}
