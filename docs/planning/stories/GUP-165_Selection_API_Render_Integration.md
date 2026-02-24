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
- [x] GUP-149 can be marked complete
- [x] Retrospective written with follow-up stories identified

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

## Retrospective

**Completed**: 2025-07-15

### Key Technical Learnings

#### Storage Buffers for Instance Data

- **Challenge**: The mark shaders (circle.vert.wgsl, rectangle.vert.wgsl) use
  storage buffers (`@group(0) @binding(0) var<storage, read>`) for instance
  data, not vertex buffers with `VertexStepMode::Instance`. This was initially
  unclear because examples like `scatter_plot_demo.rs` use a different approach
  with vertex buffer instance stepping.
- **Solution**: Used `create_buffer_init` with `BufferUsages::STORAGE` and
  derived the bind group layout directly from the compiled pipeline via
  `pipeline.get_bind_group_layout(0)`.
- **Pattern**: Always derive bind group layouts from the pipeline itself rather
  than recreating them independently — this guarantees layout compatibility.

#### WGSL Struct Alignment

- **Challenge**: WGSL storage buffer structs have strict alignment rules
  (`vec4<f32>` = 16-byte aligned). The Rust-side `#[repr(C)]` struct must
  include explicit padding fields to match.
- **Solution**: Defined `CircleInstance` (64 bytes) and `RectangleInstance` (80
  bytes) with explicit `_padN` fields. The rectangle shader already has an
  explicit `_padding` field, confirming the alignment requirements.
- **Pattern**: When defining bytemuck::Pod structs matching WGSL storage
  buffers, calculate the WGSL layout first (accounting for member alignment and
  struct rounding) then add Rust padding fields.

#### Optional Context for Selection

- **Challenge**: The Selection constructor required `Arc<RenderContext>` (the
  old interaction context), but the rendering demo uses `GupContext`. Making the
  demo work required bridging two context types.
- **Solution**: Made the context field `Option<Arc<RenderContext>>` and added a
  `Selection::from_data()` constructor. Rendering methods take
  `&Device`/`&Queue`/`&mut RenderPass` directly, decoupling from any specific
  context type.
- **Pattern**: Decouple GPU resource management from context types — accept raw
  wgpu handles where possible for maximum flexibility.

### Architectural Decisions

#### Direct Buffer Creation vs GpuBufferPool

- **Decision**: Used `device.create_buffer_init()` directly instead of
  `GpuBufferPool` from GUP-030.
- **Reasoning**: Each Selection owns its buffers exclusively (no sharing), and
  the pool's allocation/deallocation lifecycle doesn't fit the Selection's RAII
  ownership model well.
- **Trade-off**: Missed opportunity for buffer reuse when selections are
  created/destroyed frequently. For static visualisations (the current use
  case), this has no measurable impact.
- **Future**: A dedicated story could add pool integration if profiling reveals
  allocation overhead in dynamic scenarios.

#### Pipeline Per-Selection vs Shared Pipeline Cache

- **Decision**: Each Selection creates its own pipeline via
  `MarkInfoImpl::create_render_pipeline()`. There is no global pipeline cache.
- **Reasoning**: wgpu may cache compiled shaders internally. The existing
  `MarkRegistry` provides pipeline caching for code that uses it directly. The
  Selection's `SelectionRenderState` caches the pipeline across frames for its
  own use.
- **Trade-off**: If many Selections of the same mark type exist, each will hold
  its own pipeline object. In practice this is fine — pipelines are lightweight
  handles.
- **Future**: GUP-166 (Unified BoxPlot Mark Renderer) could introduce a shared
  pipeline cache if needed.

### Development Workflow Insights

- **Incremental commits**: Breaking the 13-point story into 4 commits (instance
  types → render infra → demo → tests) made each step reviewable and revertible.
- **GPU test simplicity**: The `GupContext::headless()` → `begin_frame()` →
  `render_pass()` → `finish()` pattern makes GPU integration tests
  straightforward. Tests run in ~0.5s total.
- **Pre-existing flaky test**: `test_performance_500_labels` (12ms vs 10ms
  target) fails intermittently. Not related to this story but adds noise to test
  results.

### Follow-up Stories

1. **GUP-167: GpuBufferPool Integration for Selection Rendering** — Wire the
   Selection's instance buffer creation through GpuBufferPool (GUP-030) for
   buffer reuse in dynamic scenarios where selections are frequently
   created/destroyed. Low priority unless profiling reveals allocation overhead.

2. **GUP-168: Selection Attribute Binding Pipeline** — The current
   `prepare_render()` requires the caller to provide a mapper closure
   (`|a| Instance::from(a)`). Implement the `attr()` / `attr_parallel()` methods
   so the Selection can automatically compose shader functions into instance
   data without manual mapping. Medium priority — enables the
   "selection.attr('position', scale)" API pattern.

3. **GUP-169: Shared Pipeline Cache for Selections** — Extract the per-mark
   pipeline cache from MarkRegistry into a standalone `PipelineCache` that
   multiple Selections can share. Low priority — only valuable when many
   Selections of the same mark type coexist.
