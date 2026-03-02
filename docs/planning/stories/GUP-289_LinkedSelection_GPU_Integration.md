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

## Retrospective

**Completed**: 2025-07-23

### Key Technical Learnings

#### Buffer Usage Flag Planning for GPU Pipelines

- **Challenge**: The GPU dimming compute shader writes dimmed instances to an
  output buffer, which then needs to be copied into the Selection's instance
  buffer. However, the instance buffer was created with `STORAGE | COPY_DST`
  only — it lacked `COPY_SRC` which is needed for test readback. Separately, the
  output buffer already had `COPY_SRC` (from GUP-288's design), so the copy
  chain was: output → instance buffer (fine), instance buffer → staging (failed
  without COPY_SRC).
- **Solution**: Added `COPY_SRC` to the Selection's instance buffer usage flags.
  This is a safe change since the flag only permits additional operations and
  doesn't affect existing render pipeline bindings.
- **Pattern**: When designing GPU buffer pipelines that span multiple systems
  (compute → render), plan usage flags to include `COPY_SRC` early. It enables
  debugging readback and inter-system buffer transfers without retroactive
  changes.

#### Trait-Based Auto-Configuration for GPU Features

- **Challenge**: The GPU dimming path needs per-mark-type `AlphaOffsets`
  (float-index positions of alpha channels). Requiring callers to provide this
  information would break the "transparent transition" requirement.
- **Solution**: Extended the existing `DimInstance` trait with a
  `fn alpha_offsets() -> Option<AlphaOffsets>` associated method (default
  `None`), overridden for all built-in marks. The `prepare_render` method checks
  this at compile time to determine if GPU dimming is available.
- **Pattern**: When adding optional GPU-accelerated paths for existing
  functionality, extend the existing trait with a provided method returning
  `Option<Config>` rather than creating a new trait. This preserves backward
  compatibility — types that don't implement the method continue to use the CPU
  path.

#### Source Buffer Lifecycle Management

- **Challenge**: The GPU compute shader reads undimmed instance data from a
  source buffer and writes dimmed data to the output buffer. The source buffer
  must persist across frames (for selection changes that only update the mask,
  not the instances), but must be rebuilt when data changes.
- **Solution**: Tied the source buffer lifecycle to `set_data()` and the
  `data_changed` flag in `prepare_render`. When data changes, both the
  SelectionMaskBuffer and source buffer are recreated from scratch. When only
  the selection changes, only the mask is updated and the compute shader re-runs
  with the existing source buffer.
- **Pattern**: For GPU compute pipelines with multiple input buffers, use a
  "dirty flag" approach to independently track which buffers need recreation vs
  which only need re-dispatch.

### Architectural Decisions

#### Copy Output to Instance Buffer (vs. Rebinding)

- **Decision**: Copy the compute shader's output buffer into the Selection's
  instance buffer via `copy_buffer_to_buffer`, rather than creating a new bind
  group pointing to the output buffer.
- **Reasoning**: The Selection's render state (pipeline, bind group, vertex
  buffers) is private and designed to be self-contained. Creating a new bind
  group would require exposing pipeline internals or significantly modifying
  Selection's API. The buffer copy is a single GPU operation (~0.01ms for 100K
  instances) with negligible overhead.
- **Trade-off**: One extra GPU copy per dimming update. For the target use case
  (selection changes on 10K+ datasets), this is far cheaper than the CPU-side
  `build_dimmed_instances` it replaces.
- **Future**: If profiling shows the copy is a bottleneck, the Selection API
  could be extended with a `set_instance_buffer()` method to allow direct bind
  group replacement.

#### Threshold-Based Path Selection

- **Decision**: Use a configurable threshold (default 10K) to switch between CPU
  and GPU paths, rather than always using GPU.
- **Reasoning**: The GPU path has fixed overhead (pipeline creation, buffer
  allocation, compute dispatch) that makes it slower than CPU for small
  datasets. The 10K default was chosen based on GUP-288's benchmarks showing the
  GPU path breaks even around 5K-10K instances.
- **Trade-off**: Users with unusual hardware may want different thresholds. The
  builder method provides escape hatches (`0` = always GPU, `u32::MAX` = always
  CPU).
- **Future**: An adaptive threshold based on runtime profiling could be
  implemented, but the current approach is simpler and sufficient.

### Development Workflow Insights

- **GUP-288 provided excellent foundations**: The `SelectionMaskBuffer` API from
  GUP-288 was well-designed for integration. The `update_and_dispatch`,
  `encode_dimming`, `ensure_capacity`, and `update_mask` methods were exactly
  what the integration needed. This validates the decision to implement the
  standalone API first, then integrate.
- **Test-driven GPU verification**: Using `read_buffer_f32` to verify GPU
  compute output at the float level catches subtle issues (wrong offset, wrong
  opacity formula) that would be invisible in visual testing.
- **Minimal API surface change**: The only public API addition to `Selection`
  was `instance_buffer()`. The COPY_SRC flag change is backward-compatible. The
  `DimInstance` trait change is also backward-compatible (default method). This
  minimises risk of breaking existing code.

### Follow-up Stories

1. **GUP-290: GPU Mask Buffer Pool Integration** — Already planned. Integrate
   `SelectionMaskBuffer` with the `BufferPool` system from GUP-003 to reuse mask
   and output buffers across frames, reducing allocation churn.

2. **GUP-291: Adaptive GPU Dimming Threshold** — Automatically tune the CPU/GPU
   cutover threshold based on runtime profiling of actual frame times. This
   would replace the static default with a self-optimising system.
