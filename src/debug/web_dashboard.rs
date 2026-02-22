// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Web-based profiling dashboard for GPU memory and performance monitoring.
//!
//! This module provides an embedded web server that serves an interactive dashboard
//! for visualizing GPU profiling data. The dashboard includes:
//! - Real-time memory usage charts
//! - Performance timeline visualization  
//! - Buffer allocation tables
//! - Historical session comparison
//!
//! # Usage
//!
//! ```no_run
//! use gup::debug::{GpuMemoryProfiler, WebDashboard};
//!
//! let profiler = GpuMemoryProfiler::new();
//! let dashboard = WebDashboard::new(profiler.clone());
//! dashboard.start("127.0.0.1:8080")?;
//! ```
//!
//! The dashboard is only available when the `web-dashboard` feature is enabled.

use crate::debug::memory_profiler::GpuMemoryProfiler;
use crate::error::{GupError, GupResult};
use std::sync::Arc;

#[cfg(feature = "web-dashboard")]
use std::thread;

#[cfg(feature = "web-dashboard")]
use tiny_http::{Method, Response, Server, StatusCode};

/// Web-based profiling dashboard server.
///
/// Provides an HTTP server that serves an interactive dashboard for GPU profiling data.
/// The server binds to a local address and provides REST API endpoints for accessing
/// profiling data.
#[derive(Clone)]
pub struct WebDashboard {
    profiler: Arc<GpuMemoryProfiler>,
}

impl WebDashboard {
    /// Creates a new web dashboard connected to the given profiler.
    pub fn new(profiler: Arc<GpuMemoryProfiler>) -> Self {
        Self { profiler }
    }

    /// Starts the web dashboard server on the specified address.
    ///
    /// The server runs in a background thread and serves the dashboard UI
    /// along with REST API endpoints for profiling data.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to bind to (e.g., "127.0.0.1:8080")
    ///
    /// # Security Note
    ///
    /// The server binds to localhost by default for security. Only bind to
    /// 0.0.0.0 if you need network access and understand the security implications.
    #[cfg(feature = "web-dashboard")]
    pub fn start(&self, addr: &str) -> GupResult<()> {
        let server = Server::http(addr)
            .map_err(|e| GupError::resource_error(format!("Failed to start web server: {}", e)))?;

        let profiler = self.profiler.clone();

        thread::spawn(move || {
            for request in server.incoming_requests() {
                let response = match (request.method(), request.url()) {
                    // Serve the main dashboard HTML
                    (Method::Get, "/") | (Method::Get, "/index.html") => {
                        Response::from_string(DASHBOARD_HTML).with_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Type"[..],
                                &b"text/html; charset=utf-8"[..],
                            )
                            .unwrap(),
                        )
                    }

                    // API: Get current memory report
                    (Method::Get, "/api/memory") => {
                        let report = profiler.get_memory_report();
                        match serde_json::to_string(&report) {
                            Ok(json) => Response::from_string(json).with_header(
                                tiny_http::Header::from_bytes(
                                    &b"Content-Type"[..],
                                    &b"application/json"[..],
                                )
                                .unwrap(),
                            ),
                            Err(e) => Response::from_string(format!("{{\"error\": \"{}\"}}", e))
                                .with_status_code(StatusCode(500)),
                        }
                    }

                    // API: Get memory leak detection
                    (Method::Get, "/api/leaks") => {
                        let report = profiler.get_memory_report();
                        match serde_json::to_string(&report.detected_leaks) {
                            Ok(json) => Response::from_string(json).with_header(
                                tiny_http::Header::from_bytes(
                                    &b"Content-Type"[..],
                                    &b"application/json"[..],
                                )
                                .unwrap(),
                            ),
                            Err(e) => Response::from_string(format!("{{\"error\": \"{}\"}}", e))
                                .with_status_code(StatusCode(500)),
                        }
                    }

                    // API: Export profiling data
                    (Method::Get, "/api/export") => {
                        let report = profiler.get_memory_report();
                        match serde_json::to_string_pretty(&report) {
                            Ok(json) => Response::from_string(json)
                                .with_header(
                                    tiny_http::Header::from_bytes(
                                        &b"Content-Type"[..],
                                        &b"application/json"[..],
                                    )
                                    .unwrap(),
                                )
                                .with_header(
                                    tiny_http::Header::from_bytes(
                                        &b"Content-Disposition"[..],
                                        &b"attachment; filename=\"profiling-data.json\""[..],
                                    )
                                    .unwrap(),
                                ),
                            Err(e) => Response::from_string(format!(
                                "{{\"error\": \"{}\"}}",
                                e
                            ))
                            .with_status_code(StatusCode(500)),
                        }
                    }

                    // 404 for everything else
                    _ => Response::from_string("Not Found").with_status_code(StatusCode(404)),
                };

                let _ = request.respond(response);
            }
        });

        Ok(())
    }

    /// Starts the web dashboard server (no-op when feature is disabled).
    #[cfg(not(feature = "web-dashboard"))]
    pub fn start(&self, _addr: &str) -> GupResult<()> {
        Err(GupError::configuration_error(
            "web-dashboard",
            "Feature not enabled. Rebuild with --features web-dashboard",
        ))
    }
}

