// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Profiling data export and visualization.
//!
//! This module provides export capabilities for performance profiling data
//! collected by [`PerformanceProfiler`](crate::performance::PerformanceProfiler).
//!
//! # Export Formats
//!
//! * **JSON** — Full-fidelity structured export for programmatic analysis.
//! * **CSV** — Tabular per-frame data for spreadsheet and data-science tools.
//! * **Chrome Trace** — Chrome DevTools Performance panel format (`chrome://tracing`).
//!
//! # Flame Graphs
//!
//! [`FlameGraphGenerator`] produces self-contained SVG flame graphs from render
//! pass hierarchies, suitable for embedding in reports or opening in a browser.
//!
//! # HTML Dashboard
//!
//! [`DashboardGenerator`] produces a self-contained HTML file with interactive
//! charts showing historical trends, baseline comparisons, and active alerts.
//!
//! # Example
//!
//! ```rust,no_run
//! use gup::performance_export::{ProfileExporter, ExportConfig, ExportGranularity};
//! use gup::performance::PerformanceProfiler;
//! use std::path::Path;
//!
//! # fn example(profiler: &PerformanceProfiler) -> gup::error::GupResult<()> {
//! let exporter = ProfileExporter::new(profiler);
//! let config = ExportConfig {
//!     granularity: ExportGranularity::PerFrame,
//!     ..Default::default()
//! };
//! exporter.export_json(Path::new("profile.json"), &config)?;
//! exporter.export_csv(Path::new("profile.csv"), &config)?;
//! exporter.export_chrome_trace(Path::new("trace.json"), &config)?;
//! # Ok(())
//! # }
//! ```

use crate::error::{GupError, GupResult};
use crate::performance::{
    AggregateStats, DetailedFrameStats, PerformanceAlert, PerformanceBaseline, PerformanceProfiler,
};
use serde::Serialize;
use std::path::Path;

// ---------------------------------------------------------------------------
// Export configuration
// ---------------------------------------------------------------------------

/// Controls how much detail is included in exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum ExportGranularity {
    /// Export only aggregate statistics.
    Aggregate,
    /// Export per-frame data.
    PerFrame,
    /// Export both aggregate and per-frame data.
    #[default]
    Full,
}

/// Configuration for profile data exports.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Level of detail to include.
    pub granularity: ExportGranularity,
    /// Whether to include bandwidth statistics.
    pub include_bandwidth: bool,
    /// Whether to include alert data.
    pub include_alerts: bool,
    /// Whether to include baseline data.
    pub include_baselines: bool,
    /// Pretty-print JSON output.
    pub pretty_print: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            granularity: ExportGranularity::Full,
            include_bandwidth: true,
            include_alerts: true,
            include_baselines: true,
            pretty_print: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Serializable snapshot used by JSON and Chrome-trace exporters
// ---------------------------------------------------------------------------

/// A serializable snapshot of the profiler state.
#[derive(Debug, Serialize)]
struct ProfileSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    aggregate: Option<AggregateStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frames: Option<Vec<DetailedFrameStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    baselines: Option<Vec<PerformanceBaseline>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alerts: Option<Vec<PerformanceAlert>>,
}

// ---------------------------------------------------------------------------
// ProfileExporter
// ---------------------------------------------------------------------------

/// Exports profiling data from a [`PerformanceProfiler`] in various formats.
pub struct ProfileExporter<'a> {
    profiler: &'a PerformanceProfiler,
}

impl<'a> ProfileExporter<'a> {
    /// Create a new exporter bound to *profiler*.
    pub fn new(profiler: &'a PerformanceProfiler) -> Self {
        Self { profiler }
    }

    // -- JSON ---------------------------------------------------------------

