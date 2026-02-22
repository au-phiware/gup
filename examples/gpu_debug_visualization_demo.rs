// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive GPU Debug Data Visualization Demo
//!
//! This example demonstrates the advanced debug data visualization capabilities
//! implemented in GUP-081, showcasing:
//!
//! - GPU-accelerated interactive visualizations using Gup itself (dog-fooding)
//! - Real-time performance trend monitoring
//! - Memory usage visualization
//! - Buffer content analysis
//! - Performance dashboard creation
//!
//! This demonstrates how Gup can visualize its own GPU debug data using the
//! same GPU-accelerated rendering primitives it provides for application data.

use gup::debug::{
    BufferVisualizationType, GpuDebugContext, GpuDebugVisualizer, PerformanceSnapshot,
};
use gup::interaction::ElementData;
use gup::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🎨 Interactive GPU Debug Visualization Demo");
    println!("============================================");
    println!("\nThis demo showcases GUP-081: Advanced Debug Data Visualization");
    println!("Dog-fooding: Using Gup to visualize its own GPU debug data!\n");

    // Initialize GPU context
    let context = Arc::new(RenderContext::new().await?);
    println!("✅ GPU context initialized");

    // Create debug context
    let mut debug_context = GpuDebugContext::new(context.device(), context.queue());
    println!("✅ Debug context initialized");

    // Create visualizer
    let visualizer = debug_context.create_visualizer(context.clone());
    println!("✅ Interactive visualizer created");

    // Part 1: Performance Trend Visualization
    println!("\n📊 Part 1: Performance Trend Visualization");
    println!("-------------------------------------------");
    demonstrate_performance_visualization(&visualizer, &mut debug_context).await?;

    // Part 2: Memory Usage Visualization
    println!("\n💾 Part 2: Memory Usage Visualization");
    println!("-------------------------------------");
    demonstrate_memory_visualization(&visualizer).await?;

    // Part 3: Buffer Content Visualization
    println!("\n🔍 Part 3: Buffer Content Visualization");
    println!("---------------------------------------");
    demonstrate_buffer_visualization(&visualizer, &context).await?;

    // Part 4: Integrated Dashboard
    println!("\n📈 Part 4: Performance Dashboard");
    println!("--------------------------------");
    demonstrate_dashboard_creation(&visualizer, &mut debug_context).await?;

    // Part 5: Visualization Configuration
    println!("\n⚙️  Part 5: Visualization Configuration");
    println!("---------------------------------------");
    demonstrate_configuration_options(&context).await?;

    println!("\n✨ Interactive GPU Debug Visualization Demo Complete!");
    println!("\nKey Features Demonstrated:");
    println!("  ✓ GPU-accelerated performance trend charts");
    println!("  ✓ Interactive memory usage visualizations");
    println!("  ✓ Buffer content analysis and display");
    println!("  ✓ Integrated performance dashboards");
    println!("  ✓ Configurable visualization options");
    println!("\nThis demonstrates the power of dog-fooding: Gup visualizing");
    println!("its own GPU debug data using the same primitives it provides!");

    Ok(())
}

