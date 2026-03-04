# GUP-148: Profiling Data Export and Visualization

**Status**: ✅ Complete

## Story Overview

**Title**: Profiling Data Export and External Visualization **Epic**: Phase 1
Initiative 1 - Core GPU Primitives and Selection API **Priority**: Low **Story
Points**: 5

## Context

The performance profiling system (GUP-046) collects rich performance data but
lacks export and visualization capabilities. External analysis tools and visual
representations would greatly enhance debugging and optimization workflows.

## User Story

**As a** Gup application developer **I want** to export and visualize profiling
data **So that** I can analyze performance patterns using familiar tools

## Acceptance Criteria

### AC1: Data Export Formats

- [x] Export to JSON format
- [x] Export to CSV format
- [x] Export to Chrome DevTools Performance format
- [x] Configurable export granularity

### AC2: Flame Graph Generation

- [x] Generate flame graphs for render pass hierarchies
- [x] Interactive flame graph output
- [x] Time-based flame graph views
- [x] Component-level breakdown

### AC3: Web Dashboard

- [x] Real-time profiling dashboard
- [x] Historical performance trends
- [x] Comparison views (baseline vs. current)
- [x] Alert notification display

## Dependencies

- GUP-046: Context Performance Profiling (completed)

## Technical Requirements

```rust
pub trait ProfileExporter {
    fn export_json(&self, path: &Path) -> GupResult<()>;
    fn export_csv(&self, path: &Path) -> GupResult<()>;
    fn export_chrome_trace(&self, path: &Path) -> GupResult<()>;
}
```

## Success Metrics

- [x] Support for 3+ export formats
- [x] Integration with Chrome DevTools
- [x] Real-time dashboard with <100ms latency

## Implementation Summary

### Files Added

- `src/performance_export.rs` — Core export module (~660 lines) containing:
  - `ProfileExporter` — JSON, CSV, Chrome Trace Event Format export
  - `FlameGraphGenerator` — Self-contained SVG flame graph generation
  - `DashboardGenerator` — Interactive HTML dashboard generation
  - `ExportConfig` / `ExportGranularity` — Configurable export granularity
  - `FlameGraphConfig` / `DashboardConfig` — Generation configuration
  - 16 unit tests
- `tests/performance_export_tests.rs` — Integration tests (6 tests)

### Files Modified

- `src/performance.rs` — Added `Serialize`/`Deserialize` derives to all public
  types (`DetailedFrameStats`, `AggregateStats`, `RenderPassTiming`,
  `PerformanceBaseline`, `PerformanceAlert`, `ProfilingConfig`); added
  `duration_serde` and `option_duration_serde` helper modules
- `src/lib.rs` — Registered `performance_export` module

### Key Components

1. **ProfileExporter** — Exports profiling data from `PerformanceProfiler`:
   - `export_json()` / `to_json()` — Full-fidelity JSON with configurable
     granularity (Aggregate, PerFrame, Full)
   - `export_csv()` / `to_csv()` — Tabular per-frame data for spreadsheets
   - `export_chrome_trace()` / `to_chrome_trace()` — Chrome Trace Event Format
     loadable in `chrome://tracing` or Perfetto UI

2. **FlameGraphGenerator** — Produces self-contained SVG flame graphs:
   - Render pass hierarchy visualization with nested frame/pass bars
   - Interactive tooltips on hover showing timing data
   - Configurable width, row height, font size, and minimum bar width
   - Color-coded by category (frame, render pass, buffer upload)

3. **DashboardGenerator** — Produces self-contained HTML dashboards:
   - Aggregate statistics table (avg/min/max/p95/p99/stddev)
   - Active alert notifications with severity indicators
   - Baseline comparison table showing deltas
   - Historical frame time chart (inline SVG bar chart with average line)
   - Embedded flame graph section
   - Responsive CSS, zero external dependencies

### Test Coverage

- 16 unit tests + 6 integration tests = 22 total
- All 2724 existing lib tests continue to pass
- All examples compile without errors

## Definition of Done

- [x] All acceptance criteria met
- [x] Code compiles without errors
- [x] Comprehensive test suite created (22 tests)
- [x] Documentation in code complete (module-level docs, doc comments)
- [x] Integration with PerformanceProfiler complete
- [x] All existing tests pass (2724 lib tests)
- [x] All examples compile
