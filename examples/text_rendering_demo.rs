// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Comprehensive Text Rendering Demo
//!
//! This example demonstrates the full capabilities of the GPU text rendering system:
//! - Various text styles (size, color, weight)
//! - Different text anchors (positioning)
//! - Text layout and spacing
//! - Performance with multiple text elements
//! - Integration with data visualization

use gup::{
    GupContext, GupResult, PhysicalSize, SurfaceId,
    shader_function::Vec2,
    text::{FontAtlas, TextAnchor, TextLayoutEngine, TextRenderConfig, TextRenderer, TextStyle},
};
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Text demo item with position, content, and styling
#[derive(Debug, Clone)]
struct TextDemo {
    position: Vec2,
    text: String,
    style: TextStyle,
    #[allow(dead_code)]
    description: String,
}

/// Application state for the text rendering demo
struct TextRenderingApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,

    // Text rendering components
    text_renderer: Option<TextRenderer>,
    font_atlas: Option<FontAtlas>,
    layout_engine: Option<TextLayoutEngine>,

    // Demo data
    demo_texts: Vec<TextDemo>,
    #[allow(dead_code)]
    current_demo: usize,
}

impl TextRenderingApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            text_renderer: None,
            font_atlas: None,
            layout_engine: None,
            demo_texts: Self::generate_demo_texts(),
            current_demo: 0,
        }
    }

    fn generate_demo_texts() -> Vec<TextDemo> {
        vec![
            TextDemo {
                position: Vec2 { x: 100.0, y: 50.0 },
                text: "Text Rendering Demo".to_string(),
                style: TextStyle::title().with_rgba(0.1, 0.2, 0.8, 1.0),
                description: "Title style - Large, bold".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 100.0, y: 100.0 },
                text: "Various Text Styles".to_string(),
                style: TextStyle::heading().with_rgba(0.2, 0.2, 0.2, 1.0),
                description: "Heading style".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 100.0, y: 140.0 },
                text: "Normal body text with default styling".to_string(),
                style: TextStyle::body(),
                description: "Body text".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 100.0, y: 170.0 },
                text: "Small caption text in gray".to_string(),
                style: TextStyle::caption(),
                description: "Caption style".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 100.0, y: 220.0 },
                text: "Red Error Text".to_string(),
                style: TextStyle::error(),
                description: "Error message style".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 100.0, y: 250.0 },
                text: "Green Success Text".to_string(),
                style: TextStyle::success(),
                description: "Success message style".to_string(),
            },
            // Text anchor demonstrations
            TextDemo {
                position: Vec2 { x: 400.0, y: 320.0 },
                text: "TopLeft".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::TopLeft)
                    .with_rgba(0.8, 0.2, 0.2, 1.0),
                description: "TopLeft anchor".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 500.0, y: 320.0 },
                text: "TopCenter".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::TopCenter)
                    .with_rgba(0.2, 0.8, 0.2, 1.0),
                description: "TopCenter anchor".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 600.0, y: 320.0 },
                text: "TopRight".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::TopRight)
                    .with_rgba(0.2, 0.2, 0.8, 1.0),
                description: "TopRight anchor".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 400.0, y: 370.0 },
                text: "CenterLeft".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::CenterLeft)
                    .with_rgba(0.8, 0.8, 0.2, 1.0),
                description: "CenterLeft anchor".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 500.0, y: 370.0 },
                text: "Center".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::Center)
                    .with_rgba(0.8, 0.2, 0.8, 1.0),
                description: "Center anchor".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 600.0, y: 370.0 },
                text: "CenterRight".to_string(),
                style: TextStyle::body()
                    .with_anchor(TextAnchor::CenterRight)
                    .with_rgba(0.2, 0.8, 0.8, 1.0),
                description: "CenterRight anchor".to_string(),
            },
            // Font sizes
            TextDemo {
                position: Vec2 { x: 100.0, y: 450.0 },
                text: "Tiny (10px)".to_string(),
                style: TextStyle::new(10.0),
                description: "10 pixel font".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 200.0, y: 450.0 },
                text: "Small (12px)".to_string(),
                style: TextStyle::new(12.0),
                description: "12 pixel font".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 300.0, y: 450.0 },
                text: "Medium (16px)".to_string(),
                style: TextStyle::new(16.0),
                description: "16 pixel font".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 450.0, y: 450.0 },
                text: "Large (20px)".to_string(),
                style: TextStyle::new(20.0),
                description: "20 pixel font".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 580.0, y: 450.0 },
                text: "XL (24px)".to_string(),
                style: TextStyle::new(24.0),
                description: "24 pixel font".to_string(),
            },
            // Font weights
            TextDemo {
                position: Vec2 { x: 100.0, y: 520.0 },
                text: "Thin Text".to_string(),
                style: TextStyle::body().thin(),
                description: "Thin weight".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 200.0, y: 520.0 },
                text: "Normal Text".to_string(),
                style: TextStyle::body(),
                description: "Normal weight".to_string(),
            },
            TextDemo {
                position: Vec2 { x: 320.0, y: 520.0 },
                text: "Bold Text".to_string(),
                style: TextStyle::body().bold(),
                description: "Bold weight".to_string(),
            },
            // Rotated text
            TextDemo {
                position: Vec2 { x: 500.0, y: 520.0 },
                text: "Rotated 45°".to_string(),
                style: TextStyle::body()
                    .with_rotation_degrees(45.0)
                    .with_rgba(0.6, 0.2, 0.8, 1.0),
                description: "45 degree rotation".to_string(),
            },
            // Performance test
            TextDemo {
                position: Vec2 { x: 100.0, y: 600.0 },
                text: "Performance Test: Many Text Elements".to_string(),
                style: TextStyle::heading().with_rgba(0.1, 0.1, 0.1, 1.0),
                description: "Performance header".to_string(),
            },
        ]
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
        let title = "Gup Text Rendering Demo";
        let window_attributes = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        println!("🖼️ Creating window...");

        // Add surface to context
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

    async fn initialize_text_rendering(&mut self) -> GupResult<()> {
        if let Some(context) = &self.context {
            if self.text_renderer.is_none() {
                let device = &context.device; // Access device from context
                let queue = &context.queue; // Access queue from context

                let text_renderer = TextRenderer::new(device)?;
                let font_atlas = FontAtlas::new(device, queue, "DejaVu Sans", 16.0)?;
                let layout_engine = TextLayoutEngine::new();

                self.text_renderer = Some(text_renderer);
                self.font_atlas = Some(font_atlas);
                self.layout_engine = Some(layout_engine);

                println!("✅ Text rendering initialized successfully");
            }
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize text rendering if not done yet
        if self.text_renderer.is_none() {
            match pollster::block_on(self.initialize_text_rendering()) {
                Ok(()) => {
                    println!("✅ Text rendering components initialized");
                }
                Err(e) => {
                    eprintln!("❌ Failed to initialize text rendering: {e}");
                    return Err(e.into());
                }
            }
        }

        // Render visual frame
        if let Some(surface_id) = self.surface_id {
            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        // Clear background with light gray
                        let clear_color = wgpu::Color {
                            r: 0.95,
                            g: 0.95,
                            b: 0.95,
                            a: 1.0,
                        };

                        // Clear the background first
                        {
                            let _render_pass = frame.render_pass(Some(clear_color));
                        }

                        // Render all demo texts
                        if let (Some(text_renderer), Some(font_atlas), Some(layout_engine)) = (
                            &mut self.text_renderer,
                            &mut self.font_atlas,
                            &mut self.layout_engine,
                        ) {
                            let mut rendered_count = 0;

                            for text_demo in &self.demo_texts {
                                let config = TextRenderConfig {
                                    text: &text_demo.text,
                                    position: text_demo.position,
                                    style: &text_demo.style,
                                    font_atlas,
                                    layout_engine,
                                    screen_width: 1200.0,
                                    screen_height: 800.0,
                                };

                                if let Err(e) = text_renderer.render_text(&mut frame, config) {
                                    eprintln!(
                                        "⚠️ Failed to render text '{}': {}",
                                        text_demo.text, e
                                    );
                                } else {
                                    rendered_count += 1;
                                }
                            }

                            // Generate additional performance test texts
                            for i in 0..20 {
                                let x = 120.0 + (i % 10) as f32 * 100.0;
                                let y = 630.0 + (i / 10) as f32 * 30.0;
                                let text = format!("Item {}", i + 1);

                                let style = TextStyle::new(12.0).with_rgba(0.3, 0.3, 0.3, 1.0);

                                let config = TextRenderConfig {
                                    text: &text,
                                    position: Vec2 { x, y },
                                    style: &style,
                                    font_atlas,
                                    layout_engine,
                                    screen_width: 1200.0,
                                    screen_height: 800.0,
                                };

                                if text_renderer.render_text(&mut frame, config).is_ok() {
                                    rendered_count += 1;
                                }
                            }

                            println!("✅ Rendered {rendered_count} text elements successfully");
                        }

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

    fn print_demo_info(&self) {
        println!("🎨 Text Rendering Demo Features:");
        println!(
            "  • {} different text styles and configurations",
            self.demo_texts.len()
        );
        println!("  • Text anchoring (TopLeft, Center, BottomRight, etc.)");
        println!("  • Font sizes from 10px to 24px");
        println!("  • Font weights (thin, normal, bold)");
        println!("  • Text rotation (45° example)");
        println!("  • Color variations and transparency");
        println!("  • Performance test with 20+ additional text elements");
        println!("  • Real-time GPU text rendering with SDF fonts");
    }
}

impl ApplicationHandler for TextRenderingApp {
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
            println!("🎨 Demonstrating comprehensive text rendering...");
            self.print_demo_info();
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
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id {
                    if let Some(ctx) = self.context.take() {
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
                println!("📐 Window resized to {}x{}", size.width, size.height);
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
        // Request redraw to continuously update the display
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🎨 Gup Text Rendering Demo");
    println!("========================");
    println!();
    println!("This demo showcases the full GPU text rendering capabilities:");
    println!("• Multiple text styles (title, heading, body, caption, error, success)");
    println!("• Text anchoring demonstrations (9 different anchor points)");
    println!("• Font size variations (10px to 24px)");
    println!("• Font weight options (thin, normal, bold)");
    println!("• Text rotation and transformations");
    println!("• Color and transparency effects");
    println!("• Performance testing with many text elements");
    println!("• Real-time SDF (Signed Distance Field) text rendering");
    println!();
    println!("Controls:");
    println!("• ESC - Exit the demo");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = TextRenderingApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_text_generation() {
        let demo_texts = TextRenderingApp::generate_demo_texts();

        // Should have a good variety of demo texts
        assert!(demo_texts.len() >= 15);

        // Check that we have examples of different styles
        let has_title = demo_texts.iter().any(|t| t.style.font_size >= 20.0);
        let has_small = demo_texts.iter().any(|t| t.style.font_size <= 12.0);
        let has_colored = demo_texts
            .iter()
            .any(|t| t.style.color.x != 0.0 || t.style.color.y != 0.0 || t.style.color.z != 0.0);

        assert!(has_title, "Should have title-sized text");
        assert!(has_small, "Should have small text");
        assert!(has_colored, "Should have colored text");
    }

    #[test]
    fn test_app_initialization() {
        let app = TextRenderingApp::new();

        assert!(app.context.is_none());
        assert!(app.text_renderer.is_none());
        assert!(!app.demo_texts.is_empty());
        assert_eq!(app.current_demo, 0);
    }

    #[test]
    fn test_text_positioning() {
        let demo_texts = TextRenderingApp::generate_demo_texts();

        // All texts should have reasonable positions
        for text in &demo_texts {
            assert!(text.position.x >= 0.0 && text.position.x <= 1200.0);
            assert!(text.position.y >= 0.0 && text.position.y <= 800.0);
        }
    }

    #[test]
    fn test_text_styles_variety() {
        let demo_texts = TextRenderingApp::generate_demo_texts();

        // Should have variety in font sizes
        let mut font_sizes: Vec<f32> = demo_texts.iter().map(|t| t.style.font_size).collect();
        font_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        font_sizes.dedup();

        assert!(
            font_sizes.len() >= 5,
            "Should have at least 5 different font sizes"
        );

        // Should have different anchors
        let anchors: Vec<_> = demo_texts.iter().map(|t| t.style.anchor).collect();
        let unique_anchors: std::collections::HashSet<_> = anchors.into_iter().collect();

        assert!(
            unique_anchors.len() >= 3,
            "Should have at least 3 different text anchors"
        );
    }
}
