// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive demonstration of GPU debugging and profiling tools.
//!
//! This example showcases all the debugging capabilities provided by the GUP-015
//! GPU Debugging Tools implementation, including:
//!
//! - Buffer content inspection and analysis
//! - Memory layout validation for Rust ↔ WGSL compatibility
//! - Shader execution profiling and performance monitoring
//! - Performance regression detection
//! - Debug report generation

use gup::{
    GupContext,
    debug::{
        GpuBufferInspector, GpuDebugContext, MemoryLayoutValidator, PerformanceBaseline,
        PerformanceSnapshot, ShaderProfiler, validate_common_gpu_structs,
    },
    interaction::ElementData,
};
use std::sync::Arc;
use std::time::Duration;
use wgpu::{BufferDescriptor, BufferUsages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔧 GPU Debugging Tools Demo");
    println!("============================");

    // Initialize GPU context
    let context = Arc::new(GupContext::headless().await?);
    let mut debug_context = GpuDebugContext::new(&context.device, &context.queue);

    // Part 1: Memory Layout Validation
    println!("\n📐 Part 1: Memory Layout Validation");
    println!("-----------------------------------");

    demonstrate_layout_validation(&mut debug_context.layout_validator).await?;

    // Part 2: Buffer Inspection
    println!("\n🔍 Part 2: Buffer Content Inspection");
    println!("------------------------------------");

    demonstrate_buffer_inspection(&mut debug_context.buffer_inspector, &context).await?;

    // Part 3: Shader Profiling
    println!("\n⏱️  Part 3: Shader Execution Profiling");
    println!("--------------------------------------");

    demonstrate_shader_profiling(&mut debug_context.shader_profiler).await?;

    // Part 4: Performance Monitoring
    println!("\n📊 Part 4: Performance Monitoring & Regression Detection");
    println!("--------------------------------------------------------");

    demonstrate_performance_monitoring(&mut debug_context).await?;

    // Part 5: Debug Report Generation
    println!("\n📋 Part 5: Debug Report Generation");
    println!("---------------------------------");

    demonstrate_debug_reporting(&debug_context).await?;

    println!("\n✅ GPU Debugging Tools Demo Complete!");
    println!("Check the generated files in the current directory:");
    println!("  - element_data_dump.json");
    println!("  - element_data_dump.csv");
    println!("  - debug_report.json");

    Ok(())
}

/// Demonstrate memory layout validation for GPU structs
async fn demonstrate_layout_validation(
    validator: &mut MemoryLayoutValidator,
) -> Result<(), Box<dyn std::error::Error>> {
    // Validate common GPU struct types
    let summary = validate_common_gpu_structs(validator)?;

    println!("Validation Summary:");
    println!("  Total structs validated: {}", summary.total_structs);
    println!("  Valid structs: {}", summary.valid_structs);
    println!("  Total errors: {}", summary.total_errors);
    println!("  Total warnings: {}", summary.total_warnings);
    println!("  Success rate: {:.1}%", summary.success_rate());

    // Display detailed results
    for result in &summary.results {
        println!(
            "\nStruct: {} (size: {} bytes, alignment: {} bytes)",
            result.struct_name, result.rust_size, result.rust_alignment
        );

        if result.is_valid {
            println!("  ✅ Valid");
        } else {
            println!("  ❌ Invalid");
        }

        for warning in &result.warnings {
            println!("  ⚠️  Warning: {warning}");
        }

        for error in &result.errors {
            println!("  🚨 Error: {error}");
        }

        for recommendation in &result.recommendations {
            println!("  💡 {recommendation}");
        }

        // Show field offsets for detailed analysis
        if !result.field_offsets.is_empty() {
            println!("  Field Layout:");
            for field in &result.field_offsets {
                println!(
                    "    {} @ offset {}: {} bytes (align: {})",
                    field.field_name, field.offset, field.size, field.alignment
                );
            }
        }
    }

    Ok(())
}

