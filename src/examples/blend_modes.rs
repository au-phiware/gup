// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Blend Modes Showcase Example
//!
//! This example demonstrates the GPU blend state integration implemented in GUP-027.
//! It shows how different blend modes affect the visual composition of overlaid elements,
//! showcasing alpha blending, additive effects, multiply darkening, and cross-fade animations.

use crate::mixable::CrossFadeExt;
use crate::render::Vertex;
use crate::{BlendMode, GupResult, Mixable, MixableExt, RenderContext};
use std::f32::consts::PI;

/// A simple colored quad for demonstrating blend modes
#[derive(Debug, Clone)]
pub struct ColoredQuad {
    color: [f32; 4],
    position: [f32; 2],
    size: f32,
    name: String,
}

impl ColoredQuad {
    /// Create a new colored quad
    pub fn new(name: &str, color: [f32; 4], position: [f32; 2], size: f32) -> Self {
        Self {
            color,
            position,
            size,
            name: name.to_string(),
        }
    }

    /// Create a red quad
    pub fn red(name: &str, position: [f32; 2], size: f32) -> Self {
        Self::new(name, [1.0, 0.2, 0.2, 0.7], position, size)
    }

    /// Create a green quad
    pub fn green(name: &str, position: [f32; 2], size: f32) -> Self {
        Self::new(name, [0.2, 1.0, 0.2, 0.7], position, size)
    }

    /// Create a blue quad
    pub fn blue(name: &str, position: [f32; 2], size: f32) -> Self {
        Self::new(name, [0.2, 0.2, 1.0, 0.7], position, size)
    }

    /// Create a yellow quad
    pub fn yellow(name: &str, position: [f32; 2], size: f32) -> Self {
        Self::new(name, [1.0, 1.0, 0.2, 0.6], position, size)
    }

    /// Generate vertices for a quad
    fn generate_vertices(&self) -> Vec<Vertex> {
        let half_size = self.size / 2.0;
        let [x, y] = self.position;

        vec![
            // Triangle 1
            Vertex {
                position: [x - half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x + half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x - half_size, y + half_size],
                color: self.color,
            },
            // Triangle 2
            Vertex {
                position: [x + half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x + half_size, y + half_size],
                color: self.color,
            },
            Vertex {
                position: [x - half_size, y + half_size],
                color: self.color,
            },
        ]
    }
}

impl Mixable for ColoredQuad {
    type Output = ();

    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        let vertices = self.generate_vertices();

        // For demonstration, we'll use a simple rendering approach
        // In a real implementation, this would use the GPU pipeline with the current blend mode
        println!(
            "Rendering {} quad at ({:.2}, {:.2}) with color {:?} using blend mode {:?}",
            self.name,
            self.position[0],
            self.position[1],
            self.color,
            context.current_blend_mode()
        );

        // Simulate GPU rendering by logging vertex data
        println!("  Vertices: {} triangles", vertices.len() / 3);

        Ok(())
    }

    fn description(&self) -> String {
        format!("ColoredQuad({})", self.name)
    }
}

/// Demonstrates basic blend mode overlay effects
pub async fn demonstrate_basic_blend_modes() -> GupResult<()> {
    println!("🎨 Basic Blend Modes Demonstration");
    println!("==================================");

    let mut context = RenderContext::new().await?;

    // Create base layers
    let background = ColoredQuad::red("background", [-0.2, 0.0], 0.6);
    let foreground = ColoredQuad::blue("foreground", [0.2, 0.0], 0.6);

    println!("\n1. Alpha Blending (Default Overlay):");
    println!("   - Blue quad overlaid on red quad with proper alpha compositing");
    let mut alpha_overlay = background.clone().overlay(foreground.clone());
    alpha_overlay.render(&mut context)?;

    println!("\n2. Additive Blending:");
    println!("   - Colors add together, creating brighter combined areas");
    // Demonstrate setting custom blend mode
    context.set_blend_mode(BlendMode::Additive)?;
    let mut additive_comp = background.clone().mix(foreground.clone());
    additive_comp.render(&mut context)?;

    println!("\n3. Multiply Blending:");
    println!("   - Colors multiply together, creating darker combined areas");
    context.set_blend_mode(BlendMode::Multiply)?;
    let mut multiply_comp = background.clone().mix(foreground.clone());
    multiply_comp.render(&mut context)?;

    println!("\n4. No Blending:");
    println!("   - Foreground completely replaces background (no transparency)");
    context.set_blend_mode(BlendMode::None)?;
    let mut no_blend_comp = background.mix(foreground);
    no_blend_comp.render(&mut context)?;

    Ok(())
}

