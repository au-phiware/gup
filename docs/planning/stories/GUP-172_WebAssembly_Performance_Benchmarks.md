# GUP-172: WebAssembly Performance Benchmarks

**Priority**: Low **Complexity**: Medium **Created**: 2025-08-06 **Status**: ✅
Complete **Completed**: 2025-08-07

## Overview

Create headless browser benchmarking infrastructure to measure and compare
WebGPU/WASM performance against native GPU performance. This enables
cross-platform performance validation and identifies platform-specific
optimization opportunities.

## Context

GUP-077 established a comprehensive native benchmark suite for the interaction
system. However, the cross-platform story is incomplete without WebAssembly
benchmarks that measure the same operations in a browser environment.

## User Story

As a developer targeting web deployment, I want to see how interaction system
performance compares between native and WASM builds, so that I can set realistic
performance expectations and identify platform-specific bottlenecks.

## Acceptance Criteria

- [x] Headless browser benchmark runner (Chromium with WebGPU)
- [x] WASM-compiled versions of point, region, and batch query benchmarks
- [x] Side-by-side native vs WASM performance comparison report
- [x] Documentation of platform-specific performance characteristics
- [x] Browser compatibility matrix (Chrome, Firefox, Safari WebGPU support)

## Technical Tasks

- [x] Set up wasm-pack benchmark compilation pipeline
- [x] Create browser-side benchmark harness using web-sys timing APIs
- [x] Port interaction benchmark dataset generators to WASM-compatible code
- [x] Implement automated comparison report generation
- [x] Document known WebGPU limitations per browser

## Dependencies

- **Requires**: GUP-077 (native benchmark baseline)
- **Related**: GUP-020 (WebGPU Integration RenderContext)

## Testing Strategy

- Validate WASM benchmarks produce consistent results across runs
- Compare against native baselines from GUP-077

## Risk Assessment

- **Medium**: WebGPU support varies significantly across browsers
- **Low**: WASM compilation may require benchmark code adjustments

## Definition of Done

- [x] WASM benchmarks compile and run in headless Chromium
- [x] Comparison report generated automatically
- [x] Documentation complete

## Implementation Summary

### What Was Implemented

1. **WASM Benchmark Harness** (`src/wasm_bench.rs`): Lightweight benchmark
   runner using `web_sys::Performance.now()` for timing on WASM and
   `std::time::SystemTime` on native. Provides `BenchResult`, `BenchSuite`,
   `BenchConfig`, `Timer`, `run_bench`, `run_bench_async`, and `from_timings`.

2. **Interaction Benchmarks** (`src/wasm_bench_interaction.rs`): WASM-compatible
   port of the native criterion benchmarks from `benches/interaction_benchmarks.rs`:
   - 4 point query benchmarks (grid/clustered × 1K/10K)
   - 3 region query benchmarks (small/medium/large coverage)
   - 3 batch query benchmarks (single/5/10 queries)
   - Native JSON runner via `run_native_benchmarks()`
   - WASM entry point via `run_wasm_benchmarks()` (wasm_bindgen export)

3. **Native Runner Binary** (`src/bin/wasm_bench_native.rs`): Produces JSON
   results in the same `BenchSuite` format as the WASM runner.

4. **HTML Benchmark Page** (`benches/wasm/index.html`): Interactive browser-based
   runner with results tables, JSON download, and clipboard copy.

5. **Automation Scripts**:
   - `scripts/wasm_benchmark.sh`: Build, native run, serve, compare, all-in-one
   - `scripts/benchmark_comparison.sh`: Python-based Markdown report generator

6. **Documentation** (`docs/WASM_PERFORMANCE.md`): Architecture overview, quick
   start, benchmark catalog, browser compatibility matrix, platform-specific
   performance characteristics, JSON format specification, and troubleshooting.

7. **WASM Build Fixes**: Split tokio features for WASM compatibility
   (rt-multi-thread on native, limited features on WASM), added uuid `js`
   feature for WASM.

### Key Files Changed

| File | Change |
|---|---|
| `src/wasm_bench.rs` | New: benchmark harness (7 tests) |
| `src/wasm_bench_interaction.rs` | New: interaction benchmarks (3 tests) |
| `src/bin/wasm_bench_native.rs` | New: native JSON runner binary |
| `src/lib.rs` | Added wasm_bench modules |
| `Cargo.toml` | Performance feature, WASM deps, binary |
| `benches/wasm/index.html` | New: browser benchmark UI |
| `scripts/wasm_benchmark.sh` | New: orchestration script |
| `scripts/benchmark_comparison.sh` | New: comparison report generator |
| `docs/WASM_PERFORMANCE.md` | New: comprehensive documentation |
| `docs/README.md` | Added WASM performance docs link |
| `maskfile.md` | Added bench-wasm-* tasks |

### Test Count

- 10 new tests (7 harness + 3 interaction data generators)
- All pass on native target
