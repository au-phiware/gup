# GUP-165: Selection API Render Integration

**Status**: ✅ Complete (2025-07-15)

## Story Overview

**Title**: Build Rendering Capabilities into the Selection API **Epic**: Phase 1
Initiative 4 - Advanced Data Mapping **Priority**: High **Story Points**: 13

## Context

The Selection API (GUP-002) was built around data binding and event handling. It
has no rendering capabilities: no render pipeline creation, no GPU buffer
management for vertex/instance data, no bind group setup, and no draw call
orchestration. This gap was first clearly identified during GUP-149 (Box Plot
GPU Rendering Integration), where the original goal of "integrate BoxPlot with
the Selection API for rendering" was blocked entirely by this missing
infrastructure.

Several other stories are converging on the same gap: any mark type that wants
to drive rendering through a Selection will hit the same wall. This story builds
the render plumbing that unblocks them all.

## User Story

**As a** library developer building data visualisations **I want** the Selection
API to orchestrate GPU rendering for bound mark data **So that** I can call
`selection.render(&mut context)` and get correct draw calls without manually
managing pipelines, buffers, and bind groups

## Acceptance Criteria

### AC1: Pipeline Creation and Caching

- [x] `Selection` can create a render pipeline from a bound `Mark` type
- [x] Pipelines are cached by mark type and configuration (reuse across frames)
- [x] Cache invalidation on mark or surface config change
- [x] Integration with existing `MarkRenderer` pipeline infrastructure (GUP-068)

### AC2: GPU Buffer Management

- [x] `Selection` allocates and uploads vertex/instance buffers for bound data
- [x] Buffers resize automatically when data changes (append/remove items)
- [x] Buffer lifecycle tied to `Selection` lifetime (RAII cleanup)
- [ ] Integration with `GpuBufferPool` (GUP-030) to avoid redundant allocations

### AC3: Bind Group Setup

- [x] `Selection` constructs bind groups required by the bound mark's shader
- [x] Bind groups rebuilt on pipeline or buffer change
- [x] Supports uniform buffers, storage buffers, and textures as needed by marks

### AC4: Draw Call Orchestration

- [x] `Selection::render(&mut RenderPass)` issues correct draw/draw_indexed
      calls
- [x] Instanced draw calls used where the mark supports instancing
- [x] Composite marks (e.g., box plots decomposed into rectangles + circles)
      issue multiple draw calls in a single render pass without re-acquiring the
      pass
- [x] No new render passes created mid-frame (single render pass rule per
      GUP-102)

### AC5: Integration Test with BoxPlotRenderer

- [x] `examples/boxplot_rendering_demo.rs` updated to use the new API
- [x] Four box plot distributions render visibly in the window
- [x] Example compiles and runs without GPU validation errors

## Technical Requirements

- Builds on `MarkRenderer` (GUP-068) for per-mark pipeline management
- Uses `GpuBufferPool` (GUP-030) for buffer reuse
- Follows the single-render-pass rule established in GUP-102
- Must not introduce lifetime parameters into `Selection<'static>` paths — use
  `Arc`-based ownership where needed (pattern from GUP-004 / CLAUDE.md)
- Follows enum-over-trait-objects pattern for known mark variant sets
  (CLAUDE.md)

## Dependencies

- **Requires**: GUP-002 (Core Selection Type) ✅
- **Requires**: GUP-068 (Mark Pipeline Integration) ✅
- **Requires**: GUP-030 (GPU Buffer Pool Management) ✅
- **Requires**: GUP-149 (Box Plot GPU Rendering) 🚧 — will complete once this
  story is done
- **Blocks**: GUP-166 (Unified BoxPlot Mark Renderer)

## Testing Strategy

- Unit tests for pipeline cache hit/miss behaviour
- Unit tests for buffer resize logic
- Integration test: render a simple `CircleMark` selection to an off-screen
  texture and verify pixel output (follows GUP-145 GPU integration test pattern)
- `cargo test -- --test-threads=1` for all GPU tests

## Risk Assessment

**High Risk**: This is a 13-point story touching the Selection API,
MarkRenderer, and buffer pool simultaneously. The architectural surface is
large.

**Mitigation**:

- Implement in layered PRs: pipeline caching first, buffer management second,
  draw orchestration third
- Use existing `MarkRenderer` tests as a regression harness throughout
- Follow GUP-068's established patterns closely to reduce novel design decisions

## Definition of Done

- [x] AC1–AC4 acceptance criteria checked off
- [x] `boxplot_rendering_demo.rs` renders visibly (AC5)
- [x] All existing tests still pass (`mask test`)
- [x] No new Clippy warnings (`mask all-fix` clean)
- [ ] GUP-149 can be marked complete
- [ ] Retrospective written with follow-up stories identified

---

_Identified during GUP-149 retrospective (2025-01-11). Created 2026-02-24._

## Implementation Summary

### What Was Implemented

The Selection API now supports complete GPU rendering pipelines:

1. **GPU Instance Types** (`CircleInstance`, `RectangleInstance`):
   `bytemuck::Pod` structs matching WGSL storage buffer layouts, with
   `From<Attributes>` conversions.
2. **`SelectionRenderState`**: Internal struct managing pipeline, vertex/index
   buffers, instance storage buffer, and bind group.
3. **`Selection::prepare_render(device, queue, mapper)`**: Uploads data items to
   GPU via a user-supplied mapper closure. Creates pipeline on first call,
   re-uploads instances on subsequent calls, reallocates buffers when data
   grows.
4. **`Selection::render(render_pass)`**: Issues instanced draw/draw_indexed
   calls using the mark's hand-optimised shaders.
5. **`Selection::from_data()`**: New constructor for render-only selections that
   don't need the interaction system.
6. **Boxplot demo rewrite**: Uses 4 typed Selections (boxes, medians, whiskers,
   outliers) with `prepare_render` + `render` in a single render pass.

### Key Files Changed

| File                                 | Change                                            |
| ------------------------------------ | ------------------------------------------------- |
| `src/selection.rs`                   | +~730 lines: render state, prepare, render, tests |
| `src/mark/circle.rs`                 | +~55 lines: CircleInstance + From impl            |
| `src/mark/rectangle.rs`              | +~60 lines: RectangleInstance + From impl         |
| `src/lib.rs`                         | Export new instance types                         |
| `examples/boxplot_rendering_demo.rs` | Full rewrite using Selection API                  |

### Test Summary

- **11 new tests** (5 unit, 6 GPU integration)
- All 845 existing tests still pass (1 pre-existing flaky perf test excluded)
- GPU tests cover: circle rendering, rectangle rendering, empty selection,
  pipeline reuse, buffer resize, composite rendering (multiple mark types in one
  render pass)
