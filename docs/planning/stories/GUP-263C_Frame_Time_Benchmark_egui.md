# GUP-263C: Frame-time Benchmark for gup-egui Paths

## Story Overview

**Initiative**: Ecosystem Integration **Status**: 💡 New **Created**: 2025-07-22

## Context

GUP-263B introduced a zero-copy shared-device rendering path for gup-egui that
eliminates CPU-side pixel readback by registering the chart texture directly
with egui's renderer. While this should reduce frame-time overhead, the
improvement has not been quantified.

## User Story

> "As a developer evaluating gup-egui, I want to see benchmark data comparing
> the pixel-buffer fallback and zero-copy paths so I can make informed
> performance decisions."

## Acceptance Criteria

- [ ] A benchmark harness measures frame time for both rendering paths.
- [ ] Results are captured for at least two chart complexities (e.g. 100 and
      10,000 data points).
- [ ] A summary table or chart documents the measured improvement.
- [ ] The benchmark is reproducible via a cargo command or example.

## Technical Tasks

- [ ] Create a benchmark example or criterion bench comparing both paths.
- [ ] Measure frame time (GPU submit + present) for pixel-buffer vs shared
      device.
- [ ] Document results in a performance report or README section.

## Dependencies

### Prerequisite Stories

- GUP-263B: Shared wgpu Device for egui ✅

## Testing Strategy

- Benchmark reproducibility: running the benchmark twice produces consistent
  results (within 10% variance).

## Risk Assessment

- **Low**: Benchmarking infrastructure, no production code changes required.

## Definition of Done

- [ ] Benchmark harness created and runnable
- [ ] Results documented
- [ ] Story status updated to ✅ Complete