    /// Export profiling data to a JSON file at *path*.
    pub fn export_json(&self, path: &Path, config: &ExportConfig) -> GupResult<()> {
        let json = self.to_json(config)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Serialize profiling data to a JSON string.
    pub fn to_json(&self, config: &ExportConfig) -> GupResult<String> {
        let snapshot = self.build_snapshot(config);
        let json = if config.pretty_print {
            serde_json::to_string_pretty(&snapshot)?
        } else {
            serde_json::to_string(&snapshot)?
        };
        Ok(json)
    }

    // -- CSV ----------------------------------------------------------------

    /// Export per-frame profiling data to a CSV file at *path*.
    ///
    /// CSV exports always include per-frame rows; the granularity setting
    /// controls whether an aggregate summary row is appended.
    pub fn export_csv(&self, path: &Path, config: &ExportConfig) -> GupResult<()> {
        let csv = self.to_csv(config)?;
        std::fs::write(path, csv)?;
        Ok(())
    }

    /// Serialize per-frame profiling data to a CSV string.
    pub fn to_csv(&self, config: &ExportConfig) -> GupResult<String> {
        let mut wtr = csv::Writer::from_writer(Vec::new());

        // Header
        wtr.write_record([
            "frame",
            "cpu_time_ms",
            "gpu_time_ms",
            "draw_calls",
            "pipeline_switches",
            "compute_dispatches",
            "buffer_upload_ms",
            "render_passes",
        ])
        .map_err(|e| GupError::resource_error(format!("CSV write error: {e}")))?;

        // Per-frame rows
        if config.granularity != ExportGranularity::Aggregate {
            for (i, frame) in self.profiler.history().iter().enumerate() {
                wtr.write_record(&[
                    i.to_string(),
                    format!("{:.4}", frame.cpu_time.as_secs_f64() * 1000.0),
                    frame
                        .gpu_time
                        .map_or("".into(), |d| format!("{:.4}", d.as_secs_f64() * 1000.0)),
                    frame.draw_calls.to_string(),
                    frame.pipeline_switches.to_string(),
                    frame.compute_dispatches.to_string(),
                    format!("{:.4}", frame.buffer_upload_time.as_secs_f64() * 1000.0),
                    frame.render_pass_times.len().to_string(),
                ])
                .map_err(|e| GupError::resource_error(format!("CSV write error: {e}")))?;
            }
        }

        let inner = wtr
            .into_inner()
            .map_err(|e| GupError::resource_error(format!("CSV flush error: {e}")))?;
        String::from_utf8(inner)
            .map_err(|e| GupError::resource_error(format!("CSV encoding error: {e}")))
    }

    // -- Chrome Trace Format ------------------------------------------------

    /// Export profiling data in Chrome Trace Event Format to *path*.
    ///
    /// The output can be loaded in `chrome://tracing` or the Perfetto UI.
    pub fn export_chrome_trace(&self, path: &Path, config: &ExportConfig) -> GupResult<()> {
        let json = self.to_chrome_trace(config)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Serialize profiling data to Chrome Trace Event Format JSON.
    pub fn to_chrome_trace(&self, config: &ExportConfig) -> GupResult<String> {
        let events = self.build_trace_events(config);
        let json = if config.pretty_print {
            serde_json::to_string_pretty(&events)?
        } else {
            serde_json::to_string(&events)?
        };
        Ok(json)
    }

    // -- helpers ------------------------------------------------------------

    fn build_snapshot(&self, config: &ExportConfig) -> ProfileSnapshot {
        let aggregate = match config.granularity {
            ExportGranularity::PerFrame => None,
            _ => Some(self.profiler.aggregate_stats()),
        };

        let frames = match config.granularity {
            ExportGranularity::Aggregate => None,
            _ => Some(self.profiler.history().iter().cloned().collect()),
        };

        let baselines = if config.include_baselines {
            let b: Vec<_> = self.profiler.baselines().to_vec();
            if b.is_empty() { None } else { Some(b) }
        } else {
            None
        };

        let alerts = if config.include_alerts {
            let a: Vec<_> = self.profiler.alerts().to_vec();
            if a.is_empty() { None } else { Some(a) }
        } else {
            None
        };

        ProfileSnapshot {
            aggregate,
            frames,
            baselines,
            alerts,
        }
    }

    fn build_trace_events(&self, _config: &ExportConfig) -> Vec<ChromeTraceEvent> {
        let mut events = Vec::new();
        let mut time_offset_us: f64 = 0.0;

        for (frame_idx, frame) in self.profiler.history().iter().enumerate() {
            let frame_start_us = time_offset_us;
            let frame_dur_us = frame.cpu_time.as_secs_f64() * 1_000_000.0;

            // Frame-level duration event
            events.push(ChromeTraceEvent {
                name: format!("Frame {frame_idx}"),
                cat: "frame".into(),
                ph: "X".into(),
                ts: frame_start_us,
                dur: Some(frame_dur_us),
                pid: 1,
                tid: 1,
                args: Some(TraceEventArgs {
                    draw_calls: Some(frame.draw_calls),
                    pipeline_switches: Some(frame.pipeline_switches),
                    compute_dispatches: Some(frame.compute_dispatches),
                }),
            });

            // Render pass events on a separate thread-id
            let mut pass_offset = frame_start_us;
            for pass in &frame.render_pass_times {
                let pass_dur = pass.cpu_time.as_secs_f64() * 1_000_000.0;
                events.push(ChromeTraceEvent {
                    name: pass.label.clone().unwrap_or_else(|| "render_pass".into()),
                    cat: "render_pass".into(),
                    ph: "X".into(),
                    ts: pass_offset,
                    dur: Some(pass_dur),
                    pid: 1,
                    tid: 2,
                    args: Some(TraceEventArgs {
                        draw_calls: Some(pass.draw_calls),
                        pipeline_switches: None,
                        compute_dispatches: None,
                    }),
                });
                pass_offset += pass_dur;
            }

            // Buffer upload event
            if frame.buffer_upload_time.as_nanos() > 0 {
                events.push(ChromeTraceEvent {
                    name: "buffer_upload".into(),
                    cat: "gpu".into(),
                    ph: "X".into(),
                    ts: frame_start_us,
                    dur: Some(frame.buffer_upload_time.as_secs_f64() * 1_000_000.0),
                    pid: 1,
                    tid: 3,
                    args: None,
                });
            }

            time_offset_us += frame_dur_us;
        }

        events
    }
}

/// A single Chrome Trace Event (subset of the Trace Event Format).
#[derive(Debug, Serialize)]
struct ChromeTraceEvent {
    name: String,
    cat: String,
    /// Phase – "X" for complete events.
    ph: String,
    /// Timestamp in microseconds.
    ts: f64,
    /// Duration in microseconds (for "X" phase).
    #[serde(skip_serializing_if = "Option::is_none")]
    dur: Option<f64>,
    pid: u32,
    tid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<TraceEventArgs>,
}

/// Optional structured arguments attached to a trace event.
#[derive(Debug, Serialize)]
struct TraceEventArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    draw_calls: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pipeline_switches: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compute_dispatches: Option<u32>,
}

// ---------------------------------------------------------------------------
// Flame graph generation
// ---------------------------------------------------------------------------

/// Configuration for flame graph generation.
#[derive(Debug, Clone)]
pub struct FlameGraphConfig {
    /// SVG image width in pixels.
    pub width: u32,
    /// Row height in pixels.
    pub row_height: u32,
    /// Font size in pixels.
    pub font_size: u32,
    /// Title displayed above the graph.
    pub title: String,
    /// Minimum width (in pixels) for a frame to be visible.
    pub min_width: f64,
}

impl Default for FlameGraphConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            row_height: 20,
            font_size: 12,
            title: "Gup Render Pass Flame Graph".into(),
            min_width: 1.0,
        }
    }
}