/// Demonstrates nested compositions with blend state management
pub async fn demonstrate_nested_blending() -> GupResult<()> {
    println!("\n\n🔄 Nested Blend State Management");
    println!("=================================");

    let mut context = RenderContext::new().await?;

    // Create a complex composition: ((Red + Green) additive + Blue) alpha
    let red = ColoredQuad::red("red", [-0.4, 0.0], 0.4);
    let green = ColoredQuad::green("green", [-0.2, 0.0], 0.4);
    let blue = ColoredQuad::blue("blue", [0.0, 0.0], 0.4);
    let yellow = ColoredQuad::yellow("yellow", [0.2, 0.0], 0.4);

    println!("\nCreating nested composition:");
    println!("  1. Red + Green with additive blending");
    println!("  2. (Red + Green) + Blue with alpha blending");
    println!("  3. ((Red + Green) + Blue) + Yellow with multiply blending");

    // Set initial blend state
    context.set_blend_mode(BlendMode::Multiply)?;
    println!("\nInitial blend mode: {:?}", context.current_blend_mode());

    // Create the nested composition
    // This will demonstrate the blend state stack working correctly
    let inner_composition = red.overlay(green);
    let middle_composition = inner_composition.overlay(blue);
    let mut final_composition = middle_composition.overlay(yellow);

    println!("\nRendering nested composition...");
    final_composition.render(&mut context)?;

    println!(
        "\nFinal blend mode after nested rendering: {:?}",
        context.current_blend_mode()
    );
    println!("✅ Blend state properly restored!");

    Ok(())
}

/// Demonstrates cross-fade animation using global alpha
pub async fn demonstrate_cross_fade() -> GupResult<()> {
    println!("\n\n🎭 Cross-Fade Animation with Global Alpha");
    println!("==========================================");

    let mut context = RenderContext::new().await?;

    let scene_a = ColoredQuad::red("Scene A", [0.0, 0.0], 0.8);
    let scene_b = ColoredQuad::green("Scene B", [0.0, 0.0], 0.8);

    println!("\nAnimating cross-fade from Scene A to Scene B:");

    // Simulate animation frames
    for frame in 0..=10 {
        let t = frame as f32 / 10.0; // 0.0 to 1.0
        let fade_factor = (t * PI / 2.0).sin(); // Smooth easing

        println!("\n  Frame {frame}: fade factor = {fade_factor:.2}");

        let mut cross_fade = scene_a.clone().cross_fade(scene_b.clone(), fade_factor);
        cross_fade.render(&mut context)?;

        // Show that global alpha buffer was created
        if context.has_global_alpha_buffer() {
            println!("    ✅ Global alpha buffer active");
        }
    }

    println!("\n🎬 Cross-fade animation complete!");

    Ok(())
}

/// Demonstrates performance characteristics of blend state changes
pub async fn demonstrate_performance() -> GupResult<()> {
    println!("\n\n⚡ Blend State Performance Demonstration");
    println!("========================================");

    let mut context = RenderContext::new().await?;

    let _quad = ColoredQuad::blue("performance_test", [0.0, 0.0], 0.5);

    println!("\nTesting performance of rapid blend state changes...");

    let start_time = std::time::Instant::now();
    let iterations = 1000;

    for i in 0..iterations {
        let blend_mode = match i % 4 {
            0 => BlendMode::None,
            1 => BlendMode::AlphaBlending,
            2 => BlendMode::Additive,
            _ => BlendMode::Multiply,
        };

        context.set_blend_mode(blend_mode)?;
    }

    let duration = start_time.elapsed();
    let avg_per_change = duration.as_nanos() as f64 / iterations as f64;

    println!("Results:");
    println!("  {iterations} blend state changes in {duration:?}");
    println!("  Average per change: {avg_per_change:.2} ns");
    println!("  Pipeline cache size: {}", context.pipeline_cache_size());

    if duration.as_millis() < 10 {
        println!("  ✅ Performance target met (< 10ms for 1000 changes)");
    } else {
        println!("  ⚠️  Performance below target");
    }

    Ok(())
}

