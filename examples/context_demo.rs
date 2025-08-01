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

//! Demonstration of GupContext usage and capabilities.

use gup::{BufferType, GupContext, GupOptions, PhysicalSize, SurfaceId};
use std::sync::Arc;
use wgpu::PowerPreference;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Gup Context Demo ===\n");

    // 1. Basic context creation
    println!("1. Creating basic headless context...");
    let basic_context = GupContext::headless().await?;
    println!("   ✓ Basic context created successfully");
    println!("   - Device: {:?}", basic_context.device.limits());
    println!("   - Surface format: {:?}", basic_context.surface_format());

    // 2. Custom options context
    println!("\n2. Creating context with custom options...");
    let custom_options = GupOptions {
        power_preference: PowerPreference::LowPower,
        ..Default::default()
    };
    let _custom_context = GupContext::with_options(custom_options).await?;
    println!("   ✓ Custom context created with low power preference");

    // 3. Buffer creation and management
    println!("\n3. Testing buffer creation...");
    let context = GupContext::headless().await?;
    let mut ctx = Arc::try_unwrap(context).unwrap();

    let vertex_buffer = ctx.create_buffer::<f32>(BufferType::Vertex, 1000);
    println!(
        "   ✓ Vertex buffer created: capacity={}, type={:?}",
        vertex_buffer.capacity(),
        vertex_buffer.buffer_type()
    );

    let storage_buffer = ctx.create_buffer::<[f32; 4]>(BufferType::Storage, 5000);
    println!(
        "   ✓ Storage buffer created: capacity={}, type={:?}",
        storage_buffer.capacity(),
        storage_buffer.buffer_type()
    );

    // 4. Frame lifecycle demonstration
    println!("\n4. Testing frame lifecycle...");
    let mut frame = ctx.begin_frame()?;
    println!("   ✓ Frame started");

    // Create a render pass
    let render_pass = frame.render_pass(Some(wgpu::Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    }));
    drop(render_pass); // End render pass

    frame.finish()?;
    println!("   ✓ Frame finished and presented");

    // 5. Performance statistics
    println!("\n5. Performance statistics after rendering:");
    let stats = ctx.frame_stats();
    println!("   - Frames rendered: {}", stats.frames_rendered);
    println!("   - Average frame time: {:.2}ms", stats.avg_frame_time);
    println!("   - Current FPS: {:.1}", stats.fps());
    println!("   - GPU memory usage: {} bytes", stats.gpu_memory_usage);

    // 6. Buffer pool statistics
    println!("\n6. Buffer pool statistics:");
    let pool_stats = ctx.buffer_pool().get_stats();
    println!("   - Total allocated: {}", pool_stats.total_allocated);
    println!("   - Active buffers: {}", pool_stats.active_buffers);
    println!("   - Pool efficiency: {:.1}%", pool_stats.pool_efficiency());

    // 7. Multiple frames for performance testing
    println!("\n7. Rendering multiple frames for performance measurement...");
    for i in 0..10 {
        let frame = ctx.begin_frame()?;
        frame.finish()?;
        if (i + 1) % 5 == 0 {
            println!("   Rendered {} frames", i + 1);
        }
    }

    let final_stats = ctx.frame_stats();
    println!("   ✓ Final statistics:");
    println!("     - Total frames: {}", final_stats.frames_rendered);
    println!(
        "     - Average frame time: {:.2}ms",
        final_stats.avg_frame_time
    );
    println!("     - Final FPS: {:.1}", final_stats.fps());

    // 8. Demonstrate new surface management APIs
    println!("\n8. New surface management capabilities...");

    // Show SurfaceId creation
    let surface_id = SurfaceId::new();
    println!("   Sample Surface ID: {surface_id}");
    println!("   Raw ID value: {}", surface_id.raw());

    // Show PhysicalSize usage
    let size = PhysicalSize::new(1920u32, 1080u32);
    println!("   Sample size: {}x{}", size.width, size.height);

    // Show multi-surface support info
    println!("   Active surfaces: {}", ctx.surface_ids().len());
    println!("   Primary surface: {:?}", ctx.primary_surface_id());

    // Show surface format info
    println!("   Default surface format: {:?}", ctx.surface_format());

    // Demonstrate error handling for non-existent surfaces
    let test_id = SurfaceId::new();
    println!("   Testing operations on non-existent surface...");
    println!("     - Format query: {:?}", ctx.surface_format_for(test_id));
    println!("     - Size query: {:?}", ctx.surface_size(test_id));
    println!("     - Fullscreen check: {}", ctx.is_fullscreen(test_id));
    println!(
        "     - Scale factor: {:?}",
        ctx.surface_scale_factor(test_id)
    );

    println!("\n=== Demo completed successfully! ===");
    println!("\nFor multi-window examples, run: cargo run --example multi_window_demo");
    Ok(())
}