/// Generates SVG flame graphs from profiling data.
pub struct FlameGraphGenerator;

impl FlameGraphGenerator {
    /// Generate an SVG flame graph and write it to *path*.
    pub fn generate(
        profiler: &PerformanceProfiler,
        path: &Path,
        config: &FlameGraphConfig,
    ) -> GupResult<()> {
        let svg = Self::to_svg(profiler, config)?;
        std::fs::write(path, svg)?;
        Ok(())
    }

    /// Generate an SVG flame graph as a string.
    pub fn to_svg(profiler: &PerformanceProfiler, config: &FlameGraphConfig) -> GupResult<String> {
        let stacks = Self::build_stacks(profiler);
        if stacks.is_empty() {
            return Err(GupError::resource_error(
                "No profiling data available for flame graph generation",
            ));
        }
        Ok(Self::render_svg(&stacks, config))
    }

    // -- internal -----------------------------------------------------------

    /// Flatten profiling history into flame-graph stack entries.
    ///
    /// Each entry is (stack_path, duration_us).
    fn build_stacks(profiler: &PerformanceProfiler) -> Vec<FlameStack> {
        let mut stacks: Vec<FlameStack> = Vec::new();

        for (frame_idx, frame) in profiler.history().iter().enumerate() {
            let frame_label = format!("Frame {frame_idx}");
            let frame_dur = frame.cpu_time.as_secs_f64() * 1_000_000.0;

            // Top-level frame row
            stacks.push(FlameStack {
                labels: vec![frame_label.clone()],
                duration_us: frame_dur,
            });

            // Render-pass rows nested under the frame
            for pass in &frame.render_pass_times {
                let pass_label = pass.label.clone().unwrap_or_else(|| "render_pass".into());
                let pass_dur = pass.cpu_time.as_secs_f64() * 1_000_000.0;

                stacks.push(FlameStack {
                    labels: vec![frame_label.clone(), pass_label],
                    duration_us: pass_dur,
                });
            }

            // Buffer upload row
            if frame.buffer_upload_time.as_nanos() > 0 {
                stacks.push(FlameStack {
                    labels: vec![frame_label.clone(), "buffer_upload".into()],
                    duration_us: frame.buffer_upload_time.as_secs_f64() * 1_000_000.0,
                });
            }
        }

        stacks
    }

