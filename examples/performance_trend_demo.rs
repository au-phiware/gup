// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Demo of performance trend visualization
//!
//! This example demonstrates how to:
//! 1. Create performance baselines
//! 2. Generate trend visualizations
//! 3. Export charts as SVG and HTML

use gup::debug::ci_performance::{BaselineStorage, PerformanceBaseline, PerformanceTrendVisualizer};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Performance Trend Visualization Demo\n");

    // Set up temporary directory for demo
    let demo_dir = std::env::temp_dir().join("gup_trend_demo");
    let _ = std::fs::remove_dir_all(&demo_dir);
    std::fs::create_dir_all(&demo_dir)?;

    println!("📁 Using directory: {}\n", demo_dir.display());

    // Create baseline storage
    let storage = BaselineStorage::new(demo_dir.clone());

    // Simulate 3 months of performance data for multiple tests
    println!("📝 Creating sample performance baselines...");
    
    let tests = vec![
        ("render_100k_points", vec![16.8, 16.5, 16.2, 16.0, 15.8, 15.7]),
        ("compute_statistics", vec![5.2, 5.1, 4.9, 4.8, 4.7, 4.6]),
        ("text_rendering", vec![8.5, 8.3, 8.2, 8.1, 8.0, 7.9]),
    ];

    for (test_name, frame_times) in &tests {
        println!("  - {}", test_name);
        
        for (i, &frame_time) in frame_times.iter().enumerate() {
            let days_ago = (frame_times.len() - 1 - i) as i64 * 15; // Every 15 days
            let timestamp = chrono::Utc::now() - chrono::Duration::days(days_ago);

            let baseline = PerformanceBaseline {
                test_name: test_name.to_string(),
                category: "rendering".to_string(),
                avg_frame_time_ms: frame_time,
                avg_memory_usage_bytes: 10_000_000 + (i as u64 * 200_000),
                sample_count: 1,
                last_updated: timestamp,
                metadata: HashMap::new(),
                platform_id: "demo_platform".to_string(),
            };

            // Create unique baseline entry for each time point
            let unique_name = format!("{}_{}", test_name, i);
            storage.save_baseline(&unique_name, "rendering", "demo_platform", &baseline)?;
        }
    }

    println!("\n✅ Created {} tests with {} data points each\n", tests.len(), tests[0].1.len());

    // Create trend visualizer
    println!("📈 Generating trend charts...");
    let visualizer = PerformanceTrendVisualizer::new(demo_dir.clone());

    // Generate all trend charts
    let charts = visualizer.generate_all_trend_charts()?;
    println!("  Generated {} trend charts\n", charts.len());

    // Export individual SVG files
    println!("💾 Exporting SVG files...");
    let charts_dir = demo_dir.join("charts");
    let paths = visualizer.export_charts_to_directory(&charts_dir)?;
    
    for path in &paths {
        println!("  - {}", path.display());
    }

    // Generate and export dashboard
    println!("\n🌐 Generating HTML dashboard...");
    let dashboard_path = demo_dir.join("dashboard.html");
    visualizer.export_dashboard(&dashboard_path)?;
    
    println!("  Dashboard: {}", dashboard_path.display());

    // Display some statistics
    println!("\n📊 Chart Statistics:");
    for (name, svg) in &charts {
        let size = svg.len();
        println!("  - {}: {} bytes", name, size);
    }

    println!("\n✨ Demo complete!");
    println!("\n💡 View the results:");
    println!("   HTML Dashboard: file://{}", dashboard_path.display());
    println!("   SVG Charts:     {}", charts_dir.display());
    println!("\n   Or open in browser: xdg-open {}", dashboard_path.display());

    Ok(())
}
