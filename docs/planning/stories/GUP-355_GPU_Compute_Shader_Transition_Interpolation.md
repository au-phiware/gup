# GUP-355: GPU Compute Shader Transition Interpolation

## Story Overview

**Initiative**: Selection API  
**Status**: 💡 New  
**Created**: 2025-07-26

## Context

GUP-277 implemented CPU-side transition interpolation in
`prepare_render_bound()`. While correct and well-tested, this approach performs
per-element interpolation on the CPU, which scales linearly with element count.
For large datasets (10K+ elements), a GPU compute shader approach would be
significantly more performant.

This story adds an optional GPU-side interpolation path: upload from/to
attribute buffers and a time uniform, then run a compute shader that produces
interpolated instance data in-place.

## User Story

> "As a visualization developer working with large datasets, I want transitions
> to interpolate on the GPU so that animations remain smooth even with 10K+
> elements."

## Acceptance Criteria

- [ ] A compute shader performs from/to interpolation for all elements in a
      single dispatch.
- [ ] The existing CPU path remains as the default; the compute path is opt-in
      via a configuration flag or automatic threshold.
- [ ] Performance benchmarks show measurable improvement for 10K+ element
      transitions compared to the CPU path.
- [ ] All existing transition tests continue to pass.

## Dependencies

### Prerequisite Stories

- GUP-277: GPU Render Loop Transition Integration ✅ — provides the CPU-side
  transition interpolation and `CommittedTransition` infrastructure.

## Testing Strategy

- Benchmark comparison: CPU vs GPU interpolation at 1K, 10K, 100K elements.
- GPU validation tests with no errors during compute dispatch.
- Integration test verifying identical output between CPU and GPU paths.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Story status updated to ✅ Complete in story file and INDEX.md

## Risk Assessment

- **Medium**: Compute shader dispatch adds complexity to the render pipeline.
  Need to ensure proper synchronisation between compute output and vertex input.
- **Low**: The CPU fallback path means this is purely an optimisation — if the
  compute path has issues, the system degrades gracefully.