    fn render_svg(stacks: &[FlameStack], config: &FlameGraphConfig) -> String {
        // Compute total time across all top-level frames
        let total_us: f64 = stacks
            .iter()
            .filter(|s| s.labels.len() == 1)
            .map(|s| s.duration_us)
            .sum();

        if total_us <= 0.0 {
            return String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
        }

        let width = config.width as f64;
        let rh = config.row_height as f64;
        let max_depth = stacks.iter().map(|s| s.labels.len()).max().unwrap_or(1);
        let title_height = rh + 10.0;
        let svg_height = title_height + (max_depth as f64) * rh + 10.0;

        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"0 0 {width} {svg_height}\" \
             width=\"{width}\" height=\"{svg_height}\" \
             style=\"font-family:monospace;font-size:{}px\">\n",
            config.font_size,
        ));
        svg.push_str(
            "<style>\n\
             rect { stroke: #333; stroke-width: 0.5; }\n\
             text { fill: #fff; pointer-events: none; }\n\
             .frame { fill: #e76f51; }\n\
             .pass  { fill: #2a9d8f; }\n\
             .upload { fill: #e9c46a; }\n\
             rect:hover { opacity: 0.8; }\n\
             </style>\n",
        );
        // Title
        svg.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             fill=\"#333\" font-weight=\"bold\">{}</text>\n",
            width / 2.0,
            rh,
            Self::escape_xml(&config.title),
        ));

        // Track x offsets per depth so we can position nested bars
        let mut x_offset_at_depth: Vec<f64> = vec![0.0; max_depth];

        for stack in stacks {
            let depth = stack.labels.len() - 1;
            let x = x_offset_at_depth[depth];
            let bar_width = (stack.duration_us / total_us) * width;

            if bar_width < config.min_width {
                // Advance offset even for tiny bars
                x_offset_at_depth[depth] = x + bar_width;
                continue;
            }

            let y = title_height + depth as f64 * rh;
            let class = if depth == 0 {
                "frame"
            } else if stack.labels.last().is_some_and(|l| l == "buffer_upload") {
                "upload"
            } else {
                "pass"
            };

            let label = stack.labels.last().unwrap();
            let dur_text = if stack.duration_us >= 1000.0 {
                format!("{:.2}ms", stack.duration_us / 1000.0)
            } else {
                format!("{:.0}µs", stack.duration_us)
            };

            svg.push_str(&format!(
                "<g><title>{label} ({dur_text})</title>\
                 <rect class=\"{class}\" x=\"{x:.2}\" y=\"{y:.2}\" \
                 width=\"{bar_width:.2}\" height=\"{rh:.0}\" rx=\"2\"/>\n"
            ));

            // Only render text if bar is wide enough
            if bar_width > 40.0 {
                let text_x = x + 3.0;
                let text_y = y + rh - 5.0;
                let max_chars = ((bar_width - 6.0) / (config.font_size as f64 * 0.6)) as usize;
                let display = if label.len() > max_chars {
                    &label[..max_chars]
                } else {
                    label
                };
                svg.push_str(&format!(
                    "<text x=\"{text_x:.2}\" y=\"{text_y:.2}\">{}</text>\n",
                    Self::escape_xml(display),
                ));
            }

            svg.push_str("</g>\n");
            x_offset_at_depth[depth] = x + bar_width;
        }

        svg.push_str("</svg>\n");
        svg
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

#[derive(Debug)]
struct FlameStack {
    labels: Vec<String>,
    duration_us: f64,
}

// ---------------------------------------------------------------------------
// HTML Dashboard generation
// ---------------------------------------------------------------------------

/// Configuration for HTML dashboard generation.
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    /// Page title.
    pub title: String,
    /// Whether to include a flame graph section.
    pub include_flame_graph: bool,
    /// Whether to include baseline comparison.
    pub include_comparison: bool,
    /// Flame graph settings (used when `include_flame_graph` is true).
    pub flame_graph: FlameGraphConfig,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            title: "Gup Performance Dashboard".into(),
            include_flame_graph: true,
            include_comparison: true,
            flame_graph: FlameGraphConfig::default(),
        }
    }
}

