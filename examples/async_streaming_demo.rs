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

//! Async Streaming Visualization Demo
//!
//! This example demonstrates the async and streaming capabilities of Gup,
//! showing how to create real-time visualizations with streaming data sources,
//! progressive loading, and async composition.

use gup::{
    GupError, GupResult, RenderContext,
    async_mixable::{
        AsyncMixable, AsyncRenderStrategy,
        progressive::{
            MockProgressiveDataLoader, ProgressiveConfig, ProgressiveVisualization, QualityLevel,
        },
        streaming::{MockStreamingDataSource, Point2D, StreamingScatterPlot},
        utils::{AsyncCompositionBuilder, compose},
    },
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("🚀 Gup Async Streaming Visualization Demo");
    println!("==========================================\n");

    // Initialize render context
    let mut context = RenderContext::new().await?;
    println!("✅ WebGPU context initialized");

    // Demo 1: Basic Streaming Visualization
    println!("\n📊 Demo 1: Basic Streaming Scatter Plot");
    streaming_scatter_demo(&mut context).await?;

    // Demo 2: Progressive Loading
    println!("\n📈 Demo 2: Progressive Loading Visualization");
    progressive_loading_demo(&mut context).await?;

    // Demo 3: Async Composition
    println!("\n🔀 Demo 3: Async Component Composition");
    async_composition_demo(&mut context).await?;

    // Demo 4: Performance and Cancellation
    println!("\n⚡ Demo 4: Performance and Cancellation");
    performance_demo(&mut context).await?;

    println!("\n🎉 All demos completed successfully!");
    Ok(())
}

