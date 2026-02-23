// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating buffer validation and debugging tools.
//!
//! This example shows how to use the buffer validation system to detect common
//! buffer issues like NaN values, out-of-range data, and inefficient memory usage.

use gup::GupContext;
use gup::debug::{
    BufferSizeValidationRule, FiniteValueRule, GpuBufferInspector, RangeValidationRule,
    UtilizationValidationRule, ValidationRule,
};
use std::sync::Arc;
use wgpu::{BufferDescriptor, BufferUsages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Buffer Validation Example ===\n");

    // Create GPU context
    let context = Arc::new(GupContext::new().await?);
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);

    // Example 1: Detect NaN and infinite values
    println!("Example 1: Detecting non-finite values");
    println!("----------------------------------------");

    let data_with_nan = vec![1.0f32, 2.0, f32::NAN, 4.0, f32::INFINITY, 6.0];
    let buffer_nan = create_buffer(&context, &data_with_nan, "nan_buffer");

    let rules_finite: Vec<Box<dyn ValidationRule<f32>>> = vec![Box::new(FiniteValueRule)];

    let report = inspector
        .validate_buffer::<f32>(&buffer_nan, data_with_nan.len(), rules_finite)
        .await?;

    println!("{}", report.format_report());

    // Example 2: Range validation for normalized values
    println!("\nExample 2: Range validation");
    println!("---------------------------");

    let data_out_of_range = vec![0.0f32, 0.5, 0.9, 1.5, 2.0]; // Last two out of [0,1]
    let buffer_range = create_buffer(&context, &data_out_of_range, "range_buffer");

    let rules_range: Vec<Box<dyn ValidationRule<f32>>> =
        vec![Box::new(RangeValidationRule::new(0.0, 1.0, "normalized"))];

    let report = inspector
        .validate_buffer::<f32>(&buffer_range, data_out_of_range.len(), rules_range)
        .await?;

    println!("{}", report.format_report());

    // Example 3: Buffer utilization check
    println!("\nExample 3: Buffer utilization analysis");
    println!("--------------------------------------");

    // Create a buffer that's under-utilized
    let data_small = vec![1.0f32; 20];
    let large_capacity = 200;
    let buffer_underutilized = create_buffer(&context, &data_small, "underutilized_buffer");

    let rules_util: Vec<Box<dyn ValidationRule<f32>>> =
        vec![Box::new(UtilizationValidationRule::new(50.0))];

    let report = inspector
        .validate_buffer::<f32>(&buffer_underutilized, large_capacity, rules_util)
        .await?;

    println!("{}", report.format_report());

    // Example 4: Multiple validation rules
    println!("\nExample 4: Comprehensive validation");
    println!("-----------------------------------");

    // Data with multiple issues
    let problematic_data = vec![0.0f32, f32::NAN, 1.5, 2.0, 3.0];
    let buffer_multi = create_buffer(&context, &problematic_data, "multi_issue_buffer");

    let rules_multi: Vec<Box<dyn ValidationRule<f32>>> = vec![
        Box::new(FiniteValueRule),
        Box::new(RangeValidationRule::new(0.0, 1.0, "position")),
        Box::new(BufferSizeValidationRule::new(10, 1000)),
    ];

    let report = inspector
        .validate_buffer::<f32>(&buffer_multi, problematic_data.len(), rules_multi)
        .await?;

    println!("{}", report.format_report());

    // Example 5: Statistical summary
    println!("\nExample 5: Statistical analysis");
    println!("-------------------------------");

    let numeric_data: Vec<f32> = (1..=100).map(|x| x as f32 / 10.0).collect();
    let buffer_stats = create_buffer(&context, &numeric_data, "stats_buffer");

    let summary = inspector
        .create_statistical_summary::<f32>(&buffer_stats)
        .await?;

    println!("{}", summary);

    // Example 6: Buffer comparison
    println!("\nExample 6: Buffer comparison");
    println!("---------------------------");

    let data_a = vec![1.0f32, 2.0, 3.0, 4.0];
    let data_b = vec![1.0f32, 2.0, 3.0, 5.0]; // Last value different

    let buffer_a = create_buffer(&context, &data_a, "buffer_a");
    let buffer_b = create_buffer(&context, &data_b, "buffer_b");

    let comparison = inspector
        .compare_buffers::<f32>(&buffer_a, &buffer_b, 0.01)
        .await?;

    println!("Buffers identical: {}", comparison.is_identical);
    println!("Similarity: {:.1}%", comparison.similarity_percentage);
    if !comparison.differences.is_empty() {
        println!("Differences found:");
        for diff in &comparison.differences {
            println!(
                "  Index {}: {} vs {}",
                diff.index, diff.value_a, diff.value_b
            );
        }
    }

    // Example 7: Buffer anomaly detection
    println!("\nExample 7: Anomaly detection");
    println!("---------------------------");

    let anomaly_data = vec![1.0f32, 2.0, f32::NAN, f32::INFINITY, 0.0, 5.0];
    let buffer_anomaly = create_buffer(&context, &anomaly_data, "anomaly_buffer");

    let analysis = inspector.analyze_buffer::<f32>(&buffer_anomaly).await?;

    println!("Element count: {}", analysis.element_count);
    println!("Has NaN values: {}", analysis.has_nan_values);
    println!("Has infinite values: {}", analysis.has_infinite_values);
    println!("Has zero values: {}", analysis.has_zero_values);
    if !analysis.anomalies.is_empty() {
        println!("\nAnomalies detected:");
        for anomaly in &analysis.anomalies {
            println!("  {}", anomaly);
        }
    }

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Helper function to create a buffer and write data to it
fn create_buffer(context: &GupContext, data: &[f32], label: &str) -> wgpu::Buffer {
    let buffer_size = std::mem::size_of_val(data) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(data));

    buffer
}
