# GUP-289: Integrate SelectionMaskBuffer into LinkedSelection

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: 🚧 In Progress
**Created**: 2025-07-22

## Context

GUP-288 introduced the `SelectionMaskBuffer` type with a GPU compute shader for
alpha dimming. Currently this is a standalone API — the caller must manually
create the buffer, upload the source instances, update the mask, and dispatch
the compute shader. The existing `LinkedSelection` type still uses the CPU-based
`build_dimmed_instances` path for all dataset sizes. This story wires the GPU
path into `LinkedSelection::prepare_render` so that charts transparently switch
to the GPU path when instance counts exceed a configurable threshold.

## User Story

> "As a visualization developer, I want LinkedSelection to automatically use
> GPU-side dimming for large datasets so that I don't need to manually manage
> SelectionMaskBuffer."

## Acceptance Criteria

- [ ] `LinkedSelection::prepare_render` automatically uses `SelectionMaskBuffer`
      when instance count exceeds a configurable threshold (default 10K)
- [ ] The transition between CPU and GPU paths is transparent to the caller
- [ ] A `gpu_dimming_threshold` builder method allows customising the cutover
      point
- [ ] The GPU path output buffer is used for rendering instead of the CPU-built
      instance buffer
- [ ] Performance regression tests verify no slowdown for small datasets (<1K)

## Technical Tasks

- [ ] Add `SelectionMaskBuffer` as an optional field in `LinkedSelection`
- [ ] Implement threshold-based path selection in `prepare_render`
- [ ] Ensure source instance buffer is maintained for the GPU path
- [ ] Add `gpu_dimming_threshold` builder method
- [ ] Update existing `LinkedSelection` tests
- [ ] Add integration tests for the automatic switchover

## Dependencies

### Prerequisite Stories

- GUP-288: GPU Selection Mask Buffer ✅ — provides SelectionMaskBuffer
- GUP-279: Linked View Coordination ✅ — provides LinkedSelection

## Testing Strategy

- Unit tests for threshold-based path selection
- GPU integration tests verifying correctness of automatic switchover
- Performance tests comparing small vs large dataset paths

## Risk Assessment

- **Low**: The GPU path is already tested in GUP-288. The main risk is correctly
  managing the source instance buffer lifetime.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
