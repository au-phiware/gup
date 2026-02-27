# WebAssembly Performance Benchmarks

This document describes Gup's cross-platform performance benchmarking
infrastructure, covering the WASM benchmark tooling, browser compatibility, and
known platform-specific performance characteristics.

## Overview

Gup targets both native desktop and web (WebAssembly + WebGPU) platforms. The
benchmark infrastructure measures the same interaction-system operations on both
targets, enabling side-by-side comparison and identification of
platform-specific bottlenecks.

### Architecture

```text
┌──────────────────┐     ┌──────────────────┐
│  Native Runner   │     │   WASM Runner    │
│  (Rust binary)   │     │  (Browser page)  │
│  criterion-like  │     │  web-sys timing  │
└────────┬─────────┘     └────────┬─────────┘
         │                        │
         ▼                        ▼
   native.json              wasm.json
         │                        │
         └───────────┬────────────┘
                     ▼
         benchmark_comparison.sh
                     │
                     ▼
          comparison_report.md
```

**Components:**

| Component | Location | Purpose |
|---|---|---|
| `wasm_bench` module | `src/wasm_bench.rs` | Timing harness with `Performance.now()` |
| Interaction benchmarks | `src/wasm_bench_interaction.rs` | Point, region, batch query benchmarks |
| Native runner binary | `src/bin/wasm_bench_native.rs` | Produces native JSON baseline |
| HTML benchmark page | `benches/wasm/index.html` | Browser-based benchmark runner |
| Orchestration script | `scripts/wasm_benchmark.sh` | Build, run, serve, compare |
| Comparison script | `scripts/benchmark_comparison.sh` | JSON → Markdown report |

## Quick Start

### Run Native Benchmarks

```bash
# Run native benchmarks and save JSON results
./scripts/wasm_benchmark.sh native

# Or directly:
cargo run --release --bin wasm_bench_native > native_results.json
```

### Run WASM Benchmarks

```bash
# Build WASM package and start a local server
./scripts/wasm_benchmark.sh serve

# Or step by step:
wasm-pack build --target web --out-dir benches/wasm/pkg --release
# Then open benches/wasm/index.html in a WebGPU-enabled browser
```

In the browser, click **Run Benchmarks**, then **Download JSON** to save
results.

### Generate Comparison Report

```bash
./scripts/wasm_benchmark.sh compare native_results.json wasm_results.json

# Output: Markdown report with side-by-side performance table
```

### Full Workflow

```bash
# Runs native benchmarks, builds WASM, shows next steps
./scripts/wasm_benchmark.sh all
```

## Benchmarks Included

The WASM benchmark suite mirrors the native criterion benchmarks from
`benches/interaction_benchmarks.rs` (GUP-077):

### Point Queries

| Benchmark | Dataset | Pattern | Query Point |
|---|---|---|---|
| `point_queries/grid/1000` | 1K points | Uniform grid | (500, 500) |
| `point_queries/grid/10000` | 10K points | Uniform grid | (500, 500) |
| `point_queries/clustered/1000` | 1K points | 10 clusters | (125, 166) |
| `point_queries/clustered/10000` | 10K points | 10 clusters | (125, 166) |

### Region Queries

| Benchmark | Dataset | Region Size | Coverage |
|---|---|---|---|
| `region_queries/small_region_10k` | 10K grid | 100×100 | ~1% |
| `region_queries/medium_region_10k` | 10K grid | 300×300 | ~10% |
| `region_queries/large_region_10k` | 10K grid | 700×700 | ~50% |

### Batch Queries

| Benchmark | Dataset | Query Count |
|---|---|---|
| `batch_queries/single_query_10k` | 10K grid | 1 |
| `batch_queries/batch_5_queries_10k` | 10K grid | 5 |
| `batch_queries/batch_10_queries_10k` | 10K grid | 10 |

## Browser Compatibility Matrix

### WebGPU Support Status

| Browser | WebGPU | Notes |
|---|---|---|
| Chrome 113+ | ✅ Stable | Full WebGPU support since May 2023 |
| Chrome Canary | ✅ Stable | Latest features |
| Edge 113+ | ✅ Stable | Chromium-based, same as Chrome |
| Firefox Nightly | 🔧 Behind flag | `dom.webgpu.enabled` in about:config |
| Firefox Stable | ❌ Not available | Expected in future releases |
| Safari 18+ (macOS) | ✅ Stable | WebGPU support since macOS Sequoia |
| Safari (iOS 18+) | ✅ Stable | Metal-backed WebGPU |

### Launching with WebGPU Enabled

```bash
# Using the project's Chromium wrapper (recommended)
chromium-webgpu --app=http://localhost:8081

# Manual Chrome flags
google-chrome --enable-features=WebGPU,Vulkan \
              --enable-unsafe-webgpu \
              --disable-dawn-features=disallow_unsafe_apis

# Firefox Nightly
# Set dom.webgpu.enabled = true in about:config
```