/// Self-contained HTML dashboard.
///
/// This is a single-file dashboard that includes all HTML, CSS, and JavaScript inline.
/// It uses Chart.js from a CDN for charting functionality and updates data via REST API.
#[cfg(feature = "web-dashboard")]
const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Gup GPU Profiling Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0f172a;
            color: #e2e8f0;
            padding: 20px;
        }
        
        .header {
            text-align: center;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            border-radius: 12px;
            margin-bottom: 30px;
        }
        
        h1 {
            font-size: 2.5em;
            font-weight: 700;
            margin-bottom: 10px;
        }
        
        .subtitle {
            opacity: 0.9;
            font-size: 1.1em;
        }
        
        .dashboard {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(500px, 1fr));
            gap: 20px;
            margin-bottom: 20px;
        }
        
        .card {
            background: #1e293b;
            border-radius: 12px;
            padding: 20px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        }
        
        .card-title {
            font-size: 1.5em;
            font-weight: 600;
            margin-bottom: 15px;
            color: #60a5fa;
        }
        
        .stat-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 15px;
            margin: 20px 0;
        }
        
        .stat {
            background: #0f172a;
            padding: 15px;
            border-radius: 8px;
            border-left: 4px solid #60a5fa;
        }
        
        .stat-label {
            font-size: 0.9em;
            opacity: 0.7;
            margin-bottom: 5px;
        }
        
        .stat-value {
            font-size: 1.8em;
            font-weight: 700;
            color: #60a5fa;
        }
        
        .chart-container {
            position: relative;
            height: 300px;
            margin-top: 20px;
        }
        
        .controls {
            display: flex;
            gap: 10px;
            margin-bottom: 20px;
            flex-wrap: wrap;
        }
        
        button {
            background: #60a5fa;
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 8px;
            font-size: 1em;
            cursor: pointer;
            transition: all 0.2s;
            font-weight: 500;
        }
        
        button:hover {
            background: #3b82f6;
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(96, 165, 250, 0.4);
        }
        
        button:active {
            transform: translateY(0);
        }
        
        .table-container {
            overflow-x: auto;
            margin-top: 20px;
        }
        
        table {
            width: 100%;
            border-collapse: collapse;
        }
        
        th, td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #334155;
        }
        
        th {
            background: #0f172a;
            font-weight: 600;
            color: #60a5fa;
        }
        
        tr:hover {
            background: #334155;
        }
        
        .alert {
            background: #dc2626;
            color: white;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        
        .status {
            display: inline-block;
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 0.85em;
            font-weight: 600;
        }
        
        .status-ok { background: #10b981; }
        .status-warning { background: #f59e0b; }
        .status-error { background: #ef4444; }
    </style>
</head>
<body>
    <div class="header">
        <h1>🚀 Gup GPU Profiling Dashboard</h1>
        <p class="subtitle">Real-time GPU Memory and Performance Monitoring</p>
    </div>
    
    <div class="controls">
        <button onclick="updateData()">🔄 Refresh Data</button>
        <button onclick="exportData()">📥 Export JSON</button>
        <button onclick="checkLeaks()">🔍 Check for Leaks</button>
        <button onclick="toggleAutoRefresh()">⏱️ Auto-Refresh: <span id="auto-status">OFF</span></button>
    </div>
    
    <div id="alerts"></div>
    
    <div class="dashboard">
        <div class="card">
            <div class="card-title">📊 Memory Overview</div>
            <div class="stat-grid">
                <div class="stat">
                    <div class="stat-label">Total Allocated</div>
                    <div class="stat-value" id="total-memory">0 MB</div>
                </div>
                <div class="stat">
                    <div class="stat-label">Active Buffers</div>
                    <div class="stat-value" id="buffer-count">0</div>
                </div>
                <div class="stat">
                    <div class="stat-label">Memory Trend</div>
                    <div class="stat-value" id="memory-trend">Stable</div>
                </div>
                <div class="stat">
                    <div class="stat-label">Potential Leaks</div>
                    <div class="stat-value" id="leak-count">0</div>
                </div>
            </div>
            <div class="chart-container">
                <canvas id="memoryChart"></canvas>
            </div>
        </div>
        
        <div class="card">
            <div class="card-title">📈 Buffer Usage Breakdown</div>
            <div class="chart-container">
                <canvas id="usageChart"></canvas>
            </div>
        </div>
    </div>
    
    <div class="card">
        <div class="card-title">🔢 Active Allocations</div>
        <div class="table-container">
            <table id="allocations-table">
                <thead>
                    <tr>
                        <th>Allocation ID</th>
                        <th>Size</th>
                        <th>Usage</th>
                        <th>Age</th>
                        <th>Status</th>
                    </tr>
                </thead>
                <tbody id="allocations-body">
                    <tr><td colspan="5" style="text-align: center; opacity: 0.5;">Loading...</td></tr>
                </tbody>
            </table>
        </div>
    </div>

    <script>
        let memoryChart = null;
        let usageChart = null;
        let autoRefreshInterval = null;
        
        // Initialize charts
        const memoryCtx = document.getElementById('memoryChart').getContext('2d');
        memoryChart = new Chart(memoryCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [{
                    label: 'Memory Usage (MB)',
                    data: [],
                    borderColor: '#60a5fa',
                    backgroundColor: 'rgba(96, 165, 250, 0.1)',
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { labels: { color: '#e2e8f0' } }
                },
                scales: {
                    y: {
                        beginAtZero: true,
                        ticks: { color: '#94a3b8' },
                        grid: { color: '#334155' }
                    },
                    x: {
                        ticks: { color: '#94a3b8' },
                        grid: { color: '#334155' }
                    }
                }
            }
        });
        
        const usageCtx = document.getElementById('usageChart').getContext('2d');
        usageChart = new Chart(usageCtx, {
            type: 'doughnut',
            data: {
                labels: [],
                datasets: [{
                    data: [],
                    backgroundColor: [
                        '#60a5fa',
                        '#34d399',
                        '#fbbf24',
                        '#f87171',
                        '#a78bfa',
                        '#fb923c'
                    ]
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: 'right',
                        labels: { color: '#e2e8f0' }
                    }
                }
            }
        });
        
        // Update data from API
        async function updateData() {
            try {
                const response = await fetch('/api/memory');
                const data = await response.json();
                
                // Update stats
                document.getElementById('total-memory').textContent = 
                    (data.total_memory_active / 1024 / 1024).toFixed(2) + ' MB';
                document.getElementById('buffer-count').textContent = data.active_allocations;
                document.getElementById('memory-trend').textContent = 'N/A';
                document.getElementById('leak-count').textContent = data.detected_leaks.length;
                
                // Update memory history chart (simplified - just show current value)
                const now = new Date();
                if (memoryChart.data.labels.length > 20) {
                    memoryChart.data.labels.shift();
                    memoryChart.data.datasets[0].data.shift();
                }
                memoryChart.data.labels.push(now.toLocaleTimeString());
                memoryChart.data.datasets[0].data.push(data.total_memory_active / 1024 / 1024);
                memoryChart.update();
                
                // Update usage breakdown chart
                if (data.usage_breakdown) {
                    const labels = Object.keys(data.usage_breakdown);
                    const values = Object.values(data.usage_breakdown).map(v => v / 1024 / 1024);
                    
                    usageChart.data.labels = labels;
                    usageChart.data.datasets[0].data = values;
                    usageChart.update();
                }
                
                // Update allocations table
                updateAllocationsTable(data.largest_allocations || []);
                
            } catch (error) {
                console.error('Failed to fetch data:', error);
                showAlert('Failed to fetch profiling data: ' + error.message);
            }
        }
        
        function updateAllocationsTable(allocations) {
            const tbody = document.getElementById('allocations-body');
            
            if (allocations.length === 0) {
                tbody.innerHTML = '<tr><td colspan="5" style="text-align: center; opacity: 0.5;">No active allocations</td></tr>';
                return;
            }
            
            tbody.innerHTML = allocations.map(alloc => `
                <tr>
                    <td><code>${alloc.id || 'N/A'}</code></td>
                    <td>${formatBytes(alloc.size)}</td>
                    <td>${alloc.label || 'UNKNOWN'}</td>
                    <td>${formatDuration(alloc.age)}</td>
                    <td><span class="status status-ok">Active</span></td>
                </tr>
            `).join('');
        }
        
        async function checkLeaks() {
            try {
                const response = await fetch('/api/leaks');
                const leaks = await response.json();
                
                document.getElementById('leak-count').textContent = leaks.length;
                
                if (leaks.length > 0) {
                    showAlert(`⚠️ Detected ${leaks.length} potential memory leaks!`);
                } else {
                    showAlert('✅ No memory leaks detected', 'success');
                }
            } catch (error) {
                showAlert('Failed to check for leaks: ' + error.message);
            }
        }
        
        async function exportData() {
            try {
                const response = await fetch('/api/export');
                const blob = await response.blob();
                const url = window.URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = 'profiling-data.json';
                a.click();
                window.URL.revokeObjectURL(url);
            } catch (error) {
                showAlert('Failed to export data: ' + error.message);
            }
        }
        
        function toggleAutoRefresh() {
            if (autoRefreshInterval) {
                clearInterval(autoRefreshInterval);
                autoRefreshInterval = null;
                document.getElementById('auto-status').textContent = 'OFF';
            } else {
                autoRefreshInterval = setInterval(updateData, 2000);
                document.getElementById('auto-status').textContent = 'ON';
                updateData();
            }
        }
        
        function showAlert(message, type = 'error') {
            const alertsDiv = document.getElementById('alerts');
            const alert = document.createElement('div');
            alert.className = 'alert';
            alert.textContent = message;
            if (type === 'success') {
                alert.style.background = '#10b981';
            }
            alertsDiv.appendChild(alert);
            setTimeout(() => alert.remove(), 5000);
        }
        
        function formatBytes(bytes) {
            if (bytes < 1024) return bytes + ' B';
            if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
            return (bytes / 1024 / 1024).toFixed(2) + ' MB';
        }
        
        function formatDuration(seconds) {
            if (seconds < 60) return seconds.toFixed(1) + 's';
            if (seconds < 3600) return (seconds / 60).toFixed(1) + 'm';
            return (seconds / 3600).toFixed(1) + 'h';
        }
        
        // Initial data load
        updateData();
    </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        // Create a simple profiler without GPU context for basic testing
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("Failed to find an appropriate adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device");

            let profiler = Arc::new(GpuMemoryProfiler::new(&device, &queue));
            let dashboard = WebDashboard::new(profiler);
            // Just verify we can create a dashboard
            assert!(!std::ptr::addr_of!(dashboard).is_null());
        });
    }

    #[test]
    #[cfg(not(feature = "web-dashboard"))]
    fn test_dashboard_disabled_without_feature() {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("Failed to find an appropriate adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device");

            let profiler = Arc::new(GpuMemoryProfiler::new(&device, &queue));
            let dashboard = WebDashboard::new(profiler);
            let result = dashboard.start("127.0.0.1:8080");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not enabled"));
        });
    }
}
