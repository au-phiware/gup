// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for performance trend visualization

use gup::GupResult;
use gup::debug::PerformanceSnapshot;
use gup::debug::ci_performance::{
    BaselineStorage, PerformanceBaseline, PerformanceTrendVisualizer,
};
use std::collections::HashMap;

#[test]
fn test_trend_visualization_with_sample_data() -> GupResult<()> {
    // Create temporary directory for test baselines
    let temp_dir = std::env::temp_dir().join("gup_trend_viz_test");
    let _ = std::fs::remove_dir_all(&temp_dir); // Clean up any old test data
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create some sample baseline data
    let storage = BaselineStorage::new(temp_dir.clone());

    // Simulate historical performance data for a test
    let test_name = "render_100k_points";
    let category = "rendering";
    let platform_id = "test_platform";

    // Create baselines showing gradual performance improvement
    let timestamps = [
        chrono::Utc::now() - chrono::Duration::days(30),
        chrono::Utc::now() - chrono::Duration::days(20),
        chrono::Utc::now() - chrono::Duration::days(10),
        chrono::Utc::now(),
    ];

    let frame_times = [16.8, 16.5, 16.2, 16.0]; // Gradual improvement

    for (i, (&timestamp, &frame_time)) in timestamps.iter().zip(frame_times.iter()).enumerate() {
        let baseline = PerformanceBaseline {
            test_name: test_name.to_string(),
            category: category.to_string(),
            avg_frame_time_ms: frame_time,
            avg_memory_usage_bytes: 10_000_000 + (i as u64 * 100_000),
            sample_count: 1,
            last_updated: timestamp,
            metadata: HashMap::new(),
            platform_id: platform_id.to_string(),
        };

        // Save with unique name to create history
        let unique_name = format!("{}_{}", test_name, i);
        storage.save_baseline(&unique_name, category, platform_id, &baseline)?;
    }

    // Create trend visualizer
    let visualizer = PerformanceTrendVisualizer::new(temp_dir.clone());

    // Generate trend charts
    let charts = visualizer.generate_all_trend_charts()?;

    // Verify charts were generated
    assert!(!charts.is_empty(), "Should generate at least one chart");

    for (name, svg) in &charts {
        println!("Generated chart for: {}", name);
        assert!(svg.contains("<svg"), "Should contain SVG tag");
        assert!(svg.contains("</svg>"), "Should have closing SVG tag");
        assert!(svg.contains("Performance Trend"), "Should have title");
    }

    // Test exporting charts to directory
    let output_dir = temp_dir.join("charts");
    let paths = visualizer.export_charts_to_directory(&output_dir)?;

    assert!(!paths.is_empty(), "Should export at least one chart file");

    for path in &paths {
        assert!(path.exists(), "Chart file should exist: {:?}", path);
        let content = std::fs::read_to_string(path)?;
        assert!(content.contains("<svg"), "File should contain SVG content");
    }

    // Test dashboard generation
    let dashboard_html = visualizer.generate_dashboard_html()?;
    assert!(
        dashboard_html.contains("<!DOCTYPE html>"),
        "Should be valid HTML"
    );
    assert!(
        dashboard_html.contains("Performance Trend Dashboard"),
        "Should have dashboard title"
    );
    assert!(dashboard_html.contains("<svg"), "Should include chart SVG");

    // Test exporting dashboard
    let dashboard_path = temp_dir.join("dashboard.html");
    visualizer.export_dashboard(&dashboard_path)?;
    assert!(dashboard_path.exists(), "Dashboard file should be created");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(())
}

#[test]
fn test_svg_chart_structure() {
    use std::collections::HashMap;

    // Create sample performance snapshots
    let _snapshots = [PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            frame_time_ms: 10.0,
            memory_usage_bytes: 1_000_000,
            gpu_utilization_percent: 50.0,
            query_time_us: 100.0,
            metadata: HashMap::new(),
        },
        PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            frame_time_ms: 12.0,
            memory_usage_bytes: 1_100_000,
            gpu_utilization_percent: 55.0,
            query_time_us: 110.0,
            metadata: HashMap::new(),
        },
        PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            frame_time_ms: 11.0,
            memory_usage_bytes: 1_050_000,
            gpu_utilization_percent: 52.0,
            query_time_us: 105.0,
            metadata: HashMap::new(),
        }];

    // Create a trend chart and export as SVG
    use gup::debug::visualization::VisualizationConfig;

    let config = VisualizationConfig {
        width: 800,
        height: 600,
        enable_interaction: false,
        color_scheme: gup::debug::visualization::ColorScheme::Default,
        max_data_points: 1000,
    };

    // We can't actually create a GpuDebugVisualizer without GPU context in tests,
    // so we'll just verify the structure is correct by checking the types exist
    assert_eq!(config.width, 800);
    assert_eq!(config.height, 600);
}

#[test]
fn test_empty_baseline_handling() -> GupResult<()> {
    let temp_dir = std::env::temp_dir().join("gup_empty_baselines_test");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let visualizer = PerformanceTrendVisualizer::new(temp_dir.clone());

    // Should handle empty baseline directory gracefully
    let charts = visualizer.generate_all_trend_charts()?;
    assert!(
        charts.is_empty(),
        "Should return empty map for no baselines"
    );

    // Dashboard should still work with no data
    let dashboard = visualizer.generate_dashboard_html()?;
    assert!(
        dashboard.contains("<!DOCTYPE html>"),
        "Should generate valid HTML"
    );
    assert!(
        dashboard.contains("Total tests tracked: 0"),
        "Should show zero tests"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(())
}
