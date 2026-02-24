// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for recovery metrics and analytics.

use gup::{GupContext, GupOptions, RecoveryMetrics, RecoveryTier};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_initial_metrics() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let context = Arc::try_unwrap(context).unwrap();

    let metrics = context.recovery_metrics();

    assert_eq!(metrics.total_attempts, 0);
    assert_eq!(metrics.successful_recoveries, 0);
    assert_eq!(metrics.failed_recoveries, 0);
    assert_eq!(metrics.success_rate(), 0.0);
    assert!(metrics.average_recovery_time().is_none());
    assert!(metrics.min_recovery_time.is_none());
    assert!(metrics.max_recovery_time.is_none());
}

#[tokio::test]
async fn test_single_recovery_metrics() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Trigger recovery
    context.mark_device_lost();
    let result = context.attempt_recovery().await.unwrap();
    assert!(result.success);

    let metrics = context.recovery_metrics();

    assert_eq!(metrics.total_attempts, 1);
    assert_eq!(metrics.successful_recoveries, 1);
    assert_eq!(metrics.failed_recoveries, 0);
    assert_eq!(metrics.success_rate(), 100.0);
    assert!(metrics.average_recovery_time().is_some());
    assert!(metrics.min_recovery_time.is_some());
    assert!(metrics.max_recovery_time.is_some());
    assert_eq!(metrics.full_features_count, 1);
    assert_eq!(metrics.recent_attempts.len(), 1);
}

#[tokio::test]
async fn test_multiple_recovery_metrics() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform multiple recoveries
    for _ in 0..5 {
        context.mark_device_lost();
        let result = context.attempt_recovery().await.unwrap();
        assert!(result.success);
    }

    let metrics = context.recovery_metrics();

    assert_eq!(metrics.total_attempts, 5);
    assert_eq!(metrics.successful_recoveries, 5);
    assert_eq!(metrics.failed_recoveries, 0);
    assert_eq!(metrics.success_rate(), 100.0);
    assert_eq!(metrics.full_features_count, 5);
    assert_eq!(metrics.recent_attempts.len(), 5);
}

#[tokio::test]
async fn test_recovery_timing_statistics() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform multiple recoveries to get timing data
    for _ in 0..10 {
        context.mark_device_lost();
        let _result = context.attempt_recovery().await.unwrap();
    }

    let metrics = context.recovery_metrics();

    // Check that timing statistics are populated
    assert!(metrics.min_recovery_time.is_some());
    assert!(metrics.max_recovery_time.is_some());
    assert!(metrics.average_recovery_time().is_some());

    let min = metrics.min_recovery_time.unwrap();
    let max = metrics.max_recovery_time.unwrap();
    let avg = metrics.average_recovery_time().unwrap();

    // Min should be <= avg <= max
    assert!(min <= avg);
    assert!(avg <= max);

    // All recoveries should be reasonably fast (< 2 seconds as per AC)
    assert!(max < Duration::from_secs(2));
}

#[tokio::test]
async fn test_recovery_tier_tracking() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform recovery (should use full features)
    context.mark_device_lost();
    let _result = context.attempt_recovery().await.unwrap();

    let metrics = context.recovery_metrics();

    // Should have recorded full features recovery
    assert_eq!(metrics.full_features_count, 1);
    assert_eq!(metrics.reduced_features_count, 0);
    assert_eq!(metrics.software_rendering_count, 0);
}

#[tokio::test]
async fn test_rolling_window_limit() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform more than 100 recoveries to test rolling window
    for _ in 0..105 {
        context.mark_device_lost();
        let _result = context.attempt_recovery().await.unwrap();
    }

    let metrics = context.recovery_metrics();

    assert_eq!(metrics.total_attempts, 105);
    assert_eq!(metrics.successful_recoveries, 105);
    // Recent attempts should be capped at 100
    assert_eq!(metrics.recent_attempts.len(), 100);
}

#[tokio::test]
async fn test_json_export() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform a recovery
    context.mark_device_lost();
    let _result = context.attempt_recovery().await.unwrap();

    let metrics = context.recovery_metrics();
    let json = metrics.to_json();

    // Verify JSON contains expected fields
    assert!(json.contains("total_attempts"));
    assert!(json.contains("successful_recoveries"));
    assert!(json.contains("failed_recoveries"));
    assert!(json.contains("success_rate"));
    assert!(json.contains("min_recovery_time_ms"));
    assert!(json.contains("max_recovery_time_ms"));
    assert!(json.contains("avg_recovery_time_ms"));
    assert!(json.contains("full_features_count"));
    assert!(json.contains("reduced_features_count"));
    assert!(json.contains("software_rendering_count"));

    // Verify it's valid-looking JSON
    assert!(json.starts_with('{'));
    assert!(json.ends_with('}'));
}

#[tokio::test]
async fn test_csv_export() {
    let options = GupOptions {
        automatic_device_loss_detection: false,
        ..Default::default()
    };

    let context = GupContext::with_options(options).await.unwrap();
    let mut context = Arc::try_unwrap(context).unwrap();

    // Perform a recovery
    context.mark_device_lost();
    let _result = context.attempt_recovery().await.unwrap();

    let metrics = context.recovery_metrics();
    let csv = metrics.to_csv();

    // Verify CSV format
    assert!(csv.starts_with("metric,value\n"));
    assert!(csv.contains("total_attempts,"));
    assert!(csv.contains("successful_recoveries,"));
    assert!(csv.contains("failed_recoveries,"));
    assert!(csv.contains("success_rate,"));
    assert!(csv.contains("full_features_count,"));
}

#[tokio::test]
async fn test_recovery_metrics_default() {
    let metrics = RecoveryMetrics::default();

    assert_eq!(metrics.total_attempts, 0);
    assert_eq!(metrics.successful_recoveries, 0);
    assert_eq!(metrics.failed_recoveries, 0);
    assert_eq!(metrics.full_features_count, 0);
    assert_eq!(metrics.reduced_features_count, 0);
    assert_eq!(metrics.software_rendering_count, 0);
    assert!(metrics.recent_attempts.is_empty());
    assert_eq!(metrics.success_rate(), 0.0);
    assert!(metrics.average_recovery_time().is_none());
}

#[test]
fn test_recovery_tier_enum() {
    // Test that RecoveryTier enum values exist
    let _ = RecoveryTier::FullFeatures;
    let _ = RecoveryTier::ReducedFeatures;
    let _ = RecoveryTier::SoftwareRendering;

    // Test PartialEq
    assert_eq!(RecoveryTier::FullFeatures, RecoveryTier::FullFeatures);
    assert_ne!(RecoveryTier::FullFeatures, RecoveryTier::ReducedFeatures);
}
