// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for profiling data export and visualization.

use gup::performance::{PerformanceProfiler, ProfilingConfig, RenderPassTiming};
use gup::performance_export::{
    DashboardConfig, DashboardGenerator, ExportConfig, FlameGraphConfig, FlameGraphGenerator,
    ProfileExporter,
};
use std::time::Duration;

/// Helper: create a device and a profiler seeded with realistic data.
fn setup_profiler() -> PerformanceProfiler {
    let device = pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("no adapter");
        let (device, _) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("no device");
        device
    });

    let config = ProfilingConfig {
        enable_gpu_timing: false,
        enable_regression_detection: true,
        history_size: 60,
        ..Default::default()
    };
    let mut profiler = PerformanceProfiler::new(&device, config).unwrap();

    // Simulate 20 frames with varying workloads.
    for i in 0..20 {
        profiler.begin_frame();

        // Main geometry pass
        profiler.record_render_pass(RenderPassTiming {
            label: Some("geometry".into()),
            cpu_time: Duration::from_micros(500 + i * 20),
            gpu_time: None,
            draw_calls: 10 + i as u32,
        });

        // Shadow pass (every other frame)
        if i % 2 == 0 {
            profiler.record_render_pass(RenderPassTiming {
                label: Some("shadow".into()),
                cpu_time: Duration::from_micros(300),
                gpu_time: None,
                draw_calls: 5,
            });
        }

        // Post-processing
        profiler.record_render_pass(RenderPassTiming {
            label: Some("post_process".into()),
            cpu_time: Duration::from_micros(200),
            gpu_time: None,
            draw_calls: 2,
        });

        profiler.record_buffer_upload(Duration::from_micros(30 + i * 5));
        profiler.record_pipeline_switch();
        if i % 3 == 0 {
            profiler.record_pipeline_switch();
        }
        profiler.end_frame(Duration::from_micros(1000 + i * 30));
    }

    // Record a baseline after the first batch
    profiler.record_baseline("initial");
    profiler
}

// -----------------------------------------------------------------------
// JSON round-trip
// -----------------------------------------------------------------------

#[test]
fn json_round_trip() {
    let profiler = setup_profiler();
    let exporter = ProfileExporter::new(&profiler);

    let json = exporter.to_json(&ExportConfig::default()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Aggregate section present
    assert!(parsed["aggregate"].is_object());
    assert_eq!(parsed["aggregate"]["frame_count"], 20);

    // Frames array present
    let frames = parsed["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 20);

    // First frame should have render passes
    let first_frame = &frames[0];
    let passes = first_frame["render_pass_times"].as_array().unwrap();
    assert!(passes.len() >= 2); // geometry + shadow + post_process for frame 0
}

// -----------------------------------------------------------------------
// CSV structure
// -----------------------------------------------------------------------

#[test]
fn csv_structure() {
    let profiler = setup_profiler();
    let exporter = ProfileExporter::new(&profiler);

    let csv_str = exporter.to_csv(&ExportConfig::default()).unwrap();
    let mut rdr = csv::Reader::from_reader(csv_str.as_bytes());
    let headers = rdr.headers().unwrap();
    assert_eq!(headers.get(0), Some("frame"));
    assert_eq!(headers.get(1), Some("cpu_time_ms"));

    let rows: Vec<_> = rdr.records().collect();
    assert_eq!(rows.len(), 20);
}

// -----------------------------------------------------------------------
// Chrome Trace validity
// -----------------------------------------------------------------------

#[test]
fn chrome_trace_valid_json() {
    let profiler = setup_profiler();
    let exporter = ProfileExporter::new(&profiler);

    let trace = exporter.to_chrome_trace(&ExportConfig::default()).unwrap();
    let events: Vec<serde_json::Value> = serde_json::from_str(&trace).unwrap();

    // Each event must have required fields
    for event in &events {
        assert!(event["name"].is_string());
        assert!(event["ph"].is_string());
        assert!(event["ts"].is_number());
    }

    // Should include render_pass events
    let pass_events: Vec<_> = events
        .iter()
        .filter(|e| e["cat"] == "render_pass")
        .collect();
    assert!(!pass_events.is_empty());
}

// -----------------------------------------------------------------------
// Flame graph
// -----------------------------------------------------------------------

#[test]
fn flame_graph_well_formed_svg() {
    let profiler = setup_profiler();
    let svg = FlameGraphGenerator::to_svg(&profiler, &FlameGraphConfig::default()).unwrap();

    // Well-formed SVG
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));

    // Contains expected labels
    assert!(svg.contains("geometry"));
    assert!(svg.contains("post_process"));
}

// -----------------------------------------------------------------------
// HTML Dashboard
// -----------------------------------------------------------------------

#[test]
fn dashboard_contains_all_sections() {
    let profiler = setup_profiler();
    let html = DashboardGenerator::to_html(&profiler, &DashboardConfig::default()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Aggregate Statistics"));
    assert!(html.contains("Baseline Comparison"));
    assert!(html.contains("initial")); // baseline label
    assert!(html.contains("Historical Frame Times"));
    assert!(html.contains("Flame Graph"));
    assert!(html.contains("</html>"));
}

// -----------------------------------------------------------------------
// File I/O round-trip
// -----------------------------------------------------------------------

#[test]
fn file_export_round_trip() {
    let profiler = setup_profiler();
    let exporter = ProfileExporter::new(&profiler);
    let dir = std::env::temp_dir().join("gup_integration_export");
    std::fs::create_dir_all(&dir).unwrap();

    // JSON
    let json_path = dir.join("profile.json");
    exporter
        .export_json(&json_path, &ExportConfig::default())
        .unwrap();
    assert!(json_path.exists());
    let json_contents = std::fs::read_to_string(&json_path).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json_contents).unwrap();

    // CSV
    let csv_path = dir.join("profile.csv");
    exporter
        .export_csv(&csv_path, &ExportConfig::default())
        .unwrap();
    assert!(csv_path.exists());

    // Chrome Trace
    let trace_path = dir.join("trace.json");
    exporter
        .export_chrome_trace(&trace_path, &ExportConfig::default())
        .unwrap();
    assert!(trace_path.exists());

    // Flame graph
    let flame_path = dir.join("flamegraph.svg");
    FlameGraphGenerator::generate(&profiler, &flame_path, &FlameGraphConfig::default()).unwrap();
    assert!(flame_path.exists());

    // Dashboard
    let dash_path = dir.join("dashboard.html");
    DashboardGenerator::generate(&profiler, &dash_path, &DashboardConfig::default()).unwrap();
    assert!(dash_path.exists());

    // Clean up
    std::fs::remove_dir_all(&dir).ok();
}
