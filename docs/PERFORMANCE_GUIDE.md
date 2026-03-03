# Performance Guide

This document describes Gup's performance characteristics, optimization
guidelines, and benchmarking infrastructure. It serves as a reference for
developers working on or with Gup.

## Performance Targets

Gup's Phase 1 performance targets define the minimum acceptable performance for
a production-ready GPU-accelerated visualization library.

| Metric                      | Target                                 | Measured                                     |
| --------------------------- | -------------------------------------- | -------------------------------------------- |
| Rendering throughput        | 100K points at 60 FPS (≤16.67ms frame) | ~4.4ms for 100K points (debug build)         |
| Interaction latency         | <1ms hit testing for large datasets    | ~10ms in debug; meets target in release      |
| Shader composition overhead | <5% vs hand-optimized WGSL             | <5% GPU execution (see criterion benchmarks) |
| Memory scaling              | Linear with data size                  | R²=1.0 (perfectly linear)                    |

> **Note**: Debug builds are significantly slower than release builds due to
> bounds checking and lack of optimizations. The targets above apply to
> **release builds**. Debug thresholds are relaxed (see
> `PerformanceTargets::debug()`).

## Architecture Overview

### Rendering Pipeline

The rendering pipeline consists of:

1. **Data mapping** (CPU): Convert domain data to GPU instance structs (e.g.
   `CircleInstance`).
2. **Buffer upload** (CPU→GPU): Write instance data to GPU storage buffers via
   `queue.write_buffer()`.
3. **Pipeline setup** (GPU): Set render pipeline, bind groups, and vertex
   buffers.
4. **Draw call** (GPU): Execute instanced rendering (one draw call per
   Selection).

The most expensive CPU-side operation is **data mapping** — iterating over all
data points and converting them to GPU-aligned instance structs. This is
measured by `Selection::prepare_render()`.

### Interaction Pipeline

Hit testing uses GPU compute shaders for parallel spatial queries:

1. **Spatial index build** (GPU compute): Build a grid-based spatial index.
2. **Query dispatch** (GPU compute): Test query point/region against all
   elements in parallel.
3. **Result readback** (GPU→CPU): Map result buffer and read hit indices.

The first query includes index build overhead. Subsequent queries reuse the
cached spatial index and are significantly faster.

### Shader Composition

Shader functions compose via WGSL code generation:

1. **WGSL generation** (CPU): Each `ComposableShaderFunction` generates a WGSL
   snippet.
2. **Pipeline compilation** (GPU): The combined WGSL is compiled into a render
   pipeline.
3. **Execution** (GPU): The composed shader runs on the GPU with no additional
   overhead vs a hand-written equivalent.

Pipeline compilation is the expensive step — cache pipelines via `PipelineCache`
to avoid re-compilation.

## Performance-Critical Code Paths

### Buffer Upload

`queue.write_buffer()` is the primary bottleneck for dynamic data. To minimize
overhead:

- **Batch uploads**: Prepare all instance data before uploading.
- **Minimize data size**: Use the smallest possible instance struct. GPU
  alignment padding is unavoidable (e.g. `CircleInstance` is 64 bytes for 48
  bytes of useful data).
- **Reuse buffers**: Use `BufferPool` to avoid per-frame buffer allocation.

### Pipeline Creation

Pipeline creation involves GPU shader compilation and is expensive (>1ms).

- **Cache pipelines**: Use `PipelineCache` to share pipelines across Selections
  of the same mark type.
- **Pre-create**: Build pipelines during initialization, not during the render
  loop.

### Spatial Index

The spatial index is built once per dataset change and cached.

- **Grid resolution**: Default grid cell size is tuned for uniform
  distributions. Highly clustered data may benefit from a different spatial
  index.
- **Query types**: Point queries are fastest; region queries scale with the
  number of cells intersected.

## Benchmarking Infrastructure

### Criterion Benchmarks

Gup includes 24 criterion benchmark suites in `benches/`. Run them with:

```bash
cargo bench
```

Key benchmark groups:

| Group                         | Measures                                        |
| ----------------------------- | ----------------------------------------------- |
| `shader_composition`          | Composed vs hand-optimized shader GPU execution |
| `bar_chart_build`             | 100K row chart construction                     |
| `interaction_benchmarks`      | Point/region query latency                      |
| `spatial_index_benchmarks`    | Index build and query performance               |
| `mark_performance_benchmarks` | Mark rendering at various scales                |
| `buffer_benchmarks`           | GPU buffer upload throughput                    |
| `composition_benchmarks`      | Mixable composition overhead                    |

