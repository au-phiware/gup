// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive Clipping Reveal Demo
//!
//! Demonstrates the hover reveal system for truncated/clipped text.
//! Labels are deliberately too long for their containers and are truncated
//! with ellipsis. Hovering over a truncated label reveals the full text
//! in a tooltip.
//!
//! Features demonstrated:
//! - Text truncation with ellipsis via clipping strategies
//! - Hover detection on clipped text via `ClippedTextRegistry`
//! - Smooth tooltip fade-in/fade-out via `HoverRevealState`
//! - Tooltip layout positioning and screen-edge clamping
//!
//! Controls:
//! - Move the mouse over truncated labels to see the full text
//! - Press Esc to exit

use gup::shader_function::Vec2;
use gup::text::hover_reveal::{
    ClippedTextRegistry, HoverRevealState, TooltipConfig, compute_tooltip_layout,
};
use gup::text::{
    ClippingStrategy, ClippingStrategyConfig, FontAtlas, TextLayoutEngine, TextRenderConfig,
    TextRenderer, TextStyle, ViewportBounds,
};
use gup::{GupContext, PhysicalSize, SurfaceId};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// Demo label with position and constrained width.
struct Label {
    text: String,
    position: Vec2,
    max_width: f32,
}

/// Application state.
struct App {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    text_renderer: Option<TextRenderer>,
    font_atlas: Option<FontAtlas>,
    layout_engine: Option<TextLayoutEngine>,

    // Hover reveal state
    registry: ClippedTextRegistry,
    hover_state: HoverRevealState,

    // Input state
    mouse_x: f32,
    mouse_y: f32,
    last_frame_time: Instant,

    // Demo labels
    labels: Vec<Label>,
}

