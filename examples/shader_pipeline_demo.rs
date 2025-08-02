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

//! Shader Pipeline Demo
//!
//! This example demonstrates the ShaderPipeline system, showing how to:
//! - Create a shader pipeline with multiple functions
//! - Map attributes to functions
//! - Generate optimized WGSL shaders
//! - Create GPU resources and render pipelines
//! - Measure performance characteristics

use gup::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Gup Shader Pipeline Demo");
    println!("==========================");

    // Create GPU context
    println!("\n📱 Creating GPU context...");
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Create shader pipeline
    println!("\n🔧 Building shader pipeline...");
    let mut pipeline = ComposableShaderPipeline::new();

    // Add shader functions
    let data_scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_mapping = ColorMap::new(
        Vec4::new(0.1, 0.2, 0.8, 1.0), // Blue
        Vec4::new(0.9, 0.6, 0.1, 1.0), // Orange
    );
    let size_scale = LinearScale::new(0.0, 100.0, 0.1, 2.0);
    let position_transform = PositionTransform::new(
        Vec2::new(0.8, 0.6), // Scale
        Vec2::new(0.0, 0.0), // Offset
    );

    pipeline.add_function(data_scale);
    pipeline.add_function(color_mapping);
    pipeline.add_function(size_scale);
    pipeline.add_function(position_transform);

    println!("   ✅ Added {} shader functions", pipeline.function_count());

    // Map attributes to functions
    println!("\n🗺️  Mapping attributes...");
    pipeline.map_attribute("color", "color_map");
    pipeline.map_attribute("size", "linear_scale");
    pipeline.map_attribute("position", "position_transform");

    // Generate shaders
    println!("\n🎨 Generating WGSL shaders...");
    let start = Instant::now();

    let vertex_shader = pipeline.generate_vertex_shader();
    let fragment_shader = pipeline.generate_fragment_shader();

    let generation_time = start.elapsed();

    println!(
        "   ⏱️  Generation time: {:.3}ms",
        generation_time.as_secs_f64() * 1000.0
    );

    if generation_time.as_millis() < 5 {
        println!("   ✅ Performance target met (<5ms)");
    } else {
        println!(
            "   ⚠️  Performance target exceeded ({}ms > 5ms)",
            generation_time.as_millis()
        );
    }

    // Display generated shaders
    println!("\n📄 Generated Vertex Shader:");
    println!("{}", format_shader_for_display(&vertex_shader));

    println!("\n📄 Generated Fragment Shader:");
    println!("{}", format_shader_for_display(&fragment_shader));

    // Test shader optimization
    println!("\n⚡ Testing shader optimization...");
    let optimized_vertex = pipeline.generate_optimized_vertex_shader();
    let optimized_fragment = pipeline.generate_optimized_fragment_shader();

    let vertex_size_reduction = vertex_shader.len() as f64 - optimized_vertex.len() as f64;
    let fragment_size_reduction = fragment_shader.len() as f64 - optimized_fragment.len() as f64;

    println!(
        "   📊 Vertex shader: {} bytes → {} bytes ({:.1}% change)",
        vertex_shader.len(),
        optimized_vertex.len(),
        (vertex_size_reduction / vertex_shader.len() as f64) * 100.0
    );

    println!(
        "   📊 Fragment shader: {} bytes → {} bytes ({:.1}% change)",
        fragment_shader.len(),
        optimized_fragment.len(),
        (fragment_size_reduction / fragment_shader.len() as f64) * 100.0
    );

    // Test GPU compilation
    println!("\n🔥 Testing GPU compilation...");
    let start = Instant::now();

    let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("demo_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
    });

    let _fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("demo_fragment"),
        source: wgpu::ShaderSource::Wgsl(fragment_shader.into()),
    });

    let compilation_time = start.elapsed();
    println!(
        "   ⏱️  GPU compilation time: {:.3}ms",
        compilation_time.as_secs_f64() * 1000.0
    );
    println!("   ✅ Shaders compiled successfully");

    // Create uniform buffers
    println!("\n💾 Creating uniform buffers...");
    pipeline.create_uniform_buffers(device)?;
    pipeline.update_uniforms(device, queue)?;

    println!(
        "   ✅ Created {} uniform buffers",
        pipeline.uniform_buffer_count()
    );

    // Create bind group layout
    println!("\n🔗 Creating bind group layout...");
    let _bind_group_layout = pipeline.create_bind_group_layout(device)?;
    println!(
        "   ✅ Bind group layout with {} bindings",
        pipeline.functions_with_uniforms_count()
    );

    // Create render pipeline
    println!("\n🎭 Creating render pipeline...");
    let start = Instant::now();
    let _render_pipeline = pipeline.create_render_pipeline(device)?;
    let pipeline_creation_time = start.elapsed();

    println!(
        "   ⏱️  Pipeline creation time: {:.3}ms",
        pipeline_creation_time.as_secs_f64() * 1000.0
    );
    println!("   ✅ Render pipeline created successfully");

    // Test caching performance
    println!("\n💨 Testing caching performance...");

    // First generation (cold)
    let start = Instant::now();
    let _cold_vertex = pipeline.generate_vertex_shader();
    let cold_time = start.elapsed();

    // Cache shaders
    pipeline.update_cache_public(device)?;

    // Second generation (cached)
    let start = Instant::now();
    let _cached_vertex = pipeline.generate_vertex_shader();
    let cached_time = start.elapsed();

    println!(
        "   📊 Cold generation: {:.3}ms",
        cold_time.as_secs_f64() * 1000.0
    );
    println!(
        "   📊 Cached generation: {:.3}ms",
        cached_time.as_secs_f64() * 1000.0
    );

    if cached_time < cold_time {
        let speedup = cold_time.as_secs_f64() / cached_time.as_secs_f64();
        println!("   ⚡ Cache speedup: {speedup:.1}x faster");
    }

    // Performance summary
    println!("\n📊 Performance Summary:");
    println!(
        "   • Shader generation: {:.3}ms",
        generation_time.as_secs_f64() * 1000.0
    );
    println!(
        "   • GPU compilation: {:.3}ms",
        compilation_time.as_secs_f64() * 1000.0
    );
    println!(
        "   • Pipeline creation: {:.3}ms",
        pipeline_creation_time.as_secs_f64() * 1000.0
    );
    println!(
        "   • Total pipeline time: {:.3}ms",
        (generation_time + compilation_time + pipeline_creation_time).as_secs_f64() * 1000.0
    );

    println!("\n✨ Demo completed successfully!");

    Ok(())
}

/// Format shader source for display with line numbers and syntax highlighting hints.
fn format_shader_for_display(shader: &str) -> String {
    let lines: Vec<&str> = shader.lines().collect();
    let mut formatted = String::new();

    for (i, line) in lines.iter().enumerate() {
        if i >= 20 {
            // Limit display to first 20 lines
            formatted.push_str(&format!("   ... ({} more lines)\n", lines.len() - i));
            break;
        }

        formatted.push_str(&format!("   {:2} | {}\n", i + 1, line));
    }

    formatted
}