### Performance Regression Tests

Threshold-based tests in `tests/interaction_performance_tests.rs` and
`tests/performance_validation_tests.rs` enforce absolute performance limits.

Run with:

```bash
cargo test --test performance_validation_tests -- --test-threads=1
cargo test --test interaction_performance_tests -- --test-threads=1
```

### CI Integration

Performance testing integrates with CI via:

- **`perf-thresholds.toml`**: Regression thresholds per benchmark group.
- **`tests/performance_ci_tests.rs`**: CI performance runner with baseline
  management.
- **`scripts/benchmark_baseline.sh`**: Save, compare, list, and reset baselines.
- **`scripts/perf_alert.sh`**: Alert on regressions exceeding thresholds.

### Baseline Management

```bash
# Save current benchmark results as a named baseline
./scripts/benchmark_baseline.sh save v0.1.0

# Compare current results against a baseline
./scripts/benchmark_baseline.sh compare v0.1.0

# List available baselines
./scripts/benchmark_baseline.sh list
```

## Profiling

### GPU Profiling

The `PerformanceProfiler` in `src/performance.rs` provides frame-level GPU
profiling:

```rust
use gup::performance::{PerformanceProfiler, ProfilingConfig, RenderPassTiming};

let config = ProfilingConfig {
    enable_gpu_timing: true,
    enable_regression_detection: true,
    ..Default::default()
};
let mut profiler = PerformanceProfiler::new(&device, config)?;

// Per frame:
profiler.begin_frame();
profiler.record_render_pass(RenderPassTiming {
    label: Some("main".to_string()),
    cpu_time: frame_cpu_time,
    gpu_time: None, // filled by timestamp queries if available
    draw_calls: 1,
});
profiler.end_frame(total_cpu_time);

// Aggregate stats:
let stats = profiler.aggregate_stats();
println!("avg: {:?}, p95: {:?}", stats.avg_cpu_time, stats.p95_frame_time);
```

### Bottleneck Analysis

The `BottleneckAnalyzer` in `src/performance_targets.rs` identifies hotspots
from profile data:

```rust
use gup::performance_targets::{BottleneckAnalyzer, ProfileData};

let analyzer = BottleneckAnalyzer::default();
let bottlenecks = analyzer.identify(&profile_data);
for b in &bottlenecks {
    println!("{}", b); // e.g. "[Critical] GPU:fragment_shader — 56.2% of frame (9ms)"
}
```

### Debug Tools

The `debug` module (`src/debug/`) provides:

- **ShaderProfiler**: GPU timestamp query-based shader timing.
- **GpuMemoryProfiler**: Allocation tracking and leak detection.
- **GpuBufferInspector**: Buffer content dumping to JSON.
- **LayoutValidator**: Rust↔WGSL struct alignment verification.
- **ResourceGraph**: Dependency tracking between GPU resources.

## Optimization Tips

### For Application Developers

1. **Use `PipelineCache`**: Always pass a shared `PipelineCache` to
   `prepare_render()` to avoid redundant pipeline compilation.
2. **Prefer `attr_parallel`**: Bind multiple attributes in one pass to halve
   iteration overhead.
3. **Batch data updates**: Update all data points at once rather than individual
   elements.
4. **Use `BufferPool`**: For frequently recreated Selections, pool instance
   buffers.
5. **Minimize data size**: Only include the data fields needed for
   visualization.

### For Library Developers

1. **Profile first**: Use `PerformanceProfiler` and `BottleneckAnalyzer` to
   identify hotspots before optimizing.
2. **GPU over CPU**: Move computation to GPU shaders when possible. GPU
   parallelism scales better than CPU iteration.
3. **Single render pass**: Never create multiple render passes from one command
   encoder.
4. **Workgroup size 256**: Standard for compute shaders; matches most GPU
   hardware.
5. **WGSL alignment**: Always validate struct alignment with
   `std::mem::offset_of!()` and `#[repr(C)]` + `bytemuck::Pod`.

## Cross-Platform Considerations

Performance varies across platforms and GPU hardware:

| Platform       | Expected Variance | Notes                           |
| -------------- | ----------------- | ------------------------------- |
| Linux (Vulkan) | Baseline          | Best raw performance            |
| Windows (DX12) | ±10%              | Driver overhead varies          |
| macOS (Metal)  | ±15%              | Different GPU architecture      |
| WebAssembly    | 30–50% slower     | Browser overhead + WebGPU layer |

The `tests/performance_ci_tests.rs` suite detects the current platform and
stores per-platform baselines for regression tracking.
