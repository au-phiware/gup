# GUP-289: Integrate SelectionMaskBuffer into LinkedSelection

## Story Overview

**Initiative**: Interaction & Spatial Index **Status**: ✅ Complete **Created**:
2025-07-22 **Completed**: 2025-07-23

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

- [x] `LinkedSelection::prepare_render` automatically uses `SelectionMaskBuffer`
      when instance count exceeds a configurable threshold (default 10K)
- [x] The transition between CPU and GPU paths is transparent to the caller
- [x] A `gpu_dimming_threshold` builder method allows customising the cutover
      point
- [x] The GPU path output buffer is used for rendering instead of the CPU-built
      instance buffer
- [x] Performance regression tests verify no slowdown for small datasets (<1K)

## Technical Tasks

- [x] Add `SelectionMaskBuffer` as an optional field in `LinkedSelection`
- [x] Implement threshold-based path selection in `prepare_render`
- [x] Ensure source instance buffer is maintained for the GPU path
- [x] Add `gpu_dimming_threshold` builder method
- [x] Update existing `LinkedSelection` tests
- [x] Add integration tests for the automatic switchover

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

- [x] All Acceptance Criteria are satisfied
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`

## Implementation Summary

### What Was Implemented

- **GPU dimming path in `LinkedSelection::prepare_render`** — When the instance
  count meets or exceeds `gpu_dimming_threshold` (default 10,000) and the
  instance type provides `DimInstance::alpha_offsets`, dimming is performed
  entirely on the GPU via a compute shader. Below the threshold, the existing
  CPU-based `build_dimmed_instances` path is used.

- **`DimInstance::alpha_offsets()` method** — Added to the `DimInstance` trait
  with a default `None` return. Overridden for all built-in mark instance types
  (`CircleInstance`, `RectangleInstance`, `LineInstance`, `BoxPlotInstance`) to
  return the appropriate `AlphaOffsets`. This enables automatic GPU dimming
  without any caller-side configuration.

- **`gpu_dimming_threshold()` builder method** — Fluent builder method on
  `LinkedSelection` to customise the CPU/GPU cutover point. Set to `0` to force
  GPU path (useful for testing), or `u32::MAX` to force CPU path.

- **`Selection::instance_buffer()` accessor** — Returns `Option<&wgpu::Buffer>`
  for the GPU instance buffer, enabling external GPU operations (compute shader
  output copy) to target the Selection's render buffer directly.

- **Source buffer management** — A separate `source_buffer` holds undimmed
  instances for the compute shader's read-only input. Automatically
  created/invalidated when data changes via `set_data()`.

- **Instance buffer COPY_SRC flag** — Added to the Selection's instance buffer
  creation to support GPU readback (debugging, testing).

### Key Files Changed

| File                                  | Description                        |
| ------------------------------------- | ---------------------------------- |
| `src/linked_selection.rs`             | GPU path, threshold, alpha_offsets |
| `src/selection.rs`                    | instance_buffer(), COPY_SRC flag   |
| `tests/linked_selection_gpu_tests.rs` | 8 GPU integration tests            |

### Test Counts

- 9 new unit tests (threshold, builder, alpha_offsets, GPU state)
- 8 new GPU integration tests (path activation, correctness, selection changes,
  data rebuild, clear, no-op)
- All 32 existing `linked_selection` unit tests pass unchanged
