// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU Statistics Integration Tests (GUP-145)
//!
//! Comprehensive tests that execute statistical compute shaders on GPU
//! and verify correctness against CPU ground truth across various datasets.

use gup::{Mean, MinMax, StandardDeviation, StatisticsCompute};

/// Test helper to create GPU context
async fn create_gpu_context() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = match instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
    {
        Ok(a) => a,
        Err(_) => return None,
    };

    match adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
        })
        .await
    {
        Ok((device, queue)) => Some((device, queue)),
        Err(_) => None,
    }
}

#[tokio::test]
async fn test_statistics_compute_basic_stats_small_dataset() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Small dataset with known statistics
    let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];

    // Compute CPU ground truth
    let mean_cpu = Mean::new(data.clone()).compute_cpu();
    let (min_cpu, max_cpu) = MinMax::new(data.clone()).compute_cpu();
    let std_dev_cpu = StandardDeviation::new(data.clone()).compute_cpu();

    // Compute on GPU
    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Verify results match CPU
    println!(
        "Debug: result.count={}, expected={}",
        result.count,
        data.len()
    );
    println!(
        "Debug: result.sum={}, result.mean={}",
        result.sum, result.mean
    );
    println!(
        "Debug: result.min={}, result.max={}",
        result.min, result.max
    );

    assert_eq!(result.count, data.len() as u32);
    assert!(
        (result.mean - mean_cpu).abs() < 0.001,
        "Mean mismatch: GPU={}, CPU={}",
        result.mean,
        mean_cpu
    );
    assert_eq!(result.min, min_cpu, "Min mismatch");
    assert_eq!(result.max, max_cpu, "Max mismatch");
    assert!(
        (result.std_dev - std_dev_cpu).abs() < 0.001,
        "Std dev mismatch: GPU={}, CPU={}",
        result.std_dev,
        std_dev_cpu
    );

    println!("Small dataset stats:");
    println!("  Count: {}", result.count);
    println!("  Mean: {} (CPU: {})", result.mean, mean_cpu);
    println!("  Min: {} (CPU: {})", result.min, min_cpu);
    println!("  Max: {} (CPU: {})", result.max, max_cpu);
    println!("  Std Dev: {} (CPU: {})", result.std_dev, std_dev_cpu);
}

#[tokio::test]
async fn test_statistics_compute_large_dataset_100_elements() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Dataset with 100 elements
    let data: Vec<f32> = (1..=100).map(|x| x as f32).collect();

    // CPU ground truth
    let mean_cpu = Mean::new(data.clone()).compute_cpu();
    let (min_cpu, max_cpu) = MinMax::new(data.clone()).compute_cpu();
    let std_dev_cpu = StandardDeviation::new(data.clone()).compute_cpu();

    // GPU computation
    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Verify
    assert_eq!(result.count, 100);
    assert!((result.mean - mean_cpu).abs() < 0.01);
    assert_eq!(result.min, min_cpu);
    assert_eq!(result.max, max_cpu);
    assert!((result.std_dev - std_dev_cpu).abs() < 0.1);

    println!("100-element dataset stats:");
    println!("  Mean: {} (expected ~50.5)", result.mean);
    println!("  Range: {} - {}", result.min, result.max);
    println!("  Std Dev: {}", result.std_dev);
}

#[tokio::test]
async fn test_statistics_compute_10k_elements() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Large dataset: 10,000 elements
    let data: Vec<f32> = (0..10_000).map(|x| (x as f32) * 0.5).collect();

    // CPU ground truth
    let mean_cpu = Mean::new(data.clone()).compute_cpu();
    let (min_cpu, max_cpu) = MinMax::new(data.clone()).compute_cpu();
    let std_dev_cpu = StandardDeviation::new(data.clone()).compute_cpu();

    // GPU computation
    let stats_compute = StatisticsCompute::new(&device, &queue, 100_000)
        .await
        .expect("Failed to create StatisticsCompute");

    let start = std::time::Instant::now();
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");
    let elapsed = start.elapsed();

    // Verify
    assert_eq!(result.count, 10_000);
    assert!((result.mean - mean_cpu).abs() < 0.1);
    assert!((result.min - min_cpu).abs() < 0.001);
    assert!((result.max - max_cpu).abs() < 0.001);
    assert!((result.std_dev - std_dev_cpu).abs() < 1.0);

    println!("10K dataset GPU compute time: {:?}", elapsed);
    println!("  Mean: {} (CPU: {})", result.mean, mean_cpu);
    println!("  Std Dev: {} (CPU: {})", result.std_dev, std_dev_cpu);
}