/// Generates a self-contained HTML performance dashboard.
pub struct DashboardGenerator;

impl DashboardGenerator {
    /// Generate the dashboard HTML and write it to *path*.
    pub fn generate(
        profiler: &PerformanceProfiler,
        path: &Path,
        config: &DashboardConfig,
    ) -> GupResult<()> {
        let html = Self::to_html(profiler, config)?;
        std::fs::write(path, html)?;
        Ok(())
    }

    /// Generate the dashboard HTML as a string.
    pub fn to_html(profiler: &PerformanceProfiler, config: &DashboardConfig) -> GupResult<String> {
        let aggregate = profiler.aggregate_stats();
        let alerts = profiler.alerts();
        let baselines = profiler.baselines();
        let history = profiler.history();

        let mut html = String::new();

        // --- preamble ---
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("<meta charset=\"utf-8\">\n");
        html.push_str(&format!(
            "<title>{}</title>\n",
            Self::escape_html(&config.title)
        ));
        html.push_str("<style>\n");
        html.push_str(DASHBOARD_CSS);
        html.push_str("</style>\n</head>\n<body>\n");

        // --- header ---
        html.push_str(&format!("<h1>{}</h1>\n", Self::escape_html(&config.title),));

        // --- alerts ---
        if !alerts.is_empty() {
            html.push_str("<section class=\"alerts\">\n<h2>Active Alerts</h2>\n<ul>\n");
            for alert in alerts {
                let msg = format_alert(alert);
                html.push_str(&format!("<li class=\"alert\">{}</li>\n", msg));
            }
            html.push_str("</ul>\n</section>\n");
        }

        // --- aggregate stats ---
        html.push_str("<section class=\"stats\">\n<h2>Aggregate Statistics</h2>\n");
        html.push_str("<table>\n<tr><th>Metric</th><th>Value</th></tr>\n");
        html.push_str(&format!(
            "<tr><td>Frames</td><td>{}</td></tr>\n",
            aggregate.frame_count
        ));
        html.push_str(&format!(
            "<tr><td>Avg CPU</td><td>{:.3} ms</td></tr>\n",
            aggregate.avg_cpu_time.as_secs_f64() * 1000.0
        ));
        if let Some(gpu) = aggregate.avg_gpu_time {
            html.push_str(&format!(
                "<tr><td>Avg GPU</td><td>{:.3} ms</td></tr>\n",
                gpu.as_secs_f64() * 1000.0
            ));
        }
        html.push_str(&format!(
            "<tr><td>Min</td><td>{:.3} ms</td></tr>\n",
            aggregate.min_frame_time.as_secs_f64() * 1000.0
        ));
        html.push_str(&format!(
            "<tr><td>Max</td><td>{:.3} ms</td></tr>\n",
            aggregate.max_frame_time.as_secs_f64() * 1000.0
        ));
        html.push_str(&format!(
            "<tr><td>p95</td><td>{:.3} ms</td></tr>\n",
            aggregate.p95_frame_time.as_secs_f64() * 1000.0
        ));
        html.push_str(&format!(
            "<tr><td>p99</td><td>{:.3} ms</td></tr>\n",
            aggregate.p99_frame_time.as_secs_f64() * 1000.0
        ));
        html.push_str(&format!(
            "<tr><td>Std Dev</td><td>{:.3} ms</td></tr>\n",
            aggregate.std_dev.as_secs_f64() * 1000.0
        ));
        html.push_str(&format!(
            "<tr><td>Avg Draw Calls</td><td>{:.1}</td></tr>\n",
            aggregate.avg_draw_calls
        ));
        html.push_str(&format!(
            "<tr><td>Avg Pipeline Switches</td><td>{:.1}</td></tr>\n",
            aggregate.avg_pipeline_switches
        ));
        html.push_str("</table>\n</section>\n");

        // --- baseline comparison ---
        if config.include_comparison && !baselines.is_empty() {
            html.push_str("<section class=\"comparison\">\n<h2>Baseline Comparison</h2>\n");
            html.push_str(
                "<table>\n<tr><th>Baseline</th><th>Avg CPU (ms)</th>\
                           <th>Current (ms)</th><th>Δ</th></tr>\n",
            );
            let current_cpu_ms = aggregate.avg_cpu_time.as_secs_f64() * 1000.0;
            for b in baselines {
                let base_ms = b.stats.avg_cpu_time.as_secs_f64() * 1000.0;
                let delta_ms = current_cpu_ms - base_ms;
                let sign = if delta_ms >= 0.0 { "+" } else { "" };
                html.push_str(&format!(
                    "<tr><td>{}</td><td>{:.3}</td><td>{:.3}</td>\
                     <td>{}{:.3}</td></tr>\n",
                    Self::escape_html(&b.label),
                    base_ms,
                    current_cpu_ms,
                    sign,
                    delta_ms,
                ));
            }
            html.push_str("</table>\n</section>\n");
        }

        // --- frame time chart (inline SVG) ---
        if !history.is_empty() {
            html.push_str("<section class=\"chart\">\n<h2>Historical Frame Times</h2>\n");
            html.push_str(&Self::render_frame_time_chart(history, &aggregate));
            html.push_str("</section>\n");
        }

        // --- flame graph ---
        if config.include_flame_graph && !history.is_empty() {
            html.push_str("<section class=\"flame\">\n<h2>Flame Graph</h2>\n");
            match FlameGraphGenerator::to_svg(profiler, &config.flame_graph) {
                Ok(svg) => html.push_str(&svg),
                Err(_) => html.push_str("<p>No render pass data available.</p>\n"),
            }
            html.push_str("</section>\n");
        }

        html.push_str("</body>\n</html>\n");
        Ok(html)
    }