/// Demonstrate buffer content inspection and analysis
async fn demonstrate_buffer_inspection(
    inspector: &mut GpuBufferInspector,
    context: &GupContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a buffer with sample data
    let sample_elements = vec![
        ElementData {
            position: [100.0, 200.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 1,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [150.0, 250.0],
            size: [15.0, 15.0],
            mark_type: 1, // Rectangle
            element_id: 2,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [200.0, 300.0],
            size: [5.0, 20.0],
            mark_type: 2, // Line
            element_id: 3,
            selection_id: 1,
            _padding: 0,
        },
    ];

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("demo_element_buffer"),
        size: (sample_elements.len() * std::mem::size_of::<ElementData>()) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Upload sample data
    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&sample_elements));

    // Dump buffer contents to JSON
    inspector
        .dump_buffer::<ElementData>(&buffer, "element_data_dump.json")
        .await?;
    println!("✅ Buffer dumped to element_data_dump.json");

    // Dump buffer contents to CSV
    inspector
        .dump_buffer_csv::<ElementData>(&buffer, "element_data_dump.csv")
        .await?;
    println!("✅ Buffer dumped to element_data_dump.csv");

    // Analyze buffer contents
    let analysis = inspector.analyze_buffer::<ElementData>(&buffer).await?;
    println!("\nBuffer Analysis:");
    println!("  Element count: {}", analysis.element_count);
    println!("  Unique values: {}", analysis.unique_values);
    println!("  Memory usage: {} bytes", analysis.memory_usage_bytes);
    println!("  Has zero values: {}", analysis.has_zero_values);
    println!("  Has NaN values: {}", analysis.has_nan_values);
    println!("  Has infinite values: {}", analysis.has_infinite_values);

    if !analysis.anomalies.is_empty() {
        println!("  Anomalies detected:");
        for anomaly in &analysis.anomalies {
            println!("    - {anomaly}");
        }
    }

    // Show cache statistics
    let cache_stats = inspector.get_cache_stats();
    println!("\nStaging Buffer Cache:");
    println!("  Cached buffers: {}", cache_stats.buffer_count);
    println!(
        "  Total cache memory: {} bytes",
        cache_stats.total_memory_bytes
    );

    Ok(())
}

/// Demonstrate shader execution profiling
async fn demonstrate_shader_profiling(
    _profiler: &mut ShaderProfiler,
) -> Result<(), Box<dyn std::error::Error>> {
    // Note: For this demo, we'll simulate profiling results since we don't have
    // actual compute pipelines set up in this example

    println!("Simulating shader profiling results...");

    // Create mock execution stats
    let mock_stats = vec![
        create_mock_execution_stats("hit_test_shader", Duration::from_micros(500), 85.5),
        create_mock_execution_stats("spatial_index_shader", Duration::from_micros(750), 92.3),
        create_mock_execution_stats("buffer_copy_shader", Duration::from_micros(200), 45.2),
    ];

    for stats in &mock_stats {
        println!(
            "\nShader Execution: {}",
            stats
                .metadata
                .get("shader_name")
                .unwrap_or(&"unknown".to_string())
        );
        println!("  Duration: {:.2}ms", stats.duration.as_secs_f32() * 1000.0);
        println!("  GPU Utilization: {:.1}%", stats.gpu_utilization_percent);
        println!("  Dispatch Size: {:?}", stats.dispatch_size);
        println!("  Workgroup Count: {}", stats.workgroup_count);
    }

    // Demonstrate performance baseline creation
    let baseline = PerformanceBaseline::new("hit_test_baseline", Duration::from_micros(500), 85.0)
        .with_memory_usage(1024 * 1024)
        .with_threshold(1.2); // 20% increase triggers regression

    println!("\nPerformance Baseline Created:");
    println!("  Name: {}", baseline.name);
    println!(
        "  Expected Duration: {:.2}ms",
        baseline.expected_duration.as_secs_f32() * 1000.0
    );
    println!(
        "  Expected GPU Utilization: {:.1}%",
        baseline.expected_gpu_utilization
    );
    println!(
        "  Regression Threshold: {:.0}%",
        (baseline.regression_threshold - 1.0) * 100.0
    );

    Ok(())
}