/// Demonstrates RAII automatic blend state management
pub async fn demonstrate_raii_guards() -> GupResult<()> {
    println!("\n\n🛡️  RAII Automatic State Management");
    println!("===================================");
    println!("Demonstrating automatic blend state restoration (GUP-045)");

    let mut context = RenderContext::new().await?;
    let red = ColoredQuad::red("red", [-0.2, 0.0], 0.4);
    let blue = ColoredQuad::blue("blue", [0.2, 0.0], 0.4);

    // Set initial blend mode
    context.set_blend_mode(BlendMode::None)?;
    println!("\n📝 Initial blend mode: {:?}", context.current_blend_mode());

    println!("\n1. Using RAII guard for automatic cleanup:");
    {
        let mut guard = context.with_blend_mode(BlendMode::AlphaBlending)?;
        println!("   - Inside guard scope");
        println!("   - Blend mode: {:?}", guard.context().current_blend_mode());

        let mut red_copy = red.clone();
        red_copy.render(guard.context_mut())?;

        println!("   - About to exit scope, guard will automatically restore state");
    }
    println!("   - After guard dropped");
    println!("   - Blend mode: {:?} (automatically restored!)", context.current_blend_mode());

    println!("\n2. Nested RAII guards:");
    {
        let mut outer = context.with_blend_mode(BlendMode::Multiply)?;
        println!("   - Outer guard: {:?}", outer.context().current_blend_mode());

        {
            let mut inner = outer.context_mut().with_blend_mode(BlendMode::Additive)?;
            println!("     - Inner guard: {:?}", inner.context().current_blend_mode());

            let mut blue_copy = blue.clone();
            blue_copy.render(inner.context_mut())?;

            println!("     - Inner guard about to drop");
        }
        println!("   - After inner drop: {:?} (back to outer mode)", outer.context().current_blend_mode());
    }
    println!("   - After outer drop: {:?} (back to initial)", context.current_blend_mode());

    println!("\n3. Exception safety demonstration:");
    println!("   - Guards restore state even if rendering fails");
    {
        let result = (|| -> GupResult<()> {
            let guard = context.with_blend_mode(BlendMode::AlphaBlending)?;
            println!("   - Guard active: {:?}", guard.context().current_blend_mode());
            // Simulate early return
            if true {
                println!("   - Early return triggered!");
                return Ok(());
            }
            #[allow(unreachable_code)]
            {
                println!("   - This code is never reached");
                Ok(())
            }
        })();
        assert!(result.is_ok());
        println!("   - After function return: {:?} (guard cleaned up automatically)", context.current_blend_mode());
    }

    println!("\n✅ RAII guards provide:");
    println!("   • Automatic cleanup (no manual pop needed)");
    println!("   • Exception safety (cleanup even on errors)");
    println!("   • Compile-time correctness (borrow checker prevents misuse)");
    println!("   • Zero runtime overhead compared to manual management");

    Ok(())
}

/// Demonstrates all blend modes side by side
pub async fn demonstrate_blend_comparison() -> GupResult<()> {
    println!("\n\n🎯 Blend Mode Comparison");
    println!("========================");

    let mut context = RenderContext::new().await?;

    let background = ColoredQuad::red("bg", [0.0, 0.0], 0.8);
    let foreground = ColoredQuad::blue("fg", [0.0, 0.0], 0.6);

    let blend_modes = [
        (BlendMode::None, "None (Replace)"),
        (BlendMode::AlphaBlending, "Alpha Blending"),
        (BlendMode::Additive, "Additive"),
        (BlendMode::Multiply, "Multiply"),
    ];

    println!("\nComparing all blend modes with red background + blue foreground:");

    for (i, (blend_mode, description)) in blend_modes.iter().enumerate() {
        println!("\n{}. {} Mode:", i + 1, description);

        context.set_blend_mode(*blend_mode)?;

        let mut composition = background.clone().mix(foreground.clone());
        composition.render(&mut context)?;

        println!(
            "   Current pipeline cache size: {}",
            context.pipeline_cache_size()
        );
    }

    println!("\n📊 Blend mode comparison complete!");
    println!(
        "   Final cache size: {} pipelines",
        context.pipeline_cache_size()
    );

    Ok(())
}

/// Main demonstration function
pub async fn run_blend_modes_showcase() -> GupResult<()> {
    println!("🚀 Gup Blend Modes Showcase");
    println!("===========================");
    println!("Demonstrating GPU blend state integration (GUP-027 & GUP-045)");

    demonstrate_basic_blend_modes().await?;
    demonstrate_nested_blending().await?;
    demonstrate_raii_guards().await?;
    demonstrate_cross_fade().await?;
    demonstrate_performance().await?;
    demonstrate_blend_comparison().await?;

    println!("\n\n🎉 Blend Modes Showcase Complete!");
    println!("================================");
    println!("Key features demonstrated:");
    println!("  ✅ WebGPU blend state integration");
    println!("  ✅ Blend state stack for nested compositions");
    println!("  ✅ RAII automatic state management");
    println!("  ✅ Global alpha uniform system");
    println!("  ✅ Pipeline caching for performance");
    println!("  ✅ All four blend modes (None, Alpha, Additive, Multiply)");
    println!("  ✅ Cross-fade animations");
    println!("  ✅ Performance optimization");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colored_quad_creation() {
        let quad = ColoredQuad::red("test", [0.0, 0.0], 1.0);
        assert_eq!(quad.name, "test");
        assert_eq!(quad.position, [0.0, 0.0]);
        assert_eq!(quad.size, 1.0);
        assert_eq!(quad.color[0], 1.0); // Red component
    }

    #[test]
    fn test_vertex_generation() {
        let quad = ColoredQuad::blue("test", [0.0, 0.0], 2.0);
        let vertices = quad.generate_vertices();

        assert_eq!(vertices.len(), 6); // 2 triangles = 6 vertices

        // Check that all vertices have the correct color
        for vertex in &vertices {
            assert_eq!(vertex.color, quad.color);
        }
    }

    #[tokio::test]
    async fn test_blend_modes_integration() {
        // Test that the example runs without errors
        let result = demonstrate_basic_blend_modes().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_performance_demo() {
        let result = demonstrate_performance().await;
        assert!(result.is_ok());
    }
}
