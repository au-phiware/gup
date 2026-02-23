// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Kernel Density Estimation tests (GUP-144)
//!
//! This test file verifies that KDE functions compute correct density estimates
//! for various distributions and kernel functions.

use gup::{BandwidthMethod, KernelDensity1D, KernelDensity2D, KernelFunction};

/// Test that KDE types are accessible
#[test]
fn test_kde_types_exist() {
    // Verify all KDE types can be created
    let _kde_1d = KernelDensity1D::new(vec![1.0, 2.0, 3.0]);
    let _kde_2d = KernelDensity2D::new(vec![(1.0, 2.0), (3.0, 4.0)]);
}

/// Test kernel function evaluation
#[test]
fn test_kernel_functions() {
    // Test Gaussian kernel at standard points
    let gaussian = KernelFunction::Gaussian;
    let at_zero = gaussian.evaluate(0.0);
    // At u=0, Gaussian should be 1/sqrt(2π) ≈ 0.3989
    assert!(
        (at_zero - 0.3989).abs() < 0.01,
        "Gaussian at 0: {}",
        at_zero
    );

    // Test Epanechnikov kernel
    let epan = KernelFunction::Epanechnikov;
    assert_eq!(epan.evaluate(0.0), 0.75); // At u=0
    assert_eq!(epan.evaluate(1.5), 0.0); // Outside support

    // Test Uniform kernel
    let uniform = KernelFunction::Uniform;
    assert_eq!(uniform.evaluate(0.0), 0.5);
    assert_eq!(uniform.evaluate(0.5), 0.5);
    assert_eq!(uniform.evaluate(1.5), 0.0);

    // Test Triangular kernel
    let triangular = KernelFunction::Triangular;
    assert_eq!(triangular.evaluate(0.0), 1.0);
    assert_eq!(triangular.evaluate(0.5), 0.5);
    assert_eq!(triangular.evaluate(1.5), 0.0);
}

/// Test 1D KDE with normal distribution
#[test]
fn test_kde_1d_normal_distribution() {
    // Generate samples roughly centered around 0
    let samples: Vec<f32> = vec![
        -2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, -1.8, -1.2, -0.8, -0.2, 0.2, 0.8, 1.2, 1.8,
    ];

    let kde = KernelDensity1D::new(samples)
        .with_kernel(KernelFunction::Gaussian)
        .with_bandwidth(0.5)
        .with_n_eval_points(100);

    let result = kde.compute_cpu();

    // Check that we got results
    assert_eq!(result.eval_points.len(), 100);
    assert_eq!(result.densities.len(), 100);
    assert_eq!(result.bandwidth, 0.5);

    // Density should be positive
    assert!(
        result.densities.iter().all(|&d| d >= 0.0),
        "All densities should be non-negative"
    );

    // Peak should be somewhere reasonable (not testing exact location for symmetric-ish data)
    let mode = result.mode().unwrap();
    assert!(
        mode.abs() < 2.0,
        "Mode should be within data range, got {}",
        mode
    );
}

/// Test 1D KDE with uniform distribution
#[test]
fn test_kde_1d_uniform_distribution() {
    // Generate samples from uniform distribution [0, 10]
    let samples: Vec<f32> = (0..20).map(|i| i as f32 * 0.5).collect();

    let kde = KernelDensity1D::new(samples)
        .with_kernel(KernelFunction::Epanechnikov)
        .with_bandwidth(1.0);

    let result = kde.compute_cpu();

    // Density should be relatively flat in the middle
    assert!(!result.densities.is_empty());
    assert!(result.densities.iter().all(|&d| d >= 0.0));
}

/// Test 1D KDE with bimodal distribution
#[test]
fn test_kde_1d_bimodal_distribution() {
    // Create bimodal distribution: cluster at -5 and +5
    let mut samples = Vec::new();
    for _ in 0..10 {
        samples.push(-5.0);
        samples.push(-4.8);
        samples.push(-5.2);
        samples.push(5.0);
        samples.push(4.8);
        samples.push(5.2);
    }

    let kde = KernelDensity1D::new(samples)
        .with_kernel(KernelFunction::Gaussian)
        .with_bandwidth(0.5)
        .with_n_eval_points(200);

    let result = kde.compute_cpu();

    // Should have low density in the middle
    let middle_idx = result.eval_points.len() / 2;
    let middle_density = result.densities[middle_idx];
    let peak_density = result.peak_density();

    assert!(
        middle_density < peak_density * 0.5,
        "Middle density should be much lower than peak for bimodal distribution"
    );
}

/// Test bandwidth estimation with Silverman's rule
#[test]
fn test_silverman_bandwidth() {
    let samples: Vec<f32> = (1..=100).map(|x| x as f32).collect();

    let kde = KernelDensity1D::new(samples)
        .with_bandwidth_method(BandwidthMethod::Silverman)
        .with_n_eval_points(50);

    let result = kde.compute_cpu();

    // Bandwidth should be positive and reasonable
    assert!(
        result.bandwidth > 0.0,
        "Bandwidth should be positive: {}",
        result.bandwidth
    );
    assert!(
        result.bandwidth < 50.0,
        "Bandwidth should be reasonable: {}",
        result.bandwidth
    );
}