/// Demonstrate performance monitoring and regression detection
async fn demonstrate_performance_monitoring(
    debug_context: &mut GpuDebugContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Record several performance snapshots
    let snapshots = [
        PerformanceSnapshot::new(16.67, 1024 * 1024)
            .with_gpu_utilization(85.5)
            .with_query_time(500.0)
            .with_metadata("test_phase", "baseline"),
        PerformanceSnapshot::new(16.42, 1024 * 1024)
            .with_gpu_utilization(87.2)
            .with_query_time(480.0)
            .with_metadata("test_phase", "optimized"),
        PerformanceSnapshot::new(20.15, 1024 * 1024 * 2) // Regression: 20% slower, 2x memory
            .with_gpu_utilization(82.1)
            .with_query_time(650.0)
            .with_metadata("test_phase", "regression"),
    ];

    for (i, snapshot) in snapshots.iter().enumerate() {
        debug_context.record_performance(snapshot.clone());
        println!(
            "📊 Recorded performance snapshot {} ({:.2}ms frame time)",
            i + 1,
            snapshot.frame_time_ms
        );
    }

    // Get performance summary
    let summary = debug_context.get_performance_summary();
    println!("\nPerformance Summary:");
    println!("  Sample count: {}", summary.sample_count);
    println!("  Average frame time: {:.2}ms", summary.avg_frame_time_ms);
    println!("  Min frame time: {:.2}ms", summary.min_frame_time_ms);
    println!("  Max frame time: {:.2}ms", summary.max_frame_time_ms);
    println!("  Average FPS: {:.1}", summary.fps);
    println!(
        "  Average memory usage: {} MB",
        summary.avg_memory_usage_bytes / (1024 * 1024)
    );
    println!(
        "  Peak memory usage: {} MB",
        summary.max_memory_usage_bytes / (1024 * 1024)
    );

    Ok(())
}

/// Demonstrate debug report generation
async fn demonstrate_debug_reporting(
    debug_context: &GpuDebugContext,
) -> Result<(), Box<dyn std::error::Error>> {
    // Export comprehensive debug report
    debug_context
        .export_debug_report("debug_report.json")
        .await?;
    println!("✅ Debug report exported to debug_report.json");

    // Show config summary
    let config = &debug_context.config;
    println!("\nDebug Configuration:");
    println!("  Buffer inspection: {}", config.enable_buffer_inspection);
    println!("  Layout validation: {}", config.enable_layout_validation);
    println!("  Shader profiling: {}", config.enable_shader_profiling);
    println!(
        "  Performance monitoring: {}",
        config.enable_performance_monitoring
    );
    println!("  Debug output dir: {}", config.debug_output_dir);
    println!(
        "  Max buffer inspect size: {} MB",
        config.max_buffer_inspect_size / (1024 * 1024)
    );

    println!("\nPerformance Thresholds:");
    let thresholds = &config.performance_thresholds;
    println!("  Max frame time: {:.2}ms", thresholds.max_frame_time_ms);
    println!("  Max query time: {:.0}μs", thresholds.max_query_time_us);
    println!(
        "  Max memory usage: {} MB",
        thresholds.max_memory_usage_bytes / (1024 * 1024)
    );
    println!(
        "  Regression threshold: {:.0}%",
        thresholds.regression_threshold_percent
    );

    Ok(())
}

/// Helper function to create mock shader execution statistics
fn create_mock_execution_stats(
    shader_name: &str,
    duration: Duration,
    gpu_utilization: f32,
) -> gup::debug::ShaderExecutionStats {
    use std::collections::HashMap;

    gup::debug::ShaderExecutionStats {
        duration,
        gpu_utilization_percent: gpu_utilization,
        dispatch_size: (64, 64, 1),
        workgroup_count: 4096,
        memory_bandwidth_gbps: 500.0,
        instructions_per_second: 1_000_000.0,
        timestamp: chrono::Utc::now(),
        metadata: {
            let mut map = HashMap::new();
            map.insert("shader_name".to_string(), shader_name.to_string());
            map
        },
        used_hardware_timestamps: false,
    }
}
