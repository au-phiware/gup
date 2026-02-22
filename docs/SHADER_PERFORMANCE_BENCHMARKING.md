# Shader Function Performance Benchmarking

## Overview

This document describes the performance benchmarking methodology for Gup's
shader function composition system (GUP-137).

## Benchmark Types

### 1. GPU Execution Benchmarks (`benches/shader_performance_benchmarks.rs`)

Measures actual GPU performance using Criterion.rs:

- **Composed vs Hand-Optimized**: Compares 2-stage composed pipeline
  (LinearScale + ColorMap) against hand-optimized equivalent
- **Composition Depth**: Tests performance scaling with 2, 3, and 5-stage
  composition chains
- **WGSL Generation**: Measures CPU time to generate WGSL code from shader
  functions

**Usage:**

```bash
cargo bench --bench shader_performance_benchmarks
```

### 2. GPU Execution Tests (`tests/shader_performance_tests.rs`)

Integration tests that validate performance claims with actual GPU timing:

- **Performance Target**: Validates composed shaders are within 15% of
  hand-optimized
- **Depth Scaling**: Verifies composition scales linearly with depth

**Usage:**

```bash
cargo test --test shader_performance_tests -- --ignored --test-threads=1 --nocapture
```

## Methodology

### Test Data

- **Size**: 10,000 elements per test
- **Type**: f32 input values normalized to [0, 1]
- **Iterations**: 100 iterations averaged for stable timing

### GPU Configuration

- **Workgroup Size**: 256 threads
- **Power Mode**: High performance
- **Features**: No special features required (timestamp queries optional)

### Comparison Approach

1. **Hand-Optimized Baseline**: Write functionally equivalent WGSL with manual
   inlining
2. **Composed Version**: Generate WGSL using shader function composition system
3. **Measurement**: Wall-clock time for GPU submission + poll completion
4. **Validation**: Assert overhead ≤ 15%

## Results

### Current Performance (2025-01-10)

```
=== Shader Performance Comparison ===
Hand-optimized time: 0.003447 seconds
Composed time:       0.003510 seconds
Overhead:            1.82%
Max allowed:         15.00%
✓ Performance validation passed!
```

**Key Findings:**

1. **Composition Overhead**: 1.82% - well below 15% target
2. **Depth Scaling**: 5-stage pipeline is only 1.04x slower than 3-stage
3. **WGSL Generation**: Sub-microsecond generation time (15-19 ns)

## Performance Regression Detection

### CI Integration

The benchmarks are designed to run in CI to catch performance regressions:

1. Run benchmarks on every PR
2. Compare against baseline from main branch
3. Fail if overhead exceeds 15% threshold
4. Generate performance report artifact

### Manual Baseline

Generate a baseline for comparison:

```bash
# Generate baseline
cargo bench --bench shader_performance_benchmarks -- --save-baseline main

# Compare against baseline
cargo bench --bench shader_performance_benchmarks -- --baseline main
```

## Benchmark Architecture

### Hand-Optimized Shaders

Located in test code, these provide the performance baseline:

- **2-Stage**: LinearScale + ColorMap inlined
- **3-Stage**: Scale + Add + Output
- **5-Stage**: Scale + Add + Clamp + Square + Output

### Composed Shaders

Generated dynamically from shader function primitives:

- Uses `ComposableShaderFunction::generate_wgsl()`
- Composes multiple functions with type safety
- GPU compilation happens at runtime

## Performance Factors

### What Affects Performance

1. **Function Count**: Linear scaling with depth
2. **Uniform Data**: Minimal impact (passed as buffer)
3. **Type Conversions**: Zero cost (compile-time checks)
4. **WGSL Generation**: Negligible (happens once at setup)

### What Doesn't Affect Performance

1. **Rust Composition API**: Zero runtime cost
2. **Type Safety Checks**: Compile-time only
3. **Function Trait Overhead**: Monomorphized away

## Future Work

- Add more complex composition patterns
- Test with larger datasets (100K, 1M elements)
- Measure memory bandwidth impact
- Add statistical shader functions benchmarks
- Profile WGSL compiler optimization impact

## References

- GUP-033: Shader Function Composition Engine
- GUP-137: Shader Function Performance Benchmarking
