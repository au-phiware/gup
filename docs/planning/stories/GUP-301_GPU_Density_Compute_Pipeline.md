# GUP-301: GPU Density Compute Pipeline Integration

## Story Overview

**Initiative**: Chart Builders **Status**: ✅ Complete **Created**: 2025-07-15

## Context

GUP-250 delivered the `DensityPlotBuilder` with CPU-side 2D KDE computation and
marching-squares contour extraction, along with WGSL compute shaders
(`density_kde_2d.compute.wgsl` and `density_marching_squares.compute.wgsl`).
However, the compute shaders are standalone files — they are not yet wired into
the Gup GPU pipeline. The CPU path is adequate for small datasets (< 10K
points), but for 100K+ points the O(n × m²) KDE cost becomes prohibitive and GPU
dispatch is essential.

This story completes the GPU integration: creating bind groups, compute
pipelines, staging buffers, and connecting the dispatch to
`DensityPlotBuilder::build()` when the dataset size exceeds a configurable
threshold.

## User Story

> "As a visualization developer working with large datasets, I want the density
> plot to compute KDE on the GPU so that 100K+ point density plots remain
> interactive at 60 FPS."

## Acceptance Criteria

- [x] `DensityPlotBuilder` dispatches the 2D KDE compute shader when the sample
      count exceeds a configurable threshold (default: 5,000 points)
- [x] GPU KDE output matches CPU reference within 1% relative error for all
      three test distributions from GUP-250
- [x] GPU marching-squares shader produces contour segments matching the CPU
      implementation
- [x] Pipeline caching avoids redundant pipeline creation across frames
- [x] Total GPU compute time (KDE + contour) is < 100 ms for 100K points on a
      256 × 256 grid
- [x] CPU fallback is used automatically when compute shaders are unavailable

## Dependencies

### Prerequisite Stories

- GUP-250: Density Plot Builder ✅ — provides the WGSL shaders, CPU reference,
  and builder API

## Testing Strategy

- GPU integration test: dispatch KDE shader, read back texture, compare with CPU
  reference
- GPU marching-squares test: dispatch shader, read back vertex buffer, compare
  segment count and topology with CPU
- Performance benchmark: 100K points, 256×256 grid, measure GPU timestamp

## Definition of Done

- [x] All acceptance criteria satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

**Completed**: 2025-07-18

### What was implemented

- **`GpuDensityCompute`** — reusable GPU compute context that caches both the
  KDE and marching-squares compute pipelines. Creates shader modules, bind group
  layouts, and pipelines once on construction; reuses them for all subsequent
  dispatches.
- **`compute_kde()`** — dispatches the `density_kde_2d.compute.wgsl` shader,
  uploads data points as a storage buffer, writes to a `texture_storage_2d`
  output, and reads back the density grid via a staging buffer. Grid parameters
  (bounds, bandwidth) are computed on CPU using the same logic as
  `KernelDensity2D::generate_eval_grid` to ensure exact coordinate matching with
  the CPU reference.
- **`compute_contours()`** — dispatches the
  `density_marching_squares.compute.wgsl` shader for a single iso-level,
  uploading the density grid as a `texture_2d<f32>` and reading back the atomic
  vertex count and vertex buffer.
- **`gpu_density_2d()`** — convenience function with automatic CPU fallback:
  uses GPU when sample count ≥ threshold and a `RenderContext` is available,
  otherwise falls back to `compute_density_2d`.
- **`DensityConfig.gpu_threshold`** — configurable threshold (default: 5 000)
  controlling when GPU dispatch is activated.
- **`DensityPlotBuilder::gpu_threshold()`** — builder method to set the
  threshold.
- **Synchronous API** — all GPU methods are synchronous (no async/await),
  using `Mutex<Option<Result>>` with `device.poll(PollType::Wait)` for buffer
  mapping, making the API usable from any calling context.

### Key files changed

| File | Change |
| --- | --- |
| `src/chart_builder/builders/gpu_density.rs` | New — GPU compute module (1050 lines) |
| `src/chart_builder/builders/density.rs` | Added `gpu_threshold` to DensityConfig, wired GPU path into `build_with_data` |
| `src/chart_builder/builders.rs` | Added `gpu_density` module and re-export |
| `examples/density_scatter_overlay.rs` | Fixed for new DensityConfig field |

### Test counts

- **10 GPU integration tests** in `gpu_density.rs`:
  - Pipeline creation (1), pipeline reuse (1)
  - KDE GPU vs CPU: standard normal, uniform, mixture (3)
  - Marching squares GPU vs CPU: simple peak, no contour (2)
  - CPU fallback: below threshold, above threshold (2)
  - Performance: 100K points on 256×256 grid (1)
- **23 existing density tests** continue to pass
- **2933 total lib tests** pass
