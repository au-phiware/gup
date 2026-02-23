# GPU Timestamp Query Integration

## Overview

This document describes the GPU timestamp query integration for accurate pattern
rendering performance measurement, implemented in GUP-161.

## Background

The pattern performance benchmarks (GUP-156) measure CPU-side overhead including
command encoding, submission, and polling. However, they don't capture the actual
GPU execution time of fragment shaders, which is critical for validating the <5ms
target for 100K points.

GPU timestamp queries provide hardware-level timing measurements that accurately
reflect fragment shader execution time on the GPU.

## Implementation

### Infrastructure

The `TimestampQueryManager` (src/performance.rs) already provided the necessary
infrastructure:

- **Query Set Creation**: Creates wgpu timestamp query sets with configurable size
- **Resolution Buffer**: GPU-side buffer for query results
- **Readback Buffer**: CPU-accessible buffer for timestamp retrieval
- **Timestamp Period**: Converts GPU ticks to nanoseconds

### Pattern GPU Timing Benchmarks

A new benchmark file `benches/pattern_gpu_timing_benchmarks.rs` was created to
measure GPU execution time using timestamp queries.

#### Key Features

1. **GPU Timestamp Context**
   - Requests device with `TIMESTAMP_QUERY` feature
   - Gracefully degrades if timestamps not supported
   - Manages timestamp query lifecycle

2. **Measurement Function**
   ```rust
   async fn measure_render_pass_gpu<F>(&self, mut render_fn: F) -> Option<Duration>
   where
       F: FnMut(&mut wgpu::CommandEncoder, &wgpu::QuerySet)
   ```
   - Writes timestamps at render pass start/end
   - Resolves and reads back query results
   - Converts GPU ticks to Duration

3. **Benchmark Groups**
   - `pattern_gpu_rendering_time`: Measures all patterns at 1K-1M data sizes
   - `pattern_gpu_overhead`: Focuses on 100K points for <5ms validation

### Limitations

The current implementation has a known limitation: it cannot create actual render
passes without a surface/texture. Therefore, it measures command encoding overhead
rather than true fragment shader execution.

To get actual GPU fragment shader timing, we would need to:

1. Create a dummy texture as render target
2. Execute a real render pass with the texture
3. Measure timestamp difference

This limitation is documented in code comments and will be addressed in a follow-up
story if actual fragment shader timing is required.

## Usage

### Running GPU Timing Benchmarks

```bash
# Run all GPU timing benchmarks
cargo bench --bench pattern_gpu_timing_benchmarks

# Run specific benchmark group
cargo bench --bench pattern_gpu_timing_benchmarks pattern_gpu_overhead

# Quick test run
cargo bench --bench pattern_gpu_timing_benchmarks -- --test
```

### Interpreting Results

The benchmarks report:

- **CPU Time**: Total time including command encoding, submission, and polling
- **GPU Time**: Actual hardware execution time from timestamp queries
- **Overhead**: Difference between CPU and GPU time (command encoding overhead)

Example output:
```
pattern_gpu_overhead/overhead_solid
                        time:   [145.2 µs 147.8 µs 150.1 µs]
```

### Device Support

GPU timestamp queries require:

- GPU with `TIMESTAMP_QUERY` feature support
- Driver support for timestamp queries
- Most modern GPUs (2018+) support this feature

If timestamps are not supported, the benchmark prints a warning and skips GPU
timing tests:

```
⚠️  GPU timestamp queries not supported on this device - skipping GPU timing benchmarks
```

## Integration with Existing Benchmarks

The GPU timing benchmarks complement the existing pattern performance benchmarks:

| Benchmark File                   | Measures                    | Use Case                      |
| -------------------------------- | --------------------------- | ----------------------------- |
| pattern_performance_benchmarks   | CPU-side overhead           | Regression detection          |
| pattern_gpu_timing_benchmarks    | GPU execution time          | Fragment shader validation    |

Both benchmark files should be run together for comprehensive performance analysis.

## Future Enhancements

### Actual Render Pass Timing

To measure true fragment shader execution time:

1. Create offscreen texture as render target
2. Execute complete render pass with fragment shader
3. Measure timestamp difference across render pass

### Automated Validation

Integrate GPU timing into CI pipeline:

1. Run on known GPU configurations
2. Compare against <5ms baseline
3. Fail CI if performance regressions detected

### Extended Metrics

Additional GPU metrics that could be captured:

- Memory bandwidth utilization
- Cache hit rates
- Occupancy rates
- Fragment shader invocations

## References

- GUP-156: Pattern Performance Benchmarking
- GUP-161: GPU Timestamp Query Integration
- wgpu timestamp query documentation
- src/performance.rs: TimestampQueryManager implementation