### Known Browser-Specific Limitations

#### Chrome / Chromium

- **Buffer mapping**: Async buffer mapping adds latency vs native synchronous
  readback.
- **Compute shader limits**: `maxComputeWorkgroupSizeX` may be lower than
  native (typically 256 on most GPUs).
- **Timestamp queries**: Not available in all configurations; benchmarks use
  `Performance.now()` instead.

#### Firefox

- **Early implementation**: WebGPU support is experimental and incomplete.
- **Storage buffer limits**: May have lower `maxStorageBufferBindingSize`.
- **Validation overhead**: Debug validation layers are always active in Nightly.

#### Safari

- **Metal backend only**: Uses Metal backend, which may have different
  performance characteristics compared to Vulkan/DX12 on other platforms.
- **Buffer size limits**: Lower maximum buffer sizes compared to desktop
  browsers.
- **Compute shader support**: Full support, but different workgroup size
  optimal values.

## Platform-Specific Performance Characteristics

### Expected Performance Differences

| Factor | Impact | Notes |
|---|---|---|
| **WASM overhead** | 1.1-2× | WebAssembly vs native code execution overhead |
| **JS ↔ WASM boundary** | Variable | Each wasm-bindgen call has marshalling cost |
| **GPU command submission** | 1-3× | Browser WebGPU adds validation layer overhead |
| **Buffer readback** | 2-5× | Async mapping pipeline vs native staging buffers |
| **Memory allocation** | 1.5-2× | WASM linear memory + GC pressure in browser |

### Interaction System Specifics

The interaction system's performance profile on WASM is dominated by:

1. **GPU buffer creation and upload** — Creating Selection and InteractionSystem
   objects triggers buffer allocations that go through the browser's WebGPU
   implementation.
2. **Compute shader dispatch** — The spatial index and hit-testing compute
   shaders run at near-native GPU speed, since the actual GPU work is identical.
3. **Result readback** — Reading query results back from GPU memory is the
   primary bottleneck on WASM, since buffer mapping is asynchronous and goes
   through the browser's IPC mechanism.

### Optimisation Opportunities

- **Batch queries** should show the most benefit over individual queries on
  WASM, since they amortise the buffer readback overhead across multiple
  queries.
- **Large datasets** (100K+) may be limited by WASM linear memory growth and
  buffer upload times.
- **Cached queries** (repeated queries on the same data) should approach native
  speed since GPU-side caching is platform-agnostic.

## JSON Result Format

Both native and WASM benchmarks produce JSON in the `BenchSuite` format:

```json
{
  "platform": "wasm",
  "timestamp": "2025-01-15T10:00:00Z",
  "results": [
    {
      "name": "point_queries/grid/1000",
      "iterations": 10,
      "total_ms": 50.0,
      "mean_ms": 5.0,
      "min_ms": 4.0,
      "max_ms": 6.0,
      "median_ms": 5.0,
      "std_dev_ms": 0.5
    }
  ],
  "user_agent": "Mozilla/5.0 ..."
}
```

## Adding New Benchmarks

To add a new benchmark to the WASM suite:

1. Add the benchmark function in `src/wasm_bench_interaction.rs`, following the
   existing pattern of warmup + timed measurement loop.
2. Call it from `run_interaction_benchmarks()`.
3. The native runner binary (`wasm_bench_native`) and WASM export
   (`run_wasm_benchmarks`) will automatically include the new benchmark.
4. Update the HTML page's table categories in `benches/wasm/index.html` if
   adding a new category.

## Known WASM Build Limitations

The full library does not yet compile cleanly for `wasm32-unknown-unknown` due
to pre-existing issues in the accessibility and DOM integration modules
(e.g. `LinuxAccessibility`, `TouchEvent`, `Send`/`Sync` bounds on DOM
callbacks). The benchmark-related modules (`wasm_bench`, `wasm_bench_interaction`)
compile correctly under the WASM target.

A full WASM build will require platform-gating the accessibility backends and
DOM integration code, which is tracked separately from the benchmarking
infrastructure.

## Troubleshooting

### "Failed to create context" in WASM

WebGPU is not available or not enabled in the browser. Check:

- Browser version supports WebGPU (see compatibility matrix above)
- Launch with appropriate flags (e.g. `chromium-webgpu`)
- Check `chrome://gpu` for WebGPU status

### WASM module fails to load

Ensure you built with `wasm-pack build --target web`. The HTML page expects the
package at `benches/wasm/pkg/gup.js`.

### Benchmark results show high variance

GPU benchmarks inherently have more variance than CPU benchmarks. The harness
uses 3 warmup iterations and 10 measured iterations by default. For more stable
results:

- Close other GPU-intensive applications
- Run benchmarks in a dedicated browser window
- Increase iteration count if needed (modify `BenchConfig::default()`)

### Native binary shows "Failed to create context"

The native runner needs GPU access. Ensure a display server is running or use a
headless GPU environment.
