# GUP-297: GPU Compute Shader 2D Binning

## Story Overview

**Initiative**: Performance  
**Status**: 📋 Planned  
**Created**: 2026-03-03

## Context

GUP-248 (Heatmap Chart Builder) implements CPU-side 2D binning in
`BinGrid::from_data()`. For modest datasets (up to ~1M rows) this is performant,
but for very large flat datasets (10M+ rows) the CPU binning loop may become a
bottleneck, especially in real-time streaming scenarios.

This story moves the 2D binning aggregation to a wgpu compute shader, keeping
the main thread free. The `BinGrid` interface remains the same — the GPU path is
an alternative backend selected via a feature flag or runtime heuristic.

## User Story

> "As a performance-sensitive developer working with 10M+ row datasets, I want
> the heatmap binning to run on the GPU so that the main thread stays responsive
> for user interaction."

## Acceptance Criteria

- [ ] A compute shader performs 2D binning with atomicAdd for Count and Sum
      aggregation modes.
- [ ] Mean, Min, and Max aggregation use appropriate atomic operations or
      multi-pass strategies.
- [ ] Results are read back into a `BinGrid` compatible with the existing
      rendering pipeline.
- [ ] Binning of 10M records into a 100×100 grid completes in under 50 ms on a
      mid-range discrete GPU.
- [ ] CPU fallback is used automatically when compute shaders are unavailable.
- [ ] Round-trip test: GPU-binned results match CPU-binned results within
      floating-point tolerance.

## Technical Tasks

- [ ] Create `src/chart_builder/builders/heatmap/gpu_binning.rs`.
- [ ] Write compute shader for 2D binning with workgroup size 256.
- [ ] Implement buffer upload, dispatch, and readback.
- [ ] Add CPU/GPU equivalence tests.
- [ ] Wire into `HeatmapBuilder` with a `.gpu_binning(true)` option.

## Dependencies

### Prerequisite Stories

- GUP-248: Heatmap Chart Builder ✅ — provides `BinGrid`, `BinSpec`, and the CPU
  binning baseline.

## Testing Strategy

- Equivalence tests: CPU and GPU binning produce identical results.
- Performance benchmarks: GPU vs CPU for 1M, 10M, 100M row datasets.

## Risk Assessment

- **Medium**: Atomic operations in compute shaders have driver-specific
  performance characteristics. May need multiple dispatch strategies.
- **Low**: Readback latency may add overhead for small datasets. The runtime
  heuristic should prefer CPU for small inputs.

## Definition of Done

- [ ] All acceptance criteria met
- [ ] CPU/GPU equivalence tests pass
- [ ] Performance benchmarks documented
- [ ] Feature flag or auto-detection implemented
