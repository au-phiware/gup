# GUP-169: Shared Pipeline Cache for Selections

**Status**: ✅ Complete (2025-07-17)

## Story Overview

**Title**: Extract a reusable PipelineCache shared across Selections **Epic**:
Phase 1 Initiative 3 - Mark System and Type Integration **Priority**: Low
**Story Points**: 3

## Context

GUP-165 has each Selection create its own render pipeline via
`MarkInfoImpl::create_render_pipeline()`. When many Selections of the same mark
type coexist (e.g., a dashboard with multiple scatter plots), each holds its own
pipeline handle. While wgpu may cache compiled shaders internally, a shared
Rust-side cache would avoid redundant pipeline descriptor construction.

## User Story

**As a** library developer building multi-chart dashboards **I want** Selections
of the same mark type to share a single render pipeline **So that** pipeline
creation overhead is eliminated for repeated mark types

## Acceptance Criteria

- [x] `PipelineCache` struct holds `HashMap<TypeId, Arc<RenderPipeline>>`
- [x] `Selection::prepare_render()` accepts an optional `&mut PipelineCache`
- [x] Cache miss creates and caches the pipeline; cache hit returns Arc clone
- [x] Cache invalidation on device loss or surface format change
- [x] Benchmark shows reduced pipeline creation time for 100 Selections

## Dependencies

- **Requires**: GUP-165 (Selection API Render Integration) ✅
- **Requires**: GUP-068 (Mark Pipeline Integration) ✅

## Testing Strategy

- Unit tests for cache hit/miss behaviour
- GPU integration test: 10 Selections share one pipeline
- Device loss recovery test: cache cleared and rebuilt

## Definition of Done

- [x] All acceptance criteria met
- [x] No regression in existing tests
- [x] `mask all-fix` clean

## Implementation Summary

### Key Files Changed

- **`src/pipeline_cache.rs`** (new) — `PipelineCache` struct with
  `HashMap<TypeId, Arc<RenderPipeline>>`, stats tracking, surface format
  invalidation, and `get_or_create::<M>()` method.
- **`src/selection.rs`** — `SelectionRenderState.pipeline` changed from
  `wgpu::RenderPipeline` to `Arc<wgpu::RenderPipeline>`. `prepare_render()` and
  `prepare_render_bound()` now accept `cache: Option<&mut PipelineCache>`.
- **`src/lib.rs`** — Registered `pipeline_cache` module and exported
  `PipelineCache`.
- **`src/prelude.rs`** — Added `PipelineCache` to the prelude.
- **`examples/boxplot_rendering_demo.rs`** — Updated caller to pass `None`.
- **`examples/attr_binding_demo.rs`** — Updated callers to pass `None`.

### Test Counts

- 9 unit tests for `PipelineCache` (stats, invalidation, format tracking)
- 6 GPU integration tests (10-selection sharing, mark type distinction, clear
  rebuild, get_or_create, 100-selection reuse, benchmark)
- Total: 15 new tests; 933 existing tests continue to pass