    // -- helpers --

    fn render_frame_time_chart(
        history: &std::collections::VecDeque<DetailedFrameStats>,
        aggregate: &AggregateStats,
    ) -> String {
        let chart_w = 800.0_f64;
        let chart_h = 200.0_f64;
        let margin = 40.0_f64;
        let total_w = chart_w + margin * 2.0;
        let total_h = chart_h + margin * 2.0;

        let max_ms = history
            .iter()
            .map(|f| f.cpu_time.as_secs_f64() * 1000.0)
            .fold(0.0_f64, f64::max)
            .max(0.001);

        let avg_ms = aggregate.avg_cpu_time.as_secs_f64() * 1000.0;
        let n = history.len();
        let bar_w = (chart_w / n as f64).max(1.0).min(20.0);

        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"0 0 {total_w} {total_h}\" \
             width=\"{total_w}\" height=\"{total_h}\" \
             style=\"font-family:monospace;font-size:11px\">\n"
        );

        // Background
        svg.push_str(&format!(
            "<rect x=\"{margin}\" y=\"{margin}\" \
             width=\"{chart_w}\" height=\"{chart_h}\" \
             fill=\"#f8f9fa\" stroke=\"#dee2e6\"/>\n"
        ));

        // Average line
        let avg_y = margin + chart_h - (avg_ms / max_ms) * chart_h;
        svg.push_str(&format!(
            "<line x1=\"{margin}\" y1=\"{avg_y:.1}\" \
             x2=\"{}\" y2=\"{avg_y:.1}\" \
             stroke=\"#e76f51\" stroke-dasharray=\"4,4\"/>\n",
            margin + chart_w
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{:.1}\" fill=\"#e76f51\">\
             avg {avg_ms:.2}ms</text>\n",
            margin + chart_w + 4.0,
            avg_y + 4.0,
        ));

        // Bars
        for (i, frame) in history.iter().enumerate() {
            let ms = frame.cpu_time.as_secs_f64() * 1000.0;
            let h = (ms / max_ms) * chart_h;
            let x = margin + i as f64 * bar_w;
            let y = margin + chart_h - h;
            svg.push_str(&format!(
                "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" \
                 height=\"{h:.2}\" fill=\"#2a9d8f\" opacity=\"0.8\">\
                 <title>Frame {i}: {ms:.3}ms</title></rect>\n",
                (bar_w - 0.5).max(0.5),
            ));
        }

        // Y axis label
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" transform=\"rotate(-90 {} {})\" \
             text-anchor=\"middle\" fill=\"#333\">ms</text>\n",
            margin - 25.0,
            margin + chart_h / 2.0,
            margin - 25.0,
            margin + chart_h / 2.0,
        ));

        svg.push_str("</svg>\n");
        svg
    }

    fn escape_html(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
}