/// Test bandwidth estimation with Scott's rule
#[test]
fn test_scott_bandwidth() {
    let samples: Vec<f32> = (1..=100).map(|x| x as f32).collect();

    let kde = KernelDensity1D::new(samples)
        .with_bandwidth_method(BandwidthMethod::Scott)
        .with_n_eval_points(50);

    let result = kde.compute_cpu();

    // Bandwidth should be positive and reasonable
    assert!(
        result.bandwidth > 0.0,
        "Bandwidth should be positive: {}",
        result.bandwidth
    );
}

/// Test manual bandwidth setting
#[test]
fn test_manual_bandwidth() {
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let manual_bw = 2.5;

    let kde = KernelDensity1D::new(samples).with_bandwidth(manual_bw);

    let result = kde.compute_cpu();

    assert_eq!(result.bandwidth, manual_bw);
}

/// Test custom evaluation points
#[test]
fn test_custom_eval_points() {
    let samples = vec![1.0, 2.0, 3.0];
    let custom_points = vec![0.0, 1.0, 2.0, 3.0, 4.0];

    let kde = KernelDensity1D::new(samples)
        .with_bandwidth(0.5)
        .with_eval_points(custom_points.clone());

    let result = kde.compute_cpu();

    assert_eq!(result.eval_points, custom_points);
    assert_eq!(result.densities.len(), custom_points.len());
}

/// Test 2D KDE with simple distribution
#[test]
fn test_kde_2d_simple() {
    // Simple 2D samples
    let samples = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (0.5, 0.5), (1.5, 1.5)];

    let kde = KernelDensity2D::new(samples)
        .with_kernel(KernelFunction::Gaussian)
        .with_bandwidth(0.5)
        .with_n_eval_points(20); // 20x20 = 400 points

    let result = kde.compute_cpu();

    // Check dimensions
    assert_eq!(result.x_points.len(), 20);
    assert_eq!(result.y_points.len(), 20);
    assert_eq!(result.densities.len(), 400); // 20 * 20

    // All densities should be non-negative
    assert!(result.densities.iter().all(|&d| d >= 0.0));

    // Should have a mode
    let mode = result.mode();
    assert!(mode.is_some());
}

/// Test 2D KDE with separate bandwidths
#[test]
fn test_kde_2d_separate_bandwidths() {
    let samples = vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]; // Horizontal line

    let kde = KernelDensity2D::new(samples)
        .with_bandwidths(0.5, 1.0) // Different bandwidth for x and y
        .with_n_eval_points(10);

    let result = kde.compute_cpu();

    assert_eq!(result.bandwidth_x, 0.5);
    assert_eq!(result.bandwidth_y, 1.0);
}

/// Test 2D KDE density_at accessor
#[test]
fn test_kde_2d_density_at() {
    let samples = vec![(0.0, 0.0), (1.0, 1.0)];

    let kde = KernelDensity2D::new(samples)
        .with_bandwidth(1.0)
        .with_n_eval_points(10);

    let result = kde.compute_cpu();

    // Test valid access
    let density = result.density_at(0, 0);
    assert!(density.is_some());
    assert!(density.unwrap() >= 0.0);

    // Test invalid access
    assert!(result.density_at(100, 100).is_none());
}

/// Test KDE with empty data
#[test]
fn test_kde_empty_data() {
    let kde_1d = KernelDensity1D::new(vec![]);
    let result_1d = kde_1d.compute_cpu();
    assert_eq!(result_1d.densities.len(), 0);
    assert_eq!(result_1d.eval_points.len(), 0);

    let kde_2d = KernelDensity2D::new(vec![]);
    let result_2d = kde_2d.compute_cpu();
    assert_eq!(result_2d.densities.len(), 0);
}

/// Test KDE with single sample
#[test]
fn test_kde_single_sample() {
    let kde = KernelDensity1D::new(vec![5.0])
        .with_bandwidth(1.0)
        .with_n_eval_points(50);

    let result = kde.compute_cpu();

    // Should have peak at the sample location
    let mode = result.mode().unwrap();
    assert!((mode - 5.0).abs() < 1.0);
}

/// Test KDE result utility methods
#[test]
fn test_kde_result_methods() {
    let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let kde = KernelDensity1D::new(samples).with_bandwidth(0.5);
    let result = kde.compute_cpu();

    // Test peak_density
    let peak = result.peak_density();
    assert!(peak > 0.0);
    assert!(result.densities.iter().all(|&d| d <= peak));

    // Test mode
    let mode = result.mode();
    assert!(mode.is_some());
}

/// Performance test: KDE should complete for moderate dataset
#[test]
fn test_kde_performance_moderate_dataset() {
    use std::time::Instant;

    // 1000 samples
    let samples: Vec<f32> = (0..1000).map(|i| (i as f32) * 0.01).collect();

    let start = Instant::now();
    let kde = KernelDensity1D::new(samples)
        .with_bandwidth(0.1)
        .with_n_eval_points(1000); // 1M kernel evaluations

    let result = kde.compute_cpu();
    let duration = start.elapsed();

    // Should complete in reasonable time (< 5 seconds on CPU)
    assert!(duration.as_secs() < 5, "KDE took too long: {:?}", duration);
    assert_eq!(result.densities.len(), 1000);

    println!(
        "1000 samples × 1000 eval points = 1M evaluations in {:?}",
        duration
    );
}
