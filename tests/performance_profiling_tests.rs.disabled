// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance profiling integration tests.
//!
//! These tests verify that the advanced performance profiling system works correctly,
//! including GPU timestamp queries, frame statistics, and regression detection.

use gup::context::GupContext;
use gup::performance::{
    AggregateStats, DetailedFrameStats, PerformanceAlert, PerformanceProfiler, ProfilingConfig,
    RenderPassTiming,
};
use std::time::Duration;

#[tokio::test]
async fn test_profiling_enable_disable() {
    let mut context = GupContext::new().await.expect("Failed to create context");

    // Should not be enabled initially
    assert!(!context.is_profiling_enabled());
    assert!(context.profiler().is_none());

    // Enable profiling
    let config = ProfilingConfig::default();
    context
        .enable_profiling(config)
        .expect("Failed to enable profiling");

    assert!(context.is_profiling_enabled());
    assert!(context.profiler().is_some());

    // Disable profiling
    context.disable_profiling();
    assert!(!context.is_profiling_enabled());
    assert!(context.profiler().is_none());
}

#[tokio::test]
async fn test_profiling_duplicate_enable() {
    let mut context = GupContext::new().await.expect("Failed to create context");

    context
        .enable_profiling(ProfilingConfig::default())
        .expect("Failed to enable profiling");

    // Should fail to enable again
    let result = context.enable_profiling(ProfilingConfig::default());
    assert!(result.is_err());
}

#[tokio::test]
async fn test_detailed_frame_stats() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false, // Disable GPU timing for this test
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Begin a frame
    profiler.begin_frame();

    // Record some render pass timings
    profiler.record_render_pass(RenderPassTiming {
        label: Some("test_pass".to_string()),
        cpu_time: Duration::from_millis(10),
        gpu_time: None,
        draw_calls: 5,
    });

    profiler.record_buffer_upload(Duration::from_millis(2));
    profiler.record_pipeline_switch();
    profiler.record_pipeline_switch();
    profiler.record_compute_dispatch();

    // End the frame
    profiler.end_frame(Duration::from_millis(16));

    // Check stats
    let current = profiler.current_frame();
    assert_eq!(current.cpu_time, Duration::from_millis(16));
    assert_eq!(current.render_pass_times.len(), 1);
    assert_eq!(current.buffer_upload_time, Duration::from_millis(2));
    assert_eq!(current.pipeline_switches, 2);
    assert_eq!(current.draw_calls, 5);
    assert_eq!(current.compute_dispatches, 1);
}

#[tokio::test]
async fn test_profiler_history() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 10,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record several frames
    for i in 0..15 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10 + i),
            gpu_time: None,
            draw_calls: i as u32,
        });
        profiler.end_frame(Duration::from_millis(16 + i));
    }

    // History should be limited to 10
    let history = profiler.history();
    assert_eq!(history.len(), 10);

    // Most recent frame should be the last one recorded
    let last_frame = history.back().unwrap();
    assert_eq!(last_frame.draw_calls, 14);
}

#[tokio::test]
async fn test_aggregate_stats() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 100,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record frames with varying timings
    for i in 0..50 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10 + (i % 5)),
            gpu_time: None,
            draw_calls: 10 + (i % 3) as u32,
        });
        profiler.end_frame(Duration::from_millis(16 + (i % 5)));
    }

    let stats = profiler.aggregate_stats();
    assert_eq!(stats.frame_count, 50);
    assert!(stats.avg_cpu_time.as_millis() >= 16);
    assert!(stats.avg_cpu_time.as_millis() <= 20);
    assert!(stats.min_frame_time < stats.avg_cpu_time);
    assert!(stats.max_frame_time > stats.avg_cpu_time);
    assert!(stats.p95_frame_time >= stats.avg_cpu_time);
    assert!(stats.p99_frame_time >= stats.p95_frame_time);
    assert!(stats.avg_draw_calls >= 10.0 && stats.avg_draw_calls <= 12.0);
}

