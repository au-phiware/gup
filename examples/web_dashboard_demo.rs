// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating the web-based profiling dashboard.
//!
//! This example shows how to use the WebDashboard to monitor GPU memory usage
//! and performance in real-time through a web interface.
//!
//! # Usage
//!
//! Build with the web-dashboard feature:
//! ```bash
//! cargo run --example web_dashboard_demo --features web-dashboard
//! ```
//!
//! Then open http://127.0.0.1:8080 in your web browser to view the dashboard.

use gup::GupContext;
use gup::debug::{GpuMemoryProfiler, WebDashboard};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use wgpu::BufferUsages;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Starting Gup Web Profiling Dashboard Demo");
    println!("================================================\n");

    // Initialize GPU context
    println!("📊 Initializing GPU context...");
    let context = Arc::new(GupContext::new().await?);
    println!("✅ GPU context initialized\n");

    // Create memory profiler
    println!("🔍 Creating GPU memory profiler...");
    let profiler = Arc::new(GpuMemoryProfiler::new(&context.device, &context.queue));
    println!("✅ Memory profiler created\n");

    // Start web dashboard
    println!("🌐 Starting web dashboard server...");
    let dashboard = WebDashboard::new(profiler.clone());
    dashboard.start("127.0.0.1:8080")?;
    println!("✅ Dashboard server started\n");

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║                                                          ║");
    println!("║   📊 Web Dashboard Running at http://127.0.0.1:8080     ║");
    println!("║                                                          ║");
    println!("║   Open the URL in your web browser to view:             ║");
    println!("║   • Real-time memory usage charts                        ║");
    println!("║   • Buffer usage breakdown                               ║");
    println!("║   • Active allocations table                             ║");
    println!("║   • Memory leak detection                                ║");
    println!("║                                                          ║");
    println!("║   Press Ctrl+C to stop the server                        ║");
    println!("║                                                          ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Simulate some GPU memory allocations to demonstrate the dashboard
    println!("🎯 Simulating GPU memory allocations...\n");

    for i in 1..=10 {
        // Create a buffer
        let size = 1024 * 1024 * i; // 1MB to 10MB
        let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Demo buffer #{}", i)),
            size: size as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Register with profiler
        let allocation_id = profiler.register_allocation(
            &buffer,
            Some(&format!("Demo buffer #{}", i)),
            size as u64,
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );

        println!("  ✓ Allocated {} MB (ID: {})", i, allocation_id);

        // Keep the buffer alive by moving it into a closure
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(30));
            drop(buffer);
        });

        thread::sleep(Duration::from_millis(500));
    }

    println!("\n✅ Allocations complete. Dashboard is now showing live data.\n");
    println!("💡 Tip: Try these dashboard features:");
    println!("   • Click 'Auto-Refresh' to enable live updates every 2 seconds");
    println!("   • Click 'Check for Leaks' to detect potential memory leaks");
    println!("   • Click 'Export JSON' to download profiling data\n");

    // Keep the program running to serve the dashboard
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
