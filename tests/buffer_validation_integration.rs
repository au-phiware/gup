// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for buffer validation and debugging tools.
//!
//! These tests demonstrate the complete buffer validation workflow including
//! validation rules, buffer inspection, and debug wrappers.

use gup::GupContext;
use gup::debug::{
    BufferSizeValidationRule, FiniteValueRule, GpuBufferInspector, RangeValidationRule,
    UtilizationValidationRule, ValidationSeverity,
};
use std::sync::Arc;
use wgpu::BufferDescriptor;
use wgpu::BufferUsages;

#[tokio::test]
async fn test_finite_value_validation() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer with some invalid values
    let data = vec![1.0f32, 2.0, f32::NAN, 4.0, f32::INFINITY];
    let buffer_size = (data.len() * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Validate with FiniteValueRule
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let rules: Vec<Box<dyn gup::debug::ValidationRule<f32>>> = vec![Box::new(FiniteValueRule)];

    let report = inspector
        .validate_buffer::<f32>(&buffer, data.len(), rules)
        .await
        .unwrap();

    // Should detect the non-finite values
    assert!(report.has_errors());
    assert!(!report.results[0].passed);
    assert_eq!(report.results[0].severity, ValidationSeverity::Error);
}

#[tokio::test]
async fn test_range_validation() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer with values outside expected range
    let data = vec![0.0f32, 0.5, 0.8, 1.5, 2.0]; // Last two out of [0, 1] range
    let buffer_size = (data.len() * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Validate with RangeValidationRule
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let rules: Vec<Box<dyn gup::debug::ValidationRule<f32>>> =
        vec![Box::new(RangeValidationRule::new(0.0, 1.0, "test"))];

    let report = inspector
        .validate_buffer::<f32>(&buffer, data.len(), rules)
        .await
        .unwrap();

    // Should detect out-of-range values
    assert!(report.has_warnings());
    assert!(!report.results[0].passed);
}

#[tokio::test]
async fn test_utilization_validation() {
    // This test demonstrates that validation needs to know the actual used length
    // In practice, this would come from GpuBuffer which tracks len separately
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer where we know we're only using a small portion
    let actual_data_len = 10;
    let buffer_capacity = 100;

    // For this test, we'll simulate by creating a smaller buffer and comparing against capacity
    let data = vec![1.0f32; actual_data_len];
    let buffer_size = (actual_data_len * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Validate with UtilizationValidationRule (50% minimum)
    // Pass the larger capacity to simulate under-utilization
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let rules: Vec<Box<dyn gup::debug::ValidationRule<f32>>> =
        vec![Box::new(UtilizationValidationRule::new(50.0))];

    let report = inspector
        .validate_buffer::<f32>(&buffer, buffer_capacity, rules)
        .await
        .unwrap();

    // Debug: print the report
    println!("{}", report.format_report());

    // Should detect low utilization (10 used out of 100 capacity = 10%)
    assert!(report.has_warnings());
    assert!(!report.results[0].passed);
}

#[tokio::test]
async fn test_buffer_size_validation() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer that is too small
    let data = vec![1.0f32; 50]; // Less than minimum of 100
    let buffer_size = (data.len() * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Validate with BufferSizeValidationRule
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let rules: Vec<Box<dyn gup::debug::ValidationRule<f32>>> =
        vec![Box::new(BufferSizeValidationRule::new(100, 1000))];

    let report = inspector
        .validate_buffer::<f32>(&buffer, data.len(), rules)
        .await
        .unwrap();

    // Should detect buffer too small
    assert!(report.has_errors());
    assert!(!report.results[0].passed);
}

#[tokio::test]
async fn test_multiple_validation_rules() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer with multiple issues
    let data = vec![0.0f32, f32::NAN, 1.5, 2.0]; // NaN and out of range
    let capacity = 100; // Also low utilization
    let buffer_size = (capacity * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Apply multiple validation rules
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let rules: Vec<Box<dyn gup::debug::ValidationRule<f32>>> = vec![
        Box::new(FiniteValueRule),
        Box::new(RangeValidationRule::new(0.0, 1.0, "test")),
        Box::new(UtilizationValidationRule::new(50.0)),
    ];

    let report = inspector
        .validate_buffer::<f32>(&buffer, capacity, rules)
        .await
        .unwrap();

    // Should detect all three issues
    assert!(report.has_failures());
    assert!(report.has_errors()); // From FiniteValueRule
    assert!(report.has_warnings()); // From range and utilization
    assert_eq!(report.results.len(), 3);

    // Print the report for visual inspection during test
    println!("{}", report.format_report());
}

#[tokio::test]
async fn test_statistical_summary() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer with known statistical properties
    let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
    let buffer_size = (data.len() * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Create statistical summary
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let summary = inspector
        .create_statistical_summary::<f32>(&buffer)
        .await
        .unwrap();

    println!("{}", summary);

    // Verify it contains expected statistics
    assert!(summary.contains("Element count: 5"));
    assert!(summary.contains("Min: 1.0"));
    assert!(summary.contains("Max: 5.0"));
    assert!(summary.contains("Mean: 3.0")); // Should be exactly 3.0
}

#[tokio::test]
async fn test_buffer_comparison() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create two similar buffers
    let data_a = vec![1.0f32, 2.0, 3.0, 4.0];
    let data_b = vec![1.0f32, 2.0, 3.0, 5.0]; // Last value different

    let buffer_a = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer_a"),
        size: (data_a.len() * std::mem::size_of::<f32>()) as u64,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let buffer_b = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer_b"),
        size: (data_b.len() * std::mem::size_of::<f32>()) as u64,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer_a, 0, bytemuck::cast_slice(&data_a));
    context
        .queue
        .write_buffer(&buffer_b, 0, bytemuck::cast_slice(&data_b));

    // Compare buffers
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let comparison = inspector
        .compare_buffers::<f32>(&buffer_a, &buffer_b, 0.0)
        .await
        .unwrap();

    // Should detect one difference at index 3
    assert!(!comparison.is_identical);
    assert_eq!(comparison.differences.len(), 1);
    assert_eq!(comparison.differences[0].index, 3);
    assert_eq!(comparison.similarity_percentage, 75.0); // 3 out of 4 match
}

#[tokio::test]
async fn test_buffer_analysis() {
    let context = Arc::new(GupContext::new().await.unwrap());

    // Create a buffer with anomalies
    let data = vec![1.0f32, 2.0, f32::NAN, f32::INFINITY, 0.0];
    let buffer_size = (data.len() * std::mem::size_of::<f32>()) as u64;

    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    context
        .queue
        .write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

    // Analyze buffer
    let mut inspector = GpuBufferInspector::new(&context.device, &context.queue);
    let analysis = inspector.analyze_buffer::<f32>(&buffer).await.unwrap();

    // Should detect anomalies
    assert_eq!(analysis.element_count, 5);
    assert!(analysis.has_nan_values);
    assert!(analysis.has_infinite_values);
    assert!(analysis.has_zero_values);
    assert!(!analysis.anomalies.is_empty());
}