#[tokio::test]
async fn test_performance_baseline() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 100,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record some frames
    for i in 0..30 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(16));
    }

    // Record a baseline
    profiler.record_baseline("initial");
    let baselines = profiler.baselines();
    assert_eq!(baselines.len(), 1);
    assert_eq!(baselines[0].label, "initial");
    assert_eq!(baselines[0].stats.frame_count, 30);

    // Record more frames
    for i in 0..20 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(12),
            gpu_time: None,
            draw_calls: 15,
        });
        profiler.end_frame(Duration::from_millis(18));
    }

    // Record another baseline
    profiler.record_baseline("after_changes");
    let baselines = profiler.baselines();
    assert_eq!(baselines.len(), 2);
    assert_eq!(baselines[1].label, "after_changes");
}

#[tokio::test]
async fn test_regression_detection() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 100,
            enable_regression_detection: true,
            regression_threshold_percent: 20.0,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record good performance baseline
    for i in 0..40 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(10));
    }

    profiler.record_baseline("good");
    profiler.clear_alerts();

    // Simulate performance regression (>20% slower)
    for i in 0..40 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(15),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(15));
    }

    // Alerts should be generated
    let alerts = profiler.alerts();
    assert!(!alerts.is_empty());

    // Check for frame time regression alert
    let has_regression = alerts.iter().any(|alert| {
        matches!(
            alert,
            PerformanceAlert::FrameTimeRegression { percent_increase, .. }
            if *percent_increase > 20.0
        )
    });
    assert!(has_regression);
}

#[tokio::test]
async fn test_draw_call_spike_detection() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 100,
            enable_regression_detection: true,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record baseline with low draw calls
    for i in 0..40 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(10));
    }

    profiler.record_baseline("low_draws");
    profiler.clear_alerts();

    // Simulate draw call spike (>1.5x)
    for i in 0..40 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 20, // 2x the baseline
        });
        profiler.end_frame(Duration::from_millis(10));
    }

    // Alerts should be generated
    let alerts = profiler.alerts();
    let has_draw_spike = alerts
        .iter()
        .any(|alert| matches!(alert, PerformanceAlert::DrawCallSpike { .. }));
    assert!(has_draw_spike);
}

#[tokio::test]
async fn test_excessive_pipeline_switches() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 100,
            enable_regression_detection: true,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record baseline
    for i in 0..40 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(10));
    }

    profiler.record_baseline("few_switches");
    profiler.clear_alerts();

    // Simulate excessive pipeline switches (>50 per frame)
    for i in 0..40 {
        profiler.begin_frame();
        for _ in 0..60 {
            profiler.record_pipeline_switch();
        }
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(10));
    }

    // Alerts should be generated
    let alerts = profiler.alerts();
    let has_pipeline_alert = alerts
        .iter()
        .any(|alert| matches!(alert, PerformanceAlert::ExcessivePipelineSwitches { .. }));
    assert!(has_pipeline_alert);
}

#[tokio::test]
async fn test_clear_history() {
    let context = GupContext::new().await.expect("Failed to create context");
    let mut profiler = PerformanceProfiler::new(
        &context.device,
        ProfilingConfig {
            enable_gpu_timing: false,
            ..Default::default()
        },
    )
    .expect("Failed to create profiler");

    // Record some frames
    for i in 0..10 {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some(format!("pass_{}", i)),
            cpu_time: Duration::from_millis(10),
            gpu_time: None,
            draw_calls: 10,
        });
        profiler.end_frame(Duration::from_millis(16));
    }

    assert_eq!(profiler.history().len(), 10);

    // Clear history
    profiler.clear_history();
    assert_eq!(profiler.history().len(), 0);
}

#[tokio::test]
async fn test_profiler_with_context() {
    let mut context = GupContext::new().await.expect("Failed to create context");

    // Enable profiling
    context
        .enable_profiling(ProfilingConfig {
            enable_gpu_timing: false,
            track_components: true,
            history_size: 50,
            enable_regression_detection: false,
            regression_threshold_percent: 20.0,
        })
        .expect("Failed to enable profiling");

    // Use the profiler
    if let Some(profiler) = context.profiler_mut() {
        profiler.begin_frame();
        profiler.record_render_pass(RenderPassTiming {
            label: Some("main_pass".to_string()),
            cpu_time: Duration::from_millis(8),
            gpu_time: None,
            draw_calls: 15,
        });
        profiler.end_frame(Duration::from_millis(16));
    }

    // Read stats
    if let Some(profiler) = context.profiler() {
        let current = profiler.current_frame();
        assert_eq!(current.draw_calls, 15);
        assert_eq!(current.cpu_time, Duration::from_millis(16));
    }
}
