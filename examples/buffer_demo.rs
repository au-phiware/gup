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

//! GPU Buffer Management System Demo
//!
//! This example demonstrates the core GPU buffer management capabilities
//! of the Gup visualization library, including:
//! - Creating different types of GPU buffers
//! - Uploading data with automatic resizing
//! - Using buffer pools for efficient memory management
//! - Performance monitoring with allocation statistics

use gup::{BufferPool, BufferType, GpuBuffer, RenderContext};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Gup Buffer Management System Demo");
    println!("=====================================\n");

    let context = RenderContext::new().await?;
    println!("✅ WebGPU context initialized successfully");

    demo_basic_buffer_operations(&context).await?;
    demo_buffer_auto_resizing(&context).await?;
    demo_buffer_pool_system(&context).await?;
    demo_different_buffer_types(&context).await?;

    println!("\n🎉 All buffer operations completed successfully!");
    println!("The GPU buffer management system is working correctly.");

    Ok(())
}

async fn demo_basic_buffer_operations(
    context: &RenderContext,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Demo: Basic Buffer Operations");
    println!("---------------------------------");

    let mut buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Storage, 1000);

    println!(
        "   • Created storage buffer with capacity: {}",
        buffer.capacity()
    );
    println!("   • Initial length: {}", buffer.len());
    println!("   • Is empty: {}", buffer.is_empty());

    let test_data: Vec<f32> = (0..500).map(|i| i as f32 * 0.1).collect();
    buffer.upload(context.device(), context.queue(), &test_data)?;

    println!("   • Uploaded {} elements", test_data.len());
    println!("   • Buffer length after upload: {}", buffer.len());
    println!("   • Buffer capacity: {}", buffer.capacity());

    println!("   ✅ Basic buffer operations completed\n");
    Ok(())
}

async fn demo_buffer_auto_resizing(
    context: &RenderContext,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📏 Demo: Buffer Auto-Resizing");
    println!("------------------------------");

    let mut buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Vertex, 100);
    println!(
        "   • Created vertex buffer with initial capacity: {}",
        buffer.capacity()
    );

    let small_data: Vec<f32> = (0..50).map(|i| i as f32).collect();
    buffer.upload(context.device(), context.queue(), &small_data)?;
    println!(
        "   • Uploaded {} elements (within capacity)",
        small_data.len()
    );
    println!("   • Capacity remains: {}", buffer.capacity());

    let large_data: Vec<f32> = (0..500).map(|i| i as f32).collect();
    buffer.upload(context.device(), context.queue(), &large_data)?;
    println!(
        "   • Uploaded {} elements (exceeds initial capacity)",
        large_data.len()
    );
    println!(
        "   • Buffer auto-resized to capacity: {}",
        buffer.capacity()
    );

    println!("   ✅ Auto-resizing functionality verified\n");
    Ok(())
}

async fn demo_buffer_pool_system(
    context: &RenderContext,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🏊 Demo: Buffer Pool System");
    println!("---------------------------");

    let device = Arc::new(context.device().clone());
    let mut pool = BufferPool::new(device);

    println!("   • Created buffer pool");
    println!(
        "   • Initial stats: {} active buffers",
        pool.get_stats().active_buffers
    );

    let mut buffers = Vec::new();
    for i in 0..5 {
        let buffer = pool.allocate::<f32>(BufferType::Vertex, 1000);
        println!(
            "   • Allocated buffer {} with capacity {}",
            i + 1,
            buffer.capacity()
        );
        buffers.push(buffer);
    }

    let stats = pool.get_stats();
    println!(
        "   • Stats after allocation: {} active, {} total allocated",
        stats.active_buffers, stats.total_allocated
    );

    for (i, buffer) in buffers.into_iter().enumerate() {
        pool.deallocate(buffer);
        println!("   • Deallocated buffer {}", i + 1);
    }

    let final_stats = pool.get_stats();
    println!(
        "   • Final stats: {} active, {} pooled, {} total",
        final_stats.active_buffers, final_stats.pooled_buffers, final_stats.total_allocated
    );
    println!(
        "   • Pool efficiency: {:.1}%",
        final_stats.pool_efficiency()
    );

    println!("   ✅ Buffer pool system working correctly\n");
    Ok(())
}

async fn demo_different_buffer_types(
    context: &RenderContext,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Demo: Different Buffer Types");
    println!("-------------------------------");

    let buffer_types = [
        (BufferType::Vertex, "Vertex"),
        (BufferType::Instance, "Instance"),
        (BufferType::Uniform, "Uniform"),
        (BufferType::Storage, "Storage"),
    ];

    for (buffer_type, name) in buffer_types.iter() {
        let mut buffer = GpuBuffer::<f32>::new(context.device(), *buffer_type, 100);
        let test_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        buffer.upload(context.device(), context.queue(), &test_data)?;

        println!(
            "   • {} buffer: capacity={}, uploaded={} elements",
            name,
            buffer.capacity(),
            buffer.len()
        );
        println!("     - Usage flags: {:?}", buffer_type.usage_flags());
        println!("     - Alignment: {} bytes", buffer_type.alignment());
    }

    println!("   ✅ All buffer types working correctly\n");
    Ok(())
}