fn format_alert(alert: &PerformanceAlert) -> String {
    match alert {
        PerformanceAlert::FrameTimeRegression {
            current,
            baseline,
            percent_increase,
        } => format!(
            "⚠️ Frame time regression: {:.2}ms → {:.2}ms ({:+.1}%)",
            baseline.as_secs_f64() * 1000.0,
            current.as_secs_f64() * 1000.0,
            percent_increase,
        ),
        PerformanceAlert::DrawCallSpike { current, baseline } => {
            format!("⚠️ Draw call spike: baseline {baseline:.0} → current {current}")
        }
        PerformanceAlert::ExcessivePipelineSwitches { count } => {
            format!("⚠️ Excessive pipeline switches: {count}")
        }
        PerformanceAlert::HighMemoryBandwidth { estimated_gbps } => {
            format!("⚠️ High memory bandwidth: {estimated_gbps:.2} GB/s")
        }
    }
}

// ---------------------------------------------------------------------------
// Embedded CSS
// ---------------------------------------------------------------------------

const DASHBOARD_CSS: &str = r#"
:root { --bg: #f8f9fa; --fg: #212529; --accent: #2a9d8f; --warn: #e76f51; }
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: system-ui, -apple-system, sans-serif;
       max-width: 1200px; margin: 0 auto; padding: 1rem;
       color: var(--fg); background: var(--bg); }