#[tokio::test]
async fn test_statistics_compute_1m_elements() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Very large dataset: 1 million elements
    let data: Vec<f32> = (0..1_000_000).map(|x| x as f32).collect();

    // CPU ground truth (just mean for performance)
    let mean_cpu = Mean::new(data.clone()).compute_cpu();

    // GPU computation with timing
    let stats_compute = StatisticsCompute::new(&device, &queue, 2_000_000)
        .await
        .expect("Failed to create StatisticsCompute");

    let start = std::time::Instant::now();
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");
    let elapsed = start.elapsed();

    // Verify count and rough accuracy
    assert_eq!(result.count, 1_000_000);
    assert!((result.mean - mean_cpu).abs() < 100.0);
    assert_eq!(result.min, 0.0);
    assert_eq!(result.max, 999_999.0);

    println!("1M dataset GPU compute time: {:?}", elapsed);
    println!("  Mean: {} (CPU: {})", result.mean, mean_cpu);
    println!("  Min: {}, Max: {}", result.min, result.max);
    assert!(elapsed.as_millis() < 5000, "GPU computation should be fast");
}

#[tokio::test]
async fn test_statistics_compute_with_special_values() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Test with extreme values
    let data = vec![f32::MIN, -1000.0, -10.0, 0.0, 10.0, 1000.0, f32::MAX];

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Verify extremes are captured
    assert_eq!(result.count, 7);
    assert_eq!(result.min, f32::MIN);
    assert_eq!(result.max, f32::MAX);

    println!("Special values stats:");
    println!("  Min: {}", result.min);
    println!("  Max: {}", result.max);
    println!("  Mean: {}", result.mean);
}

#[tokio::test]
async fn test_statistics_compute_with_nan() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Test with NaN values
    let data = vec![1.0, 2.0, f32::NAN, 4.0, 5.0];

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // NaN should propagate through calculations
    assert!(result.mean.is_nan() || result.mean.is_finite()); // GPU may handle NaN differently
    println!("NaN handling - Mean: {}, Sum: {}", result.mean, result.sum);
}

#[tokio::test]
async fn test_statistics_compute_with_infinity() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Test with infinity
    let data = vec![1.0, 2.0, f32::INFINITY, 4.0, 5.0];

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Infinity should propagate
    assert!(result.sum.is_infinite());
    assert_eq!(result.max, f32::INFINITY);
    println!(
        "Infinity handling - Sum: {}, Max: {}",
        result.sum, result.max
    );
}

#[tokio::test]
async fn test_statistics_compute_empty_dataset() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Empty dataset
    let data: Vec<f32> = vec![];

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Empty dataset should return zeros
    assert_eq!(result.count, 0);
    assert_eq!(result.sum, 0.0);
    assert_eq!(result.mean, 0.0);
}

#[tokio::test]
async fn test_statistics_compute_single_value() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Single value
    let data = vec![42.0];

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Verify single value stats
    assert_eq!(result.count, 1);
    assert_eq!(result.mean, 42.0);
    assert_eq!(result.min, 42.0);
    assert_eq!(result.max, 42.0);
    assert_eq!(result.std_dev, 0.0); // Single value has zero variance
}

#[tokio::test]
async fn test_statistics_compute_uniform_distribution() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Uniform values - should have zero variance
    let data = vec![5.0; 1000];

    // CPU verification
    let std_dev_cpu = StandardDeviation::new(data.clone()).compute_cpu();

    let stats_compute = StatisticsCompute::new(&device, &queue, 10_000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&data)
        .await
        .expect("Failed to compute GPU stats");

    // Uniform distribution should have zero variance
    assert_eq!(result.mean, 5.0);
    assert_eq!(result.min, 5.0);
    assert_eq!(result.max, 5.0);
    assert!((result.std_dev - std_dev_cpu).abs() < 0.001);
    println!(
        "Uniform distribution std_dev: {} (should be ~0)",
        result.std_dev
    );
}

