// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Debug Text Positioning Example
//!
//! This example specifically tests multi-character text rendering to verify
//! the vertex offset fix works correctly for the garbled text issue.

use gup::{
    GupContext, GupResult, PhysicalSize, SurfaceId,
    shader_function::Vec2,
    text::{FontAtlas, TextLayoutEngine, TextRenderConfig, TextRenderer, TextStyle},
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

struct DebugApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    text_renderer: Option<TextRenderer>,
    font_atlas: Option<FontAtlas>,
    layout_engine: Option<TextLayoutEngine>,
}

impl DebugApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            text_renderer: None,
            font_atlas: None,
            layout_engine: None,
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
            .with_title("Debug Text Positioning")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 400));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
            println!("✅ Surface created");
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);
        Ok(())
    }

    async fn initialize_text_rendering(&mut self) -> GupResult<()> {
        if let Some(context) = &self.context
            && self.text_renderer.is_none()
        {
            let device = &context.device;
            let queue = &context.queue;

            let text_renderer = TextRenderer::new(device)?;
            let font_atlas = FontAtlas::new(device, queue, 48.0)?;
            let layout_engine = TextLayoutEngine::new();

            self.text_renderer = Some(text_renderer);
            self.font_atlas = Some(font_atlas);
            self.layout_engine = Some(layout_engine);

            println!("✅ Text rendering initialized");
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.text_renderer.is_none() {
            match pollster::block_on(self.initialize_text_rendering()) {
                Ok(()) => println!("✅ Text rendering components initialized"),
                Err(e) => {
                    eprintln!("❌ Failed to initialize text rendering: {e}");
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
                        b: 0.2,
                        a: 1.0,
                    };

                    let device = frame.device_arc();
                    let queue = frame.queue_arc();

                    let mut render_pass = frame.render_pass(Some(clear_color));

                    if let (Some(text_renderer), Some(font_atlas), Some(layout_engine)) = (
                        &mut self.text_renderer,
                        &mut self.font_atlas,
                        &mut self.layout_engine,
                    ) {
                        text_renderer.begin_frame();

                        // Test cases that specifically target the vertex offset bug
                        let test_cases = [
                            ("Single: 1", Vec2 { x: 50.0, y: 100.0 }),
                            ("Double: 22", Vec2 { x: 50.0, y: 150.0 }),
                            ("Triple: ABC", Vec2 { x: 50.0, y: 200.0 }),
                            ("Longer: Hello123", Vec2 { x: 50.0, y: 250.0 }),
                            ("Mixed: A1B2C3", Vec2 { x: 50.0, y: 300.0 }),
                        ];

                        for (text, position) in &test_cases {
                            let style = TextStyle::new(48.0).with_rgba(1.0, 1.0, 1.0, 1.0);

                            let config = TextRenderConfig {
                                text,
                                position: *position,
                                style: &style,
                                font_atlas,
                                layout_engine,
                                screen_width: 800.0,
                                screen_height: 400.0,
                                viewport_bounds: None,
                                clipping_config: None,
                            };

                            if let Err(e) =
                                text_renderer.render_text(&mut render_pass, &device, &queue, config)
                            {
                                eprintln!("⚠️ Failed to render text '{text}': {e}");
                            }
                        }
                    }

                    drop(render_pass);
                    frame.finish()?;
                }
                Err(e) => eprintln!("❌ Failed to render frame: {e}"),
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for DebugApp {
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

            println!("✅ Window created! Press ESC to exit, Space for debug info");
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
                println!("👋 Debug session complete");
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
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key_code {
                KeyCode::Escape => event_loop.exit(),
                KeyCode::Space => {
                    println!("🔍 Debug Info:");
                    println!("  - Testing multi-character text rendering");
                    println!("  - Each line should display correctly without garbled text");
                    println!("  - Fix applied: vertex offset in draw_indexed calls");
                }
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

    println!("🧪 Debug Text Positioning Test");
    println!("==============================");
    println!();
    println!("This example tests the vertex offset fix for multi-character text rendering.");
    println!("Before the fix: multi-character strings appeared as garbled patterns");
    println!("After the fix: all text should render clearly");
    println!();
    println!("Controls:");
    println!("• ESC - Exit");
    println!("• Space - Show debug info");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = DebugApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
