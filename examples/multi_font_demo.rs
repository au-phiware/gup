// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-Font Text Rendering Demo
//!
//! Demonstrates rendering text with multiple fonts in a single frame
//! using `FontAtlasManager` and `TextRenderer::queue_text_with_fonts`.
//!
//! The demo shows:
//! - Default embedded font (Squada One)
//! - System fonts resolved by family name (e.g., "DejaVu Sans", "DejaVu Serif")
//! - Automatic fallback when a font is unavailable
//! - Multiple fonts in a single render pass

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    shader_function::Vec2,
    text::{FontAtlasManager, FontDatabase, TextLayoutEngine, TextRenderer, TextStyle},
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Text demo item with position, content, style, and description.
struct TextDemo {
    position: Vec2,
    text: String,
    style: TextStyle,
    description: String,
}

/// Application state.
struct MultiFontApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,

    // Text rendering components
    text_renderer: Option<TextRenderer>,
    font_manager: Option<FontAtlasManager>,
    layout_engine: Option<TextLayoutEngine>,

    // Demo data
    demo_texts: Vec<TextDemo>,
}

impl MultiFontApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            text_renderer: None,
            font_manager: None,
            layout_engine: None,
            demo_texts: Self::generate_demo_texts(),
        }
    }

    fn generate_demo_texts() -> Vec<TextDemo> {
        vec![
            // Title using default font (Squada One)
            TextDemo {
                position: Vec2 { x: 40.0, y: 40.0 },
                text: "Multi-Font Demo".to_string(),
                style: TextStyle::new(64.0).with_rgba(0.1, 0.1, 0.1, 1.0),
                description: "Default (Squada One)".to_string(),
            },
            // Sans-serif system font
            TextDemo {
                position: Vec2 { x: 40.0, y: 140.0 },
                text: "The quick brown fox jumps over the lazy dog".to_string(),
                style: TextStyle::new(36.0)
                    .with_font_family("DejaVu Sans")
                    .with_rgba(0.2, 0.2, 0.8, 1.0),
                description: "DejaVu Sans".to_string(),
            },
            // Serif system font
            TextDemo {
                position: Vec2 { x: 40.0, y: 220.0 },
                text: "The quick brown fox jumps over the lazy dog".to_string(),
                style: TextStyle::new(36.0)
                    .with_font_family("DejaVu Serif")
                    .with_rgba(0.8, 0.2, 0.2, 1.0),
                description: "DejaVu Serif".to_string(),
            },
            // Monospace system font
            TextDemo {
                position: Vec2 { x: 40.0, y: 300.0 },
                text: "fn main() { println!(\"Hello!\"); }".to_string(),
                style: TextStyle::new(32.0)
                    .with_font_family("DejaVu Sans Mono")
                    .with_rgba(0.0, 0.6, 0.3, 1.0),
                description: "DejaVu Sans Mono".to_string(),
            },
            // Unknown font (should fall back to default)
            TextDemo {
                position: Vec2 { x: 40.0, y: 380.0 },
                text: "This font does not exist (fallback)".to_string(),
                style: TextStyle::new(36.0)
                    .with_font_family("NonExistentFont12345")
                    .with_rgba(0.6, 0.4, 0.0, 1.0),
                description: "NonExistentFont12345 (fallback)".to_string(),
            },
            // Default font again (no font_family)
            TextDemo {
                position: Vec2 { x: 40.0, y: 460.0 },
                text: "Default font, no family specified".to_string(),
                style: TextStyle::new(36.0).with_rgba(0.4, 0.4, 0.4, 1.0),
                description: "No font_family (default)".to_string(),
            },
            // Subtitle showing atlas count
            TextDemo {
                position: Vec2 { x: 40.0, y: 560.0 },
                text: "Press ESC to exit".to_string(),
                style: TextStyle::new(24.0).with_rgba(0.5, 0.5, 0.5, 1.0),
                description: "Instructions".to_string(),
            },
        ]
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            let context = GupContext::headless().await?;
            self.context = Some(context);
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Multi-Font Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 700));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to unwrap context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);
        Ok(())
    }

    fn initialize_text_rendering(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = &self.context
            && self.text_renderer.is_none()
        {
            let device = &context.device;

            self.text_renderer = Some(TextRenderer::new(device)?);
            self.font_manager = Some(FontAtlasManager::new(FontDatabase::new(), 16.0));
            self.layout_engine = Some(TextLayoutEngine::new());

            println!("✅ Text rendering initialized");
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.text_renderer.is_none() {
            self.initialize_text_rendering()?;
        }

        let surface_id = match self.surface_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let actual_surface_size = self.context.as_ref().and_then(|ctx| {
            ctx.surface_size(surface_id)
                .map(|s| (s.width as f32, s.height as f32))
        });

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to unwrap context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    let clear_color = wgpu::Color {
                        r: 0.96,
                        g: 0.96,
                        b: 0.98,
                        a: 1.0,
                    };

                    let device = frame.device_arc();
                    let queue = frame.queue_arc();
                    let (screen_width, screen_height) =
                        actual_surface_size.unwrap_or((1200.0, 700.0));

                    // Queue all text with multi-font support
                    if let (Some(text_renderer), Some(font_manager), Some(layout_engine)) = (
                        &mut self.text_renderer,
                        &mut self.font_manager,
                        &mut self.layout_engine,
                    ) {
                        text_renderer.begin_frame();

                        for demo in &self.demo_texts {
                            if let Err(e) = text_renderer.queue_text_with_fonts(
                                &frame,
                                &demo.text,
                                demo.position,
                                &demo.style,
                                font_manager,
                                layout_engine,
                                None,
                                None,
                            ) {
                                eprintln!(
                                    "⚠️ Failed to queue '{}' ({}): {e}",
                                    demo.text, demo.description
                                );
                            }
                        }

                        // Print atlas info once
                        static PRINTED: std::sync::atomic::AtomicBool =
                            std::sync::atomic::AtomicBool::new(false);
                        if !PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            println!("📊 Font atlases loaded: {}", font_manager.atlas_count());
                            for (key, atlas) in font_manager.iter() {
                                println!(
                                    "  • {}: {} glyphs, fallback={}",
                                    key,
                                    atlas.glyph_count(),
                                    atlas.is_fallback_font(),
                                );
                            }
                        }
                    }

                    // Render pass
                    let mut render_pass = frame.render_pass(Some(clear_color));

                    if let (Some(text_renderer), Some(font_manager)) =
                        (&mut self.text_renderer, &self.font_manager)
                        && let Err(e) = text_renderer.render_queued_text_multi(
                            &mut render_pass,
                            &device,
                            &queue,
                            font_manager,
                            screen_width,
                            screen_height,
                        )
                    {
                        eprintln!("⚠️ Failed to render multi-font text: {e}");
                    }

                    drop(render_pass);
                    frame.finish()?;
                }
                Err(e) => {
                    eprintln!("❌ Frame error: {e}");
                }
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for MultiFontApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("❌ Context creation failed: {e}");
                event_loop.exit();
                return;
            }

            if let Err(e) = self.create_window(event_loop) {
                eprintln!("❌ Window creation failed: {e}");
                event_loop.exit();
                return;
            }

            println!("✅ Multi-font demo ready — press ESC to exit");
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
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id
                    && let Some(ctx) = self.context.take()
                {
                    let mut context_mut = Arc::try_unwrap(ctx).unwrap_or_else(|arc| {
                        panic!("Failed to unwrap context: {} refs", Arc::strong_count(&arc))
                    });
                    let _ = context_mut
                        .resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(context_mut));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("❌ Render error: {e}");
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

    println!("🔤 Gup Multi-Font Text Rendering Demo");
    println!("======================================");
    println!();
    println!("Demonstrates rendering multiple fonts in one frame:");
    println!("  • Default embedded font (Squada One)");
    println!("  • System fonts (DejaVu Sans, Serif, Mono)");
    println!("  • Automatic fallback for missing fonts");
    println!("  • Per-atlas batched draw calls");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiFontApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
