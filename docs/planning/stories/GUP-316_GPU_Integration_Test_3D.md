# GUP-316: GPU Integration Test for 3D Marks

## Story Overview

**Initiative**: Quality **Status**: 📋 Planned **Created**: 2026-03-04

## Context

GUP-261 delivered Sphere3D, Box3D, and Line3D marks with unit tests for geometry
and bytemuck layout. However, it lacks a GPU-level integration test that actually
renders instances through the full wgpu pipeline and verifies the output. Such a
test would catch shader compilation errors, bind-group mismatches, and depth
buffer issues in CI.

## User Story

> "As a library maintainer, I want GPU integration tests for 3D marks so that
> shader regressions and pipeline mismatches are caught automatically in CI."

## Acceptance Criteria

- [ ] A headless integration test renders 1000+ `Sphere3D` instances and asserts
      no wgpu validation layer errors
- [ ] The colour attachment is verified as non-zero (something was drawn)
- [ ] A headless integration test renders `Box3D` and `Line3D` instances
- [ ] Tests run with `--test-threads=1` without segfaults

## Technical Tasks

- [ ] Add `tests/three_d_integration.rs` with headless GPU tests
- [ ] Create a headless 3D render helper that sets up DepthBuffer, Camera,
      Light, and draws instances
- [ ] Assert non-zero pixel output by reading back the colour attachment
- [ ] Add a 100K instance performance assertion (< 16ms per frame)

## Dependencies

### Prerequisite Stories

- GUP-261: 3D Visualization Support ✅

## Testing Strategy

- GPU integration tests using `GupContext::headless()`
- Pixel readback to verify non-zero output

## Risk Assessment

- **Low**: Headless GPU testing is well-established in the project.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] Tests pass in CI with `--test-threads=1`
- [ ] Story status updated in INDEX.md