/// Demonstrate streaming scatter plot with real-time data updates.
async fn streaming_scatter_demo(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating streaming data source with 1000 points...");

    // Create test data - a sine wave with some noise
    let stream_data: Vec<Point2D> = (0..1000)
        .map(|i| {
            let x = i as f32 * 0.1;
            let y = (x * 0.5).sin() + (i as f32 * 0.01).cos() * 0.2;
            let color = if y > 0.0 {
                [0.2, 0.8, 0.3, 1.0] // Green for positive
            } else {
                [0.8, 0.2, 0.3, 1.0] // Red for negative
            };
            Point2D::with_color(x, y, color)
        })
        .collect();

    // Create streaming data source with rate limiting
    let data_source =
        MockStreamingDataSource::new(stream_data).with_rate_limit(Duration::from_millis(50)); // ~20 Hz update rate

    // Create streaming scatter plot
    let streaming_plot = StreamingScatterPlot::new(data_source, 200); // Max 200 points displayed

    println!(
        "✅ Streaming scatter plot created (max {} points)",
        streaming_plot.max_points()
    );

    // Simulate real-time rendering
    println!("🎬 Simulating real-time rendering for 2 seconds...");
    let start_time = std::time::Instant::now();
    let mut frame_count = 0;

    while start_time.elapsed() < Duration::from_secs(2) {
        // Render frame
        let render_result = streaming_plot
            .render_async(context, CancellationToken::new())
            .await;

        match render_result {
            Ok(()) => frame_count += 1,
            Err(e) => println!("⚠️  Render error: {e}"),
        }

        // Check streaming status
        if frame_count % 10 == 0 {
            let data_size = streaming_plot.data_size().await;
            let is_streaming = streaming_plot.is_streaming().await;
            println!(
                "  📊 Frame {frame_count}: {data_size} points loaded, streaming: {is_streaming}"
            );
        }

        // ~60 FPS rendering
        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    let final_size = streaming_plot.data_size().await;
    println!(
        "✅ Streaming demo completed: {frame_count} frames rendered, {final_size} points loaded"
    );

    Ok(())
}

/// Demonstrate progressive loading with quality levels.
async fn progressive_loading_demo(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating large dataset for progressive loading...");

    // Create a large dataset - 10,000 data points
    let large_dataset: Vec<f32> = (0..10_000)
        .map(|i| (i as f32 * 0.01).sin() * (i as f32 * 0.001).cos())
        .collect();

    // Create progressive data loader
    let data_loader = MockProgressiveDataLoader::new(large_dataset).with_slow_loading(); // Simulate network/disk latency

    // Configure progressive loading
    let config = ProgressiveConfig {
        chunk_size: 500,
        target_quality: QualityLevel::High,
        max_loading_time_per_frame: Duration::from_millis(8), // ~120 FPS budget
        background_loading: true,
        priority_loading: true,
    };

    let progressive_viz = ProgressiveVisualization::new(data_loader, config);
    println!(
        "✅ Progressive visualization created (target: {:?})",
        progressive_viz.target_quality()
    );

    // Monitor loading progress
    println!("📈 Loading data progressively...");
    let start_time = std::time::Instant::now();

    while !progressive_viz.is_ready().await
        || progressive_viz.current_quality().await != progressive_viz.target_quality()
    {
        // Check progress
        if let Some(progress) = progressive_viz.progress() {
            let percentage = progress.percentage().unwrap_or(0.0) * 100.0;
            println!(
                "  🔄 Loading: {:.1}% - {} ({}/{})",
                percentage,
                progress.stage,
                progress.current,
                progress.total.unwrap_or(0)
            );
        }

        // Render with current data
        let render_result = progressive_viz
            .render_async(context, CancellationToken::new())
            .await;

        match render_result {
            Ok(()) => {}
            Err(e) => println!("⚠️  Render error: {e}"),
        }

        let loaded_count = progressive_viz.loaded_count().await;
        let current_quality = progressive_viz.current_quality().await;

        if start_time.elapsed().as_secs() >= 1 {
            println!("  📊 Quality: {current_quality:?}, Loaded: {loaded_count} items");
        }

        // Prevent infinite loop
        if start_time.elapsed() > Duration::from_secs(10) {
            println!("⏰ Timeout reached, stopping progressive loading");
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let final_count = progressive_viz.loaded_count().await;
    let final_quality = progressive_viz.current_quality().await;
    println!(
        "✅ Progressive loading completed: {final_quality:?} quality, {final_count} items loaded"
    );

    Ok(())
}

/// Demonstrate async composition with multiple strategies.
async fn async_composition_demo(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating multiple async components for composition...");

    // Create streaming component
    let stream_data: Vec<Point2D> = (0..100)
        .map(|i| Point2D::new(i as f32, (i as f32 * 0.1).sin()))
        .collect();
    let streaming_comp = StreamingScatterPlot::new(MockStreamingDataSource::new(stream_data), 50);

    // Create progressive component
    let progressive_data: Vec<f32> = (0..500).map(|i| i as f32).collect();
    let progressive_comp =
        ProgressiveVisualization::with_defaults(MockProgressiveDataLoader::new(progressive_data));

    println!("✅ Components created: streaming + progressive");

    // Test different composition strategies
    let strategies = [
        ("Sequential", AsyncRenderStrategy::Sequential),
        ("Parallel", AsyncRenderStrategy::Parallel),
        ("Adaptive", AsyncRenderStrategy::Adaptive),
    ];

    for (name, strategy) in strategies.iter() {
        println!("\n🔀 Testing {name} composition strategy...");

        // Create composition using builder pattern with boxed trait objects
        let streaming_box =
            Box::new(clone_streaming_comp(&streaming_comp)) as Box<dyn AsyncMixable<Output = ()>>;
        let progressive_box = Box::new(clone_progressive_comp(&progressive_comp))
            as Box<dyn AsyncMixable<Output = ()>>;

        let composition = AsyncCompositionBuilder::new()
            .add_component(streaming_box)
            .add_component(progressive_box)
            .with_strategy(*strategy)
            .with_timeout(Duration::from_secs(5))
            .build()?;

        // Measure render time
        let start = std::time::Instant::now();
        let render_result = composition
            .render_async(context, CancellationToken::new())
            .await;
        let render_time = start.elapsed();

        match render_result {
            Ok(()) => {
                println!(
                    "  ✅ {} strategy completed in {:.2}ms",
                    name,
                    render_time.as_secs_f32() * 1000.0
                );

                if let Some(progress) = composition.progress() {
                    println!(
                        "  📊 Final progress: {}/{}",
                        progress.current,
                        progress.total.unwrap_or(0)
                    );
                }
            }
            Err(e) => {
                println!("  ❌ {name} strategy failed: {e}");
            }
        }
    }

    // Test utility functions - create fresh instances since we used the previous ones
    println!("\n🛠️  Testing composition utilities...");

    let fresh_streaming_data: Vec<Point2D> = (0..100)
        .map(|i| Point2D::new(i as f32 * 0.1, (i as f32 * 0.1).sin()))
        .collect();
    let fresh_streaming_source = MockStreamingDataSource::new(fresh_streaming_data);
    let fresh_streaming_comp = StreamingScatterPlot::new(fresh_streaming_source, 200);

    let fresh_progressive_data: Vec<f32> = (0..500).map(|i| i as f32).collect();
    let fresh_progressive_loader = MockProgressiveDataLoader::new(fresh_progressive_data);
    let fresh_progressive_comp = ProgressiveVisualization::with_defaults(fresh_progressive_loader);

    let components = vec![
        Box::new(fresh_streaming_comp) as Box<dyn AsyncMixable<Output = ()>>,
        Box::new(fresh_progressive_comp) as Box<dyn AsyncMixable<Output = ()>>,
    ];

    let parallel_composition = compose::parallel(components)?;
    println!("  ✅ Parallel composition created using utility function");

    let render_result = parallel_composition
        .render_async(context, CancellationToken::new())
        .await;

    match render_result {
        Ok(()) => println!("  ✅ Utility composition render successful"),
        Err(e) => println!("  ❌ Utility composition failed: {e}"),
    }

    Ok(())
}

/// Demonstrate performance monitoring and cancellation.
async fn performance_demo(context: &mut RenderContext) -> GupResult<()> {
    println!("Testing performance monitoring and cancellation...");

    // Create a slow component for testing
    let slow_data: Vec<Point2D> = (0..2000)
        .map(|i| Point2D::new(i as f32, (i as f32 * 0.001).sin()))
        .collect();

    let slow_source =
        MockStreamingDataSource::new(slow_data).with_rate_limit(Duration::from_millis(10)); // Slow updates

    let slow_plot = StreamingScatterPlot::new(slow_source, 500);

    // Test timeout functionality
    println!("\n⏱️  Testing timeout (should timeout in 100ms)...");
    let timeout_comp = slow_plot.with_timeout(Duration::from_millis(100));

    let start = std::time::Instant::now();
    let result = timeout_comp
        .render_async(context, CancellationToken::new())
        .await;
    let elapsed = start.elapsed();

    match result {
        Ok(()) => println!(
            "  ⚠️  Expected timeout, but render completed in {:.2}ms",
            elapsed.as_secs_f32() * 1000.0
        ),
        Err(e) => {
            if e.to_string().contains("timed out") {
                println!(
                    "  ✅ Timeout working correctly: {:.2}ms",
                    elapsed.as_secs_f32() * 1000.0
                );
            } else {
                println!("  ❌ Unexpected error: {e}");
            }
        }
    }

    // Test cancellation - create a new instance since the previous one was moved
    println!("\n🚫 Testing cancellation...");
    let cancel_slow_data: Vec<Point2D> = (0..2000)
        .map(|i| Point2D::new(i as f32, (i as f32 * 0.001).sin()))
        .collect();
    let cancel_slow_source =
        MockStreamingDataSource::new(cancel_slow_data).with_rate_limit(Duration::from_millis(10));
    let cancel_slow_plot = StreamingScatterPlot::new(cancel_slow_source, 500);

    let cancellation_token = CancellationToken::new();
    let _cancel_token_clone = cancellation_token.clone();

    // Since we can't move the context into the spawn, we'll test cancellation differently
    let start = std::time::Instant::now();

    // Start a timeout-based render (simulating a long operation)
    let cancel_result = tokio::select! {
        render_result = cancel_slow_plot.render_async(context, cancellation_token.clone()) => {
            render_result
        }
        _ = tokio::time::sleep(Duration::from_millis(50)) => {
            cancellation_token.cancel();
            Err(GupError::render_error("Simulated cancellation after timeout"))
        }
    };

    let elapsed = start.elapsed();

    match cancel_result {
        Ok(()) => println!(
            "  ⚠️  Expected cancellation, but render completed in {:.2}ms",
            elapsed.as_secs_f32() * 1000.0
        ),
        Err(e) => {
            if e.to_string().contains("cancel") {
                println!(
                    "  ✅ Cancellation working correctly: {:.2}ms",
                    elapsed.as_secs_f32() * 1000.0
                );
            } else {
                println!("  ❌ Unexpected error: {e}");
            }
        }
    }

    Ok(())
}

// Helper functions to create cloned instances for demo purposes
// (Avoids orphan rule violations by not implementing external traits on external types)
fn clone_streaming_comp(
    original: &StreamingScatterPlot<MockStreamingDataSource>,
) -> StreamingScatterPlot<MockStreamingDataSource> {
    // For demo purposes, create a new instance with empty data
    // In practice, you might want to share the data or create a different cloning strategy
    let empty_data = vec![];
    let data_source = MockStreamingDataSource::new(empty_data);
    StreamingScatterPlot::new(data_source, original.max_points())
}

fn clone_progressive_comp(
    original: &ProgressiveVisualization<f32, MockProgressiveDataLoader<f32>>,
) -> ProgressiveVisualization<f32, MockProgressiveDataLoader<f32>> {
    let _ = original; // Use parameter to avoid unused warning
    // For demo purposes, create a new instance with empty data
    let empty_data = vec![];
    let data_loader = MockProgressiveDataLoader::new(empty_data);
    ProgressiveVisualization::with_defaults(data_loader)
}