impl App {
    fn new() -> Self {
        let labels = vec![
            Label {
                text: "Revenue per Quarter (2024) — North America Region".into(),
                position: Vec2 { x: 40.0, y: 60.0 },
                max_width: 150.0,
            },
            Label {
                text: "Customer Acquisition Cost vs. Lifetime Value".into(),
                position: Vec2 { x: 40.0, y: 110.0 },
                max_width: 120.0,
            },
            Label {
                text: "Employee Satisfaction Survey Results — Engineering".into(),
                position: Vec2 { x: 40.0, y: 160.0 },
                max_width: 180.0,
            },
            Label {
                text: "Monthly Active Users (Mobile + Desktop)".into(),
                position: Vec2 { x: 40.0, y: 210.0 },
                max_width: 160.0,
            },
            Label {
                text: "Infrastructure Costs — AWS + GCP Combined".into(),
                position: Vec2 { x: 40.0, y: 260.0 },
                max_width: 100.0,
            },
            Label {
                text: "Short".into(),
                position: Vec2 { x: 40.0, y: 310.0 },
                max_width: 200.0,
            },
            Label {
                text: "Net Promoter Score (NPS) — Quarterly Trend".into(),
                position: Vec2 { x: 400.0, y: 60.0 },
                max_width: 130.0,
            },
            Label {
                text: "Production Defect Rate per Sprint".into(),
                position: Vec2 { x: 400.0, y: 110.0 },
                max_width: 140.0,
            },
        ];

        Self {
            context: None,
            surface_id: None,
            window: None,
            text_renderer: None,
            font_atlas: None,
            layout_engine: None,
            registry: ClippedTextRegistry::new(),
            hover_state: HoverRevealState::new(TooltipConfig {
                show_delay: 0.3,
                fade_in_duration: 0.15,
                fade_out_duration: 0.1,
                font_size: 13.0,
                ..Default::default()
            }),
            mouse_x: 0.0,
            mouse_y: 0.0,
            last_frame_time: Instant::now(),
            labels,
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
            .with_title("Gup — Interactive Clipping Reveal Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 500));

        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface_id = SurfaceId::new();

        // Add surface to context
        if let Some(ctx_arc) = self.context.take() {
            let mut ctx = Arc::try_unwrap(ctx_arc).map_err(|_| "Failed to get mutable context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;

            // Initialise rendering components
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

        let (screen_w, screen_h) = ctx
            .surface_size(surface_id)
            .map(|s| (s.width as f32, s.height as f32))
            .unwrap_or((800.0, 500.0));

        match ctx.begin_frame_for_surface(surface_id) {
            Ok(mut frame) => {
                let device = frame.device_arc();
                let queue = frame.queue_arc();

                // ── Delta time ───────────────────────────────────────
                let now = Instant::now();
                let dt = (now - self.last_frame_time).as_secs_f32().min(0.1);
                self.last_frame_time = now;

                if let (Some(text_renderer), Some(font_atlas), Some(layout_engine)) = (
                    &mut self.text_renderer,
                    &mut self.font_atlas,
                    &mut self.layout_engine,
                ) {
                    // Clear per-frame state
                    self.registry.clear();
                    text_renderer.begin_frame();

                    // ── Queue title ──────────────────────────────────
                    let title_style = TextStyle::new(20.0).with_rgba(0.2, 0.2, 0.55, 1.0);
                    let mut title_cfg = TextRenderConfig {
                        text: "Hover over truncated labels to reveal full text",
                        position: Vec2 { x: 40.0, y: 20.0 },
                        style: &title_style,
                        font_atlas,
                        layout_engine,
                        screen_width: screen_w,
                        screen_height: screen_h,
                        viewport_bounds: None,
                        clipping_config: None,
                    };
                    let _ = text_renderer.queue_text(&frame, &mut title_cfg);

                    // ── Queue labels with clipping ───────────────────
                    let label_style = TextStyle::new(16.0).with_rgba(0.1, 0.1, 0.1, 1.0);
                    let clipping_config = ClippingStrategyConfig {
                        primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                            ellipsis_text: "…".to_string(),
                            preserve_words: true,
                        },
                        fallback_strategies: vec![],
                        minimum_visible_percentage: 0.0,
                        enable_hover_reveal: true,
                    };

                    for label in &self.labels {
                        let viewport = ViewportBounds::from_container(gup::text::TextBounds::new(
                            label.position.x,
                            label.position.y,
                            label.position.x + label.max_width,
                            label.position.y + 30.0,
                        ));

                        let mut cfg = TextRenderConfig {
                            text: &label.text,
                            position: label.position,
                            style: &label_style,
                            font_atlas,
                            layout_engine,
                            screen_width: screen_w,
                            screen_height: screen_h,
                            viewport_bounds: Some(&viewport),
                            clipping_config: Some(&clipping_config),
                        };

                        if let Ok(result) = text_renderer.queue_text(&frame, &mut cfg)
                            && let Some(original) = &result.original_text
                        {
                            self.registry.register(result.bounds, original);
                        }
                    }

                    // ── Update hover state ───────────────────────────
                    self.hover_state
                        .update(&self.registry, self.mouse_x, self.mouse_y, dt);

                    // ── Queue tooltip text if active ─────────────────
                    if let Some(tooltip) = self.hover_state.active_tooltip() {
                        let tooltip_style = TextStyle::new(self.hover_state.config().font_size)
                            .with_rgba(1.0, 1.0, 1.0, tooltip.opacity);

                        // Measure tooltip text
                        if let Ok(measure) = layout_engine.layout_text(
                            &tooltip.text,
                            Vec2 { x: 0.0, y: 0.0 },
                            &tooltip_style,
                            font_atlas,
                            None,
                        ) {
                            let layout = compute_tooltip_layout(
                                &tooltip,
                                self.hover_state.config(),
                                measure.bounds.width(),
                                measure.bounds.height(),
                                screen_w,
                                screen_h,
                            );

                            let mut tt_cfg = TextRenderConfig {
                                text: &layout.text,
                                position: layout.text_position,
                                style: &tooltip_style,
                                font_atlas,
                                layout_engine,
                                screen_width: screen_w,
                                screen_height: screen_h,
                                viewport_bounds: None,
                                clipping_config: None,
                            };
                            let _ = text_renderer.queue_text(&frame, &mut tt_cfg);
                        }
                    }

                    // ── Render ────────────────────────────────────────
                    let clear_color = wgpu::Color {
                        r: 0.96,
                        g: 0.96,
                        b: 0.97,
                        a: 1.0,
                    };

                    {
                        let mut render_pass = frame.render_pass(Some(clear_color));
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
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
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
    println!("🔍 Interactive Clipping Reveal Demo");
    println!("   Hover over truncated labels to see the full text.");
    println!("   Press Esc to exit.\n");

    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