/// Demonstrate performance trend visualization
async fn demonstrate_performance_visualization(
    visualizer: &GpuDebugVisualizer,
    debug_context: &mut GpuDebugContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Generate simulated performance data
    let mut snapshots = Vec::new();

    println!("\nGenerating simulated performance data...");

    // Simulate 100 frames of performance data
    for i in 0..100 {
        let time_ms = i as f32 * 0.1; // 100ms of data
        let base_frame_time = 16.67; // Target 60 FPS

        // Add some variation and occasional spikes
        let variation = (time_ms * 0.5).sin() * 2.0;
        let spike = if i % 30 == 0 { 5.0 } else { 0.0 };
        let frame_time = base_frame_time + variation + spike;

        let memory = 1024 * 1024 * (50 + (time_ms * 0.1).sin() as u64 * 10); // 50-60 MB range
        let gpu_util = 80.0 + (time_ms * 0.3).cos() * 10.0; // 70-90% range

        let snapshot = PerformanceSnapshot::new(frame_time, memory)
            .with_gpu_utilization(gpu_util)
            .with_query_time(500.0 + (time_ms * 0.2).sin() * 100.0)
            .with_metadata(
                "phase",
                if i < 33 {
                    "startup"
                } else if i < 66 {
                    "steady"
                } else {
                    "peak"
                },
            );

        snapshots.push(snapshot.clone());
        debug_context.record_performance(snapshot);
    }

    println!("Generated {} performance snapshots", snapshots.len());

    // Create visualization
    println!("\nCreating interactive performance trend chart...");
    let chart = visualizer.visualize_performance_trends(&snapshots).await?;

    println!("✅ Performance trend chart created");
    println!("\nChart Statistics:");
    println!("  • Data points: {}", chart.data_point_count());

    if let Some((start, end)) = chart.time_range() {
        let duration = end - start;
        println!(
            "  • Time range: {:.2}s",
            duration.num_milliseconds() as f64 / 1000.0
        );
    }

    let stats = chart.get_statistics();
    println!("  • Average frame time: {:.2}ms", stats.avg_frame_time_ms);
    println!("  • Min frame time: {:.2}ms", stats.min_frame_time_ms);
    println!("  • Max frame time: {:.2}ms", stats.max_frame_time_ms);
    println!("  • Average FPS: {:.1}", stats.fps);
    println!(
        "  • Average memory: {:.2} MB",
        stats.avg_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  • Peak memory: {:.2} MB",
        stats.max_memory_bytes as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

/// Demonstrate memory usage visualization
async fn demonstrate_memory_visualization(
    visualizer: &GpuDebugVisualizer,
) -> Result<(), Box<dyn std::error::Error>> {
    use gup::debug::MemorySnapshot;
    use std::time::Instant;

    println!("\nGenerating simulated memory data...");

    // Generate simulated memory snapshots
    let mut snapshots = Vec::new();
    let start_time = Instant::now();

    for i in 0..50 {
        let elapsed = i as f64 * 20.0; // 20ms intervals
        let base_memory = 10 * 1024 * 1024; // 10MB base

        // Simulate memory growth with periodic cleanup
        let growth = (i * 1024 * 100) as u64; // 100KB per sample
        let cleanup = if i % 10 == 0 && i > 0 {
            ((i / 10) * 1024 * 500) as u64
        } else {
            0
        };

        let total_memory = base_memory + growth - cleanup;
        let active_allocations = 100 + i * 5 - (i / 10) * 20;

        snapshots.push(MemorySnapshot {
            timestamp: start_time + std::time::Duration::from_millis(elapsed as u64),
            total_memory,
            active_allocations,
        });
    }

    println!("Generated {} memory snapshots", snapshots.len());

    // Create visualization
    println!("\nCreating interactive memory trend chart...");
    let chart = visualizer.visualize_memory_trends(&snapshots).await?;

    println!("✅ Memory trend chart created");
    println!("\nChart Statistics:");
    println!("  • Data points: {}", chart.data_point_count());

    let stats = chart.get_statistics();
    println!(
        "  • Average memory: {:.2} MB",
        stats.avg_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  • Min memory: {:.2} MB",
        stats.min_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  • Max memory: {:.2} MB",
        stats.max_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("  • Average allocations: {}", stats.avg_allocations);
    println!("  • Min allocations: {}", stats.min_allocations);
    println!("  • Max allocations: {}", stats.max_allocations);

    Ok(())
}

/// Demonstrate buffer content visualization
async fn demonstrate_buffer_visualization(
    visualizer: &GpuDebugVisualizer,
    _context: &Arc<RenderContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nCreating sample GPU buffer...");

    // Create sample buffer data
    let mut element_data = Vec::new();

    // Generate scattered points in a circular pattern
    for i in 0..200 {
        let angle = (i as f32 / 200.0) * std::f32::consts::PI * 2.0;
        let radius = 100.0 + (i as f32 * 0.5).sin() * 50.0;

        let x = 400.0 + angle.cos() * radius;
        let y = 300.0 + angle.sin() * radius;

        element_data.push(ElementData {
            position: [x, y],
            size: [5.0, 5.0],
            mark_type: (i % 3) as u32, // Rotate through mark types
            element_id: i as u32,
            selection_id: (i / 50) as u32, // Group into 4 selections
            _padding: 0,
        });
    }

    println!("Generated {} buffer elements", element_data.len());

    // Create different visualizations for the same data
    println!("\nCreating scatter plot visualization...");
    let scatter_viz = visualizer
        .visualize_buffer_contents(&element_data, BufferVisualizationType::ScatterPlot)
        .await?;

    println!("✅ Scatter plot created");
    println!("  • Elements: {}", scatter_viz.element_count());
    println!("  • Type: {:?}", scatter_viz.visualization_type());

    println!("\nCreating histogram visualization...");
    let histogram_viz = visualizer
        .visualize_buffer_contents(&element_data, BufferVisualizationType::Histogram)
        .await?;

    println!("✅ Histogram created");
    println!("  • Elements: {}", histogram_viz.element_count());
    println!("  • Type: {:?}", histogram_viz.visualization_type());

    println!("\nCreating heatmap visualization...");
    let heatmap_viz = visualizer
        .visualize_buffer_contents(&element_data, BufferVisualizationType::Heatmap)
        .await?;

    println!("✅ Heatmap created");
    println!("  • Elements: {}", heatmap_viz.element_count());
    println!("  • Type: {:?}", heatmap_viz.visualization_type());

    Ok(())
}

/// Demonstrate dashboard creation
async fn demonstrate_dashboard_creation(
    visualizer: &GpuDebugVisualizer,
    debug_context: &mut GpuDebugContext,
) -> Result<(), Box<dyn std::error::Error>> {
    use gup::debug::MemorySnapshot;
    use std::time::Instant;

    println!("\nCreating integrated performance dashboard...");

    // Get performance data from debug context
    let perf_data = debug_context.performance_history().to_vec();

    // Generate corresponding memory data
    let mut mem_data = Vec::new();
    let start_time = Instant::now();

    for i in 0..perf_data.len() {
        mem_data.push(MemorySnapshot {
            timestamp: start_time + std::time::Duration::from_millis((i * 10) as u64),
            total_memory: 50 * 1024 * 1024 + (i as u64 * 1024 * 100),
            active_allocations: 100 + i * 2,
        });
    }

    // Create dashboard
    let dashboard = visualizer
        .create_performance_dashboard(&perf_data, &mem_data)
        .await?;

    println!("✅ Performance dashboard created");
    println!("\nDashboard Contents:");
    println!(
        "  • Performance chart: {} data points",
        dashboard.performance_chart().data_point_count()
    );
    println!(
        "  • Memory chart: {} data points",
        dashboard.memory_chart().data_point_count()
    );

    let perf_stats = dashboard.performance_chart().get_statistics();
    let mem_stats = dashboard.memory_chart().get_statistics();

    println!("\nPerformance Overview:");
    println!("  • Average FPS: {:.1}", perf_stats.fps);
    println!(
        "  • Frame time: {:.2}ms (min: {:.2}ms, max: {:.2}ms)",
        perf_stats.avg_frame_time_ms, perf_stats.min_frame_time_ms, perf_stats.max_frame_time_ms
    );

    println!("\nMemory Overview:");
    println!(
        "  • Average: {:.2} MB",
        mem_stats.avg_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  • Peak: {:.2} MB",
        mem_stats.max_memory_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("  • Average allocations: {}", mem_stats.avg_allocations);

    Ok(())
}

/// Demonstrate configuration options
async fn demonstrate_configuration_options(
    context: &Arc<RenderContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    use gup::debug::{ColorScheme, VisualizationConfig};

    println!("\nDemonstrating visualization configuration options...");

    // Default configuration
    let default_config = VisualizationConfig::default();
    println!("\nDefault Configuration:");
    println!(
        "  • Size: {}x{}",
        default_config.width, default_config.height
    );
    println!("  • Interactive: {}", default_config.enable_interaction);
    println!("  • Color scheme: {:?}", default_config.color_scheme);
    println!("  • Max data points: {}", default_config.max_data_points);

    // Custom configurations
    let configs = vec![
        (
            "High Resolution",
            VisualizationConfig {
                width: 1920,
                height: 1080,
                enable_interaction: true,
                color_scheme: ColorScheme::Default,
                max_data_points: 20_000,
            },
        ),
        (
            "Performance Mode",
            VisualizationConfig {
                width: 800,
                height: 600,
                enable_interaction: false,
                color_scheme: ColorScheme::Grayscale,
                max_data_points: 5_000,
            },
        ),
        (
            "Accessibility",
            VisualizationConfig {
                width: 1024,
                height: 768,
                enable_interaction: true,
                color_scheme: ColorScheme::HighContrast,
                max_data_points: 10_000,
            },
        ),
        (
            "Warm Theme",
            VisualizationConfig {
                width: 800,
                height: 600,
                enable_interaction: true,
                color_scheme: ColorScheme::Warm,
                max_data_points: 10_000,
            },
        ),
        (
            "Cool Theme",
            VisualizationConfig {
                width: 800,
                height: 600,
                enable_interaction: true,
                color_scheme: ColorScheme::Cool,
                max_data_points: 10_000,
            },
        ),
    ];

    for (name, config) in configs {
        println!("\n{} Configuration:", name);
        println!("  • Size: {}x{}", config.width, config.height);
        println!("  • Interactive: {}", config.enable_interaction);
        println!("  • Color scheme: {:?}", config.color_scheme);
        println!("  • Max data points: {}", config.max_data_points);

        // Create visualizer with custom config
        let _visualizer = GpuDebugVisualizer::with_config(context.clone(), config);
        println!("  ✅ Visualizer created with {} configuration", name);
    }

    Ok(())
}
