# Cross-Platform Axis Performance Report

**Generated on**: Linux Desktop (Vulkan) **Story**: GUP-206: Cross-Platform Axis
Performance Validation **Date**: 2025-07-19

## Executive Summary

The axis rendering system was benchmarked on the Linux Desktop platform to
establish the baseline for cross-platform comparison. All benchmarks are well
within the 1 ms per-frame performance budget (the most expensive operation,
`complete_4axis_uncached`, has a median of ~3 µs — roughly 300× faster than the
budget).

WebAssembly targets are expected to be slower due to browser WebGPU driver
overhead; the system provides relaxed thresholds (2 ms budget, more aggressive
LOD transitions) to compensate.

## Benchmark Results — Linux Desktop (Baseline)

| Benchmark                    | Median  | Min     | Max     | Iterations |
| ---------------------------- | ------- | ------- | ------- | ---------- |
| `vertex_generation_uncached` | ~0.9 µs | ~0.6 µs | ~12 µs  | 1000       |
| `vertex_generation_cached`   | ~0.4 µs | ~0.2 µs | ~0.7 µs | 1000       |
| `lod_selection`              | ~40 ns  | ~36 ns  | ~140 ns | 1000       |
| `label_generation`           | ~2.2 µs | ~2.1 µs | ~36 µs  | 1000       |
| `label_culling_100`          | ~5.7 µs | ~5.5 µs | ~22 µs  | 1000       |
| `grid_fingerprint_20`        | ~4.9 µs | ~3.3 µs | ~19 µs  | 1000       |
| `complete_4axis_uncached`    | ~3.1 µs | ~2.9 µs | ~17 µs  | 1000       |
| `complete_4axis_cached`      | ~1.3 µs | ~1.3 µs | ~16 µs  | 1000       |

> All values are approximate; exact numbers vary per run. Benchmarks use
> `cargo test -- --nocapture` for integration-test-level validation and
> `cargo bench --bench axis_performance_benchmarks` for Criterion-level
> measurement.

## Platform-Specific Tuning

### LOD Thresholds

| Threshold             | Linux / macOS / Windows | WebAssembly |
| --------------------- | ----------------------- | ----------- |
| High → Medium         | 200 px                  | 250 px      |
| Medium → Low          | 100 px                  | 130 px      |
| Low → Minimal         | 50 px                   | 65 px       |
| Performance downgrade | 5 ms                    | 3 ms        |

WebAssembly thresholds are 25–30% more aggressive to account for higher driver
overhead and lower GPU throughput in browser-hosted contexts.

### Performance Budgets

| Platform        | Target Render Time | Quality Preference |
| --------------- | ------------------ | ------------------ |
| Linux Desktop   | 1 ms               | 0.7                |
| macOS Desktop   | 1 ms               | 0.7                |
| Windows Desktop | 1 ms               | 0.7                |
| WebAssembly     | 2 ms               | 0.5                |

## Variance Analysis

### Acceptance Criterion: No >2× Variance

The maximum acceptable variance between any two platforms is 2×. This is
validated by `check_cross_platform_variance()` in the test suite.

Since all benchmarked operations are CPU-side (vertex generation, caching, label
culling, LOD selection, fingerprinting), the variance between native desktop
platforms (Linux, macOS, Windows) is expected to be minimal — likely within
1.3×. The operations do not touch the GPU pipeline.

WebAssembly targets may show higher variance due to:

- **JS/Wasm bridge overhead** for `wgpu` calls
- **Browser event loop** integration adding latency
- **Single-threaded execution** in most browser contexts
- **JIT warm-up** effects on first invocations

The 2× threshold and the relaxed WebAssembly budget (2 ms vs 1 ms) should
accommodate this.

### Cross-Platform Results

| Platform        | Status     | Notes                                     |
| --------------- | ---------- | ----------------------------------------- |
| Linux Desktop   | ✅ Tested  | All operations within 1 ms budget         |
| macOS Desktop   | 📋 Pending | Expected similar to Linux (Metal backend) |
| Windows Desktop | 📋 Pending | Expected similar to Linux (DX12/Vulkan)   |
| WebAssembly     | ✅ Ready   | Module ported (GUP-226), 2 ms budget      |

> **Note**: macOS and Windows results will be collected when the corresponding
> CI runners are enabled in `.github/workflows/performance.yml`. WebAssembly axis
> benchmarks are available via `wasm_bench_axis` module and the HTML runner at
> `benches/wasm/axis_benchmarks.html`.

## CI Integration

### Running Cross-Platform Tests

```bash
# Run on current platform
cargo test --test cross_platform_axis_performance_tests -- --test-threads=1 --nocapture

# Run Criterion benchmarks
cargo bench --bench axis_performance_benchmarks

# Run WASM axis benchmark unit tests (native, same code path)
cargo test -p gup --lib wasm_bench_axis -- --test-threads=1

# Build WASM package with axis benchmarks
wasm-pack build --target web --out-dir benches/wasm/pkg --release

# Open browser benchmark runner
# chromium-webgpu benches/wasm/axis_benchmarks.html
```

### GitHub Actions

The `performance.yml` workflow includes:

- `axis_performance` — Native axis performance tests on Linux (baseline)
- `wasm_axis_performance` — WASM compilation check, package build, and unit tests

To enable cross-platform validation:

1. Uncomment the macOS and Windows entries in the axis_performance matrix
2. Install matching chromedriver for headless browser testing in
   `wasm_axis_performance`
3. The cross-platform comparison job collects all reports and compares

### Programmatic API

```rust
use gup::axis_performance::{
    PlatformPreset, LODConfiguration, PerformanceBudget,
    PlatformBenchmarkReport, check_cross_platform_variance,
    generate_variance_report,
};

// Detect current platform
let platform = PlatformPreset::detect();

// Get platform-tuned configuration
let lod_config = LODConfiguration::for_platform(platform);
let budget = PerformanceBudget::for_platform(platform);

// Compare two platform reports
let violations = check_cross_platform_variance(&linux_report, &wasm_report, 2.0);
assert!(violations.is_empty(), "Variance exceeds 2× threshold");

// Generate a Markdown comparison table
let md = generate_variance_report(&[linux_report, wasm_report], 0);
println!("{md}");
```

### WASM Benchmark API

```rust
use gup::wasm_bench::{BenchConfig, BenchSuite};
use gup::wasm_bench_axis::run_axis_benchmarks;

// Run all 8 axis benchmarks
let config = BenchConfig { warmup_iterations: 5, measured_iterations: 50 };
let suite: BenchSuite = run_axis_benchmarks(&config);

// Serialize for cross-platform comparison
let json = serde_json::to_string_pretty(&suite)?;
```

## Future Work

1. **Enable macOS/Windows CI runners** — The workflow is ready; only runner
   labels need to be uncommented.
2. **Headless Chrome in CI** — Requires matching chromedriver version for
   `wasm-pack test --headless --chrome` integration.
3. **GPU-side benchmarks** — Current benchmarks are CPU-side only. GPU-side axis
   rendering (pipeline creation, draw calls) should be profiled separately.
4. **Historical trend tracking** — Store Criterion baselines per-platform in the
   `benchmark-history` branch for regression detection.