h1 { margin-bottom: 1rem; }
h2 { margin: 1rem 0 0.5rem; }
section { margin-bottom: 1.5rem; }
table { border-collapse: collapse; width: 100%; margin-top: 0.25rem; }
th, td { padding: 0.35rem 0.75rem; border: 1px solid #dee2e6; text-align: left; }
th { background: #e9ecef; }
.alert { color: var(--warn); padding: 0.25rem 0; }
.alerts ul { list-style: none; }
svg { max-width: 100%; height: auto; }
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::{ProfilingConfig, RenderPassTiming};
    use std::time::Duration;

    /// Build a profiler seeded with synthetic frames for testing.
    fn make_test_profiler() -> PerformanceProfiler {
        // We need a real wgpu device — use the test helper.
        let device = pollster::block_on(create_test_device());

        let config = ProfilingConfig {
            enable_gpu_timing: false,
            history_size: 120,
            ..Default::default()
        };
        let mut profiler = PerformanceProfiler::new(&device, config).unwrap();

        // Record a few synthetic frames
        for i in 0..5 {
            profiler.begin_frame();
            profiler.record_render_pass(RenderPassTiming {
                label: Some(format!("geometry_pass_{i}")),
                cpu_time: Duration::from_micros(800 + i as u64 * 100),
                gpu_time: None,
                draw_calls: 4 + i as u32,
            });
            profiler.record_render_pass(RenderPassTiming {
                label: Some("post_process".into()),
                cpu_time: Duration::from_micros(200),
                gpu_time: None,
                draw_calls: 1,
            });
            profiler.record_buffer_upload(Duration::from_micros(50));
            profiler.record_pipeline_switch();
            profiler.end_frame(Duration::from_micros(1200 + i as u64 * 100));
        }

        profiler.record_baseline("v0.1.0");
        profiler
    }

    /// Minimal wgpu device creation for unit tests.
    async fn create_test_device() -> wgpu::Device {
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
    }

    // -- JSON tests ---------------------------------------------------------

    #[test]
    fn test_json_export_full() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let json = exporter.to_json(&ExportConfig::default()).unwrap();

        // Should contain aggregate and frame data
        assert!(json.contains("\"aggregate\""));
        assert!(json.contains("\"frames\""));
        assert!(json.contains("geometry_pass_0"));
    }

    #[test]
    fn test_json_export_aggregate_only() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let config = ExportConfig {
            granularity: ExportGranularity::Aggregate,
            ..Default::default()
        };
        let json = exporter.to_json(&config).unwrap();

        assert!(json.contains("\"aggregate\""));
        assert!(!json.contains("\"frames\""));
    }

    #[test]
    fn test_json_export_per_frame_only() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let config = ExportConfig {
            granularity: ExportGranularity::PerFrame,
            ..Default::default()
        };
        let json = exporter.to_json(&config).unwrap();

        assert!(!json.contains("\"aggregate\""));
        assert!(json.contains("\"frames\""));
    }

    // -- CSV tests ----------------------------------------------------------

    #[test]
    fn test_csv_export() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let csv = exporter.to_csv(&ExportConfig::default()).unwrap();

        let lines: Vec<&str> = csv.lines().collect();
        // Header + 5 data rows
        assert_eq!(lines.len(), 6);
        assert!(lines[0].contains("cpu_time_ms"));
        assert!(lines[1].starts_with('0'));
    }

    // -- Chrome Trace tests -------------------------------------------------

    #[test]
    fn test_chrome_trace_export() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let trace = exporter.to_chrome_trace(&ExportConfig::default()).unwrap();

        // Must be valid JSON array
        let events: Vec<serde_json::Value> = serde_json::from_str(&trace).unwrap();
        assert!(!events.is_empty());

        // Should have frame events
        let frame_events: Vec<_> = events.iter().filter(|e| e["cat"] == "frame").collect();
        assert_eq!(frame_events.len(), 5);
    }

    // -- Flame graph tests --------------------------------------------------

    #[test]
    fn test_flame_graph_svg() {
        let profiler = make_test_profiler();
        let svg = FlameGraphGenerator::to_svg(&profiler, &FlameGraphConfig::default()).unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("geometry_pass_0"));
        assert!(svg.contains("post_process"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_flame_graph_empty_profiler() {
        let device = pollster::block_on(create_test_device());
        let config = ProfilingConfig {
            enable_gpu_timing: false,
            ..Default::default()
        };
        let profiler = PerformanceProfiler::new(&device, config).unwrap();
        let result = FlameGraphGenerator::to_svg(&profiler, &FlameGraphConfig::default());
        assert!(result.is_err());
    }

    // -- Dashboard tests ----------------------------------------------------

    #[test]
    fn test_dashboard_html() {
        let profiler = make_test_profiler();
        let html = DashboardGenerator::to_html(&profiler, &DashboardConfig::default()).unwrap();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Aggregate Statistics"));
        assert!(html.contains("Baseline Comparison"));
        assert!(html.contains("Historical Frame Times"));
        assert!(html.contains("Flame Graph"));
    }

    #[test]
    fn test_dashboard_without_flame_graph() {
        let profiler = make_test_profiler();
        let config = DashboardConfig {
            include_flame_graph: false,
            ..Default::default()
        };
        let html = DashboardGenerator::to_html(&profiler, &config).unwrap();
        assert!(!html.contains("Flame Graph"));
    }

    // -- File write tests ---------------------------------------------------

    #[test]
    fn test_export_json_file() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let dir = std::env::temp_dir().join("gup_test_export");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.json");

        exporter
            .export_json(&path, &ExportConfig::default())
            .unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"aggregate\""));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_export_csv_file() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let dir = std::env::temp_dir().join("gup_test_csv");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("profile.csv");

        exporter
            .export_csv(&path, &ExportConfig::default())
            .unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("cpu_time_ms"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_export_chrome_trace_file() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let dir = std::env::temp_dir().join("gup_test_trace");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trace.json");

        exporter
            .export_chrome_trace(&path, &ExportConfig::default())
            .unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let _: Vec<serde_json::Value> = serde_json::from_str(&contents).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_flame_graph_file() {
        let profiler = make_test_profiler();
        let dir = std::env::temp_dir().join("gup_test_flame");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("flamegraph.svg");

        FlameGraphGenerator::generate(&profiler, &path, &FlameGraphConfig::default()).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<svg"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_dashboard_file() {
        let profiler = make_test_profiler();
        let dir = std::env::temp_dir().join("gup_test_dash");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("dashboard.html");

        DashboardGenerator::generate(&profiler, &path, &DashboardConfig::default()).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<!DOCTYPE html>"));
        std::fs::remove_dir_all(&dir).ok();
    }

    // -- Granularity / config edge cases -----------------------------------

    #[test]
    fn test_export_no_baselines() {
        let device = pollster::block_on(create_test_device());
        let config = ProfilingConfig {
            enable_gpu_timing: false,
            ..Default::default()
        };
        let mut profiler = PerformanceProfiler::new(&device, config).unwrap();
        profiler.begin_frame();
        profiler.end_frame(Duration::from_millis(16));

        let exporter = ProfileExporter::new(&profiler);
        let json = exporter.to_json(&ExportConfig::default()).unwrap();
        // baselines key should be absent when empty
        assert!(!json.contains("\"baselines\""));
    }

    #[test]
    fn test_compact_json() {
        let profiler = make_test_profiler();
        let exporter = ProfileExporter::new(&profiler);
        let config = ExportConfig {
            pretty_print: false,
            ..Default::default()
        };
        let json = exporter.to_json(&config).unwrap();
        // Compact JSON should not contain leading newline indentation
        assert!(!json.contains("\n  "));
    }
}
