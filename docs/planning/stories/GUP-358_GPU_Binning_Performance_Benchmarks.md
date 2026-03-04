# GUP-358: GPU Binning Performance Benchmarks

## Story Overview

**Initiative**: Performance  
**Status**: 📋 Planned  
**Created**: 2025-07-18

## Context

GUP-297 delivered GPU compute shader 2D binning with correctness verified via
CPU/GPU equivalence tests, but formal performance benchmarks were not included.
The acceptance criterion of "10M records in a 100×100 grid in under 50 ms" needs
validation on CI hardware, and the crossover point (dataset size where GPU
becomes faster than CPU) should be documented.

## User Story

> "As a contributor, I want formal benchmark results so that I can make informed
> decisions about when to recommend `.gpu_binning(true)` to end users."

## Acceptance Criteria

- [ ] `criterion` benchmarks for GPU and CPU binning at 1M, 10M, and 100M
      records with 10×10, 100×100, and 500×500 grids.
- [ ] Benchmark results documented in `docs/PERFORMANCE_GUIDE.md`.
- [ ] Crossover point (N where GPU outperforms CPU) identified and documented.
- [ ] CI job validates that 10M/100×100 GPU binning completes under 100 ms (2×
      headroom over the 50 ms target).

## Technical Tasks

- [ ] Add `criterion` benchmarks in `benches/gpu_binning.rs`.
- [ ] Run benchmarks on CI hardware and document results.
- [ ] Update `docs/PERFORMANCE_GUIDE.md` with GPU binning section.
- [ ] Add a CI performance gate for GPU binning latency.

## Dependencies

### Prerequisite Stories

- GUP-297: GPU Compute Shader 2D Binning ✅

## Testing Strategy

- Benchmark stability: run each configuration 10+ times, report mean ± stddev.
- Compare release vs debug mode to ensure benchmarks use optimised builds.

## Risk Assessment

- **Low**: Benchmark results are hardware-dependent; CI hardware may differ from
  user environments.

## Definition of Done

- [ ] Benchmarks added and passing
- [ ] Results documented in performance guide
- [ ] CI gate configured