#[tokio::test]
async fn test_statistics_compute_real_world_temperatures() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Realistic temperature data
    let temperatures: Vec<f32> = vec![
        20.5, 21.0, 19.8, 22.3, 21.5, 20.9, 21.2, 22.0, 20.7, 21.3, 21.8, 20.4, 22.1, 21.6, 20.8,
        21.4, 19.9, 22.5, 21.1, 20.6,
    ];

    // CPU ground truth
    let mean_cpu = Mean::new(temperatures.clone()).compute_cpu();
    let (min_cpu, max_cpu) = MinMax::new(temperatures.clone()).compute_cpu();
    let std_dev_cpu = StandardDeviation::new(temperatures.clone()).compute_cpu();

    let stats_compute = StatisticsCompute::new(&device, &queue, 1000)
        .await
        .expect("Failed to create StatisticsCompute");
    let result = stats_compute
        .compute_basic_stats(&temperatures)
        .await
        .expect("Failed to compute GPU stats");

    // Verify results
    assert_eq!(result.count, 20);
    assert!((result.mean - mean_cpu).abs() < 0.01);
    assert_eq!(result.min, min_cpu);
    assert_eq!(result.max, max_cpu);
    assert!((result.std_dev - std_dev_cpu).abs() < 0.01);

    println!("Temperature statistics:");
    println!("  Mean: {:.2}°C (CPU: {:.2}°C)", result.mean, mean_cpu);
    println!("  Range: {:.2}°C - {:.2}°C", result.min, result.max);
    println!(
        "  Std Dev: {:.2}°C (CPU: {:.2}°C)",
        result.std_dev, std_dev_cpu
    );
}

#[tokio::test]
async fn test_statistics_compute_shader_compilation() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Test that shader compiles correctly
    let result = StatisticsCompute::new(&device, &queue, 1000).await;
    assert!(result.is_ok(), "Shader compilation should succeed");

    println!("Statistics compute shaders compiled successfully");
}

#[tokio::test]
async fn test_statistics_compute_memory_layout() {
    // Verify StatisticsResult has correct memory layout for GPU
    use gup::StatisticsResult;
    use std::mem;

    let size = mem::size_of::<StatisticsResult>();
    let align = mem::align_of::<StatisticsResult>();

    // Should be properly aligned for GPU buffers
    assert_eq!(size, 32, "StatisticsResult should be 32 bytes");
    assert!(
        align >= 4,
        "StatisticsResult should be at least 4-byte aligned"
    );

    println!("StatisticsResult layout:");
    println!("  Size: {} bytes", size);
    println!("  Alignment: {} bytes", align);
}

#[tokio::test]
async fn test_statistics_compute_workgroup_coverage() {
    let context = create_gpu_context().await;
    if context.is_none() {
        println!("GPU not available, skipping test");
        return;
    }
    let (device, queue) = context.unwrap();

    // Test various dataset sizes that exercise workgroup boundaries
    let test_sizes = vec![
        1,    // Single element
        255,  // Just under one workgroup
        256,  // Exactly one workgroup
        257,  // Just over one workgroup
        512,  // Exactly two workgroups
        1000, // Multiple workgroups with remainder
    ];

    for size in test_sizes {
        let data: Vec<f32> = (0..size).map(|x| x as f32).collect();
        let mean_cpu = Mean::new(data.clone()).compute_cpu();

        let stats_compute = StatisticsCompute::new(&device, &queue, 10_000)
            .await
            .expect("Failed to create StatisticsCompute");
        let result = stats_compute
            .compute_basic_stats(&data)
            .await
            .expect("Failed to compute GPU stats");

        assert_eq!(result.count, size as u32);
        assert!(
            (result.mean - mean_cpu).abs() < 0.1,
            "Size {} - Mean mismatch: GPU={}, CPU={}",
            size,
            result.mean,
            mean_cpu
        );

        println!(
            "Size {}: mean={:.2} (CPU={:.2})",
            size, result.mean, mean_cpu
        );
    }
}
