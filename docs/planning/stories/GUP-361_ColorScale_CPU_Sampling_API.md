# GUP-361: ColorScale CPU Sampling API

## Story Overview

**Initiative**: Shader Functions **Status**: 💡 New **Created**: 2026-03-05

## Context

The `ColorScale` type provides GPU-side colour mapping via shader functions and
storage buffers. However, several chart builders (area, choropleth, density)
need to sample colour scales on the CPU during data pre-processing. Currently
each builder implements ad-hoc CPU gradient sampling. A unified
`ColorScale::sample(t)` method would eliminate duplication and ensure
consistency between CPU and GPU colour mapping.

## User Story

> "As a chart builder author, I want a CPU-side `ColorScale::sample()` method so
> that I can compute colours during data pre-processing without duplicating
> gradient interpolation logic."

## Acceptance Criteria

- [ ] `ColorScale` has a `sample(t: f32) -> [f32; 4]` method
- [ ] Supports all `ColorScaleKind` variants (Continuous, Diverging, Quantize)
- [ ] Output matches GPU shader output within floating-point tolerance
- [ ] Existing ad-hoc CPU sampling code is refactored to use the new method
- [ ] Documentation and doc-tests for the new API

## Dependencies

### Prerequisite Stories

- GUP-298: Filled Polygon Mark ✅ — identified the need via `sample_gradient_cpu`

## Testing Strategy

- Unit tests for each `ColorScaleKind`
- Parity tests comparing CPU `sample()` with GPU shader output
- Edge cases: domain boundaries, single-stop gradients, zero-range domains

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass
- [ ] Lint and format clean
- [ ] Documentation updated
