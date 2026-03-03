# GUP-301: GPU Density Compute Pipeline Integration

## Story Overview

**Initiative**: Chart Builders **Status**: 📋 Planned **Created**: 2025-07-15

## Context

GUP-250 delivered the `DensityPlotBuilder` with CPU-side 2D KDE computation and
marching-squares contour extraction, along with WGSL compute shaders
(`density_kde_2d.compute.wgsl` and `density_marching_squares.compute.wgsl`).
However, the compute shaders are standalone files — they are not yet wired into
the Gup GPU pipeline. The CPU path is adequate for small datasets (< 10K
points), but for 100K+ points the O(n × m²) KDE cost becomes prohibitive and
GPU dispatch is essential.

This story completes the GPU integration: creating bind groups, compute
pipelines, staging buffers, and connecting the dispatch to
`DensityPlotBuilder::build()` when the dataset size exceeds a configurable
threshold.

## User Story

> "As a visualization developer working with large datasets, I want the density
> plot to compute KDE on the GPU so that 100K+ point density plots remain
> interactive at 60 FPS."

## Acceptance Criteria

- [ ] `DensityPlotBuilder` dispatches the 2D KDE compute shader when the sample
      count exceeds a configurable threshold (default: 5,000 points)
- [ ] GPU KDE output matches CPU reference within 1% relative error for all
      three test distributions from GUP-250
- [ ] GPU marching-squares shader produces contour segments matching the CPU
      implementation
- [ ] Pipeline caching avoids redundant pipeline creation across frames
- [ ] Total GPU compute time (KDE + contour) is < 100 ms for 100K points on a
      256 × 256 grid
- [ ] CPU fallback is used automatically when compute shaders are unavailable

## Dependencies

### Prerequisite Stories

- GUP-250: Density Plot Builder ✅ — provides the WGSL shaders, CPU reference,
  and builder API

## Testing Strategy

- GPU integration test: dispatch KDE shader, read back texture, compare with
  CPU reference
- GPU marching-squares test: dispatch shader, read back vertex buffer, compare
  segment count and topology with CPU
- Performance benchmark: 100K points, 256×256 grid, measure GPU timestamp

## Definition of Done

- [ ] All acceptance criteria satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
